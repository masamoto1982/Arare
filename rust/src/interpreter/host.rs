//! Host abstraction for the one effect that leaves the machine.
//!
//! Ajisai Core is host-independent. Output is the only effect a host can
//! observe (LANG.EFFECTS.OUTPUT): `PRINT` appends to the ordered output
//! stream, and no other Word emits output. The language's other effect,
//! dictionary mutation by `DEF`/`DEL` (LANG.DICTIONARY.MUTATION), stays inside
//! the machine and never reaches this channel. When `PRINT` runs it produces a
//! structured `HostEffect` rather than only appending a string to
//! `output_buffer`.
//!
//! The conformance suite (`tests/conformance/`) observes the ordered sequence of
//! `HostEffect`s, not the human-readable `output_buffer`. Structuring the effect
//! this way lets two independent implementations be compared
//! language-independently: they agree iff they emit the same effect sequence.

use std::sync::{Arc, Mutex};

use crate::error::{AjisaiError, Result};
use crate::interpreter::Interpreter;

/// A structured effect request. Output is the only effect that crosses the host
/// boundary, so this carries exactly one variant; it stays an enum because the conformance
/// suite matches on the stable `kind` tag and the protocol pins that shape.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum HostEffect {
    Print(String),
}

impl HostEffect {
    /// Stable, language-independent kind tag. This is the string the
    /// conformance suite carries in `data-kind` on each `ajisai-effect`.
    pub fn kind(&self) -> &'static str {
        match self {
            HostEffect::Print(_) => "print",
        }
    }

    /// The effect payload. Conformance carries this in `data-payload`.
    pub fn payload(&self) -> &str {
        match self {
            HostEffect::Print(s) => s,
        }
    }
}

/// Runtime boundary supplied by the embedding host.
///
/// The interpreter owns no effect sink of its own. A host may render, capture,
/// or discard the output stream, but may not reorder it.
pub trait HostEnv: Send + Sync {
    fn emit_effect(&self, _effect: &HostEffect) {}
}

/// Default host: discards the structured effect, because the interpreter's own
/// `output_buffer` is what the GUI and CLI read.
#[derive(Debug, Default)]
pub struct DefaultHostEnv;

impl HostEnv for DefaultHostEnv {}

pub fn default_host_env() -> Arc<dyn HostEnv> {
    Arc::new(DefaultHostEnv)
}

/// Recording host used by conformance tests to observe the ordered effect
/// sequence directly rather than through rendered output text.
#[derive(Debug, Default)]
pub struct RecordingHostEnv {
    emitted_effects: Mutex<Vec<HostEffect>>,
}

impl RecordingHostEnv {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn emitted_effects(&self) -> Vec<HostEffect> {
        self.emitted_effects.lock().unwrap().clone()
    }
}

impl HostEnv for RecordingHostEnv {
    fn emit_effect(&self, effect: &HostEffect) {
        self.emitted_effects.lock().unwrap().push(effect.clone());
    }
}

// Looking a Word up is something the *host* does, not something a program does.
//
// This was `LOOKUP`, a Word: `'ADD' ?` ran through the interpreter, and the
// answer came back out of two fields on `Interpreter` that no evaluation rule
// ever read — the host drained them after the run and painted them somewhere.
// A Word whose entire result is a side channel to the editor is not part of the
// language, and its presence in the vocabulary made two claims that were not
// true: that a program can observe reference prose, and that the reference
// prose is a value.
//
// The names are the same, the spelling at the keyboard is the same, and the
// answer is the same text. What changed is who asks: the host parses
// `'ADD' ?` itself and calls in here, and the interpreter never sees it.

/// The text a lookup produced.
///
/// Both variants are prose to *read*, and a host shows them the same way, in
/// its output area (spec/gui-semantics.md, Lookup): the cursor can be anywhere
/// in a program still being written, so nothing about a lookup may overwrite
/// the editor — an earlier design loaded a definition back into it, and a
/// lookup then replaced a half-written program. They stay two variants because
/// they are two different texts, a reference entry and a reconstructed source,
/// and a host may present the source as source rather than as a reference page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostLookup {
    /// A Core Word's reference entry, to display.
    Documentation(String),
    /// A User Word's reconstructed `DEF` source, to display as read-only
    /// reference.
    Definition(String),
}

/// Resolve `name` against the dictionary the session currently holds.
///
/// Fails with `UnknownWord` for a name that is not defined — the host reports
/// that the same way it reports any other unknown name, so a typo at the
/// lookup prompt reads like a typo in a program.
pub fn resolve_host_lookup(interp: &Interpreter, name: &str) -> Result<HostLookup> {
    let canonical_name = crate::word_name::canonical_word_name(name);

    let Some(def) = interp.resolve_word(&canonical_name) else {
        return Err(AjisaiError::UnknownWord(name.to_string()));
    };

    if def.is_builtin {
        return Ok(HostLookup::Documentation(
            crate::builtins::lookup_builtin_detail(name),
        ));
    }

    let definition = interp
        .lookup_word_definition_tokens(&canonical_name)
        .unwrap_or_default();
    Ok(HostLookup::Definition(render_def_source(
        &definition,
        name,
        def.description.as_deref(),
    )))
}

/// Reconstruct the `DEF` source of a User Word so that running it again defines
/// the same word — the point of looking one up being to see an existing
/// definition as the program that would define it once more.
///
/// The body is wrapped in `[ ]`, the only bracket a program's code is written
/// in: `DEF` takes any Vector as its body, and a bare name inside one is a
/// Symbol — data until something executes it — so a `[ ]`-wrapped body
/// round-trips exactly like the one that defined the word, whatever it called.
///
/// A multi-line body keeps its line structure. Where the break falls below the
/// body's own level it is presentation rather than meaning — but it is the
/// author's presentation, and a definition that comes back reformatted reads
/// as a definition that was changed.
///
/// A description is emitted as a leading `#` comment. `DEF` takes exactly two
/// positional arguments, so a third string on the line would be read as the
/// word's name; the comment carries the text without changing what runs.
fn render_def_source(definition: &str, name: &str, description: Option<&str>) -> String {
    let body = if definition.is_empty() {
        "[ NIL ]".to_string()
    } else if definition.contains('\n') {
        format!("[\n{}\n]", definition)
    } else {
        format!("[ {} ]", definition)
    };
    let source = format!("{} '{}' DEF", body, name);
    match description {
        Some(desc) if !desc.is_empty() => format!("# {}\n{}", desc.replace('\n', " "), source),
        _ => source,
    }
}
