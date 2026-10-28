//! Content-addressed word identity (Section 8.6).
//!
//! Each user word has a stable identity derived from its content, not its name:
//! `id = H(normalize(body) ⊕ { id(dep) })`. Every reference inside a body is
//! replaced by the identity of its target before hashing, so identity is
//! independent of which names happen to be in scope. A recursive group (a
//! strongly connected component of the dependency graph) is hashed as a unit
//! with its members referenced by position within the component.
//!
//! The digest below is BLAKE3, a standard cryptographic hash, which is what
//! LANG.AUTHORITY.FREEDOM requires of an identity: distinct content must not
//! be able to share an identity, by accident or by construction. It replaced a
//! deterministic polynomial hash that was collision-resistant against neither.
//! That mattered beyond an attacker model, because a shared `body_content_key`
//! makes the content store hand one word's body to another (`execute_def.rs`).
//!
//! The crate is built with `pure` plus `no_sse2` / `no_sse41` / `no_avx2` /
//! `no_avx512` (see `Cargo.toml`), so only `portable.rs` — zero `unsafe` — is
//! compiled. Its exact byte encoding, including the `#` prefix and the
//! lowercase 64-hex body, is an implementation contract (Section 2.1/2.3).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::types::{Token, WordDefinition};
use crate::word_name::canonical_word_name;

use super::Interpreter;

/// Deterministic content digest. Returns a `#`-prefixed 64-hex-char string.
/// The basis of content addressing (LANG.DICTIONARY.MUTATION): a Word's
/// identity is a digest over its normalized definition and the identities of
/// the Words it calls.
pub(crate) fn content_digest(bytes: &[u8]) -> String {
    format!("#{}", blake3::hash(bytes).to_hex())
}

/// Append a payload so it can never forge a boundary: the separators
/// (`0x1c`..=`0x1f`) and the escape byte itself (`0x1b`) are written as
/// `0x1b` followed by the byte. A String or a name may hold any of them
/// (`nameCharacter` is any non-whitespace), and an unescaped `0x1f` + tag
/// inside one would read as a token boundary, so two different bodies could
/// share a key — and the content store hand one Word's body to the other.
/// A payload holding none of those bytes is written unchanged, so such a
/// body keeps the identity it always had.
fn push_payload(bytes: &mut Vec<u8>, payload: &[u8]) {
    for &b in payload {
        if (0x1b..=0x1f).contains(&b) {
            bytes.push(0x1b);
        }
        bytes.push(b);
    }
}

/// Canonical byte encoding for one token: the tag-byte assignment
/// `body_content_key` uses (`N`/`n` for a number, parsed or raw; `S` for a
/// string; `Y` for a canonicalized symbol; `[`/`]`/`{`/`}`/`^`/`|`/`\n` for
/// structural tokens).
fn encode_token(bytes: &mut Vec<u8>, tok: &Token) {
    match tok {
        Token::Number(literal) => match literal.value() {
            Some(frac) => {
                let (num, den) = frac.to_bigint_pair();
                bytes.push(b'N');
                bytes.extend_from_slice(num.to_str_radix(16).as_bytes());
                bytes.push(b'/');
                bytes.extend_from_slice(den.to_str_radix(16).as_bytes());
            }
            None => {
                bytes.push(b'n');
                push_payload(bytes, literal.lexeme().as_bytes());
            }
        },
        Token::String(s) => {
            bytes.push(b'S');
            push_payload(bytes, s.as_bytes());
        }
        Token::Symbol(s) => {
            bytes.push(b'Y');
            push_payload(bytes, canonical_word_name(s).as_bytes());
        }
        Token::VectorStart => bytes.push(b'['),
        Token::VectorEnd => bytes.push(b']'),
        Token::Value(value) => {
            bytes.push(b'V');
            bytes.extend_from_slice(value_identity(value).as_bytes());
        }
    }
}

/// The identity of a value a body carries whole (`Token::Value`): its
/// denotation digest (LANG.VALUES.DENOTATION), so equal values identify
/// alike.
fn value_identity(value: &crate::types::Value) -> String {
    crate::interpreter::value_digest::value_digest(value)
}

/// Canonical content key for a word body, independent of references' identities
/// (references are keyed by their canonical spelling). Two textually identical
/// bodies — e.g. the same definition exported and imported again under another
/// name — produce the same key and can share one stored body (LANG.AUTHORITY.FREEDOM
/// content store). Exact-rational numbers are normalized so `1` and `1/1` agree.
pub(crate) fn body_content_key(body: &[Token]) -> String {
    let mut bytes = Vec::new();
    for tok in body {
        bytes.push(0x1f);
        encode_token(&mut bytes, tok);
    }
    content_digest(&bytes)
}

/// One canonical element of a word body.
enum Atom {
    /// A reference to another user word (a dependency), by name.
    Ref(String),
    /// Any other token, already serialized to canonical bytes.
    Raw(Vec<u8>),
}

fn structural_atom(tag: u8) -> Atom {
    Atom::Raw(vec![tag])
}

/// A number literal's atom: the bytes `encode_token` writes for it, so the
/// body shape and the identity encoding cannot spell a number two ways. (An
/// unparseable literal falls back to its raw spelling there, so the shape
/// stays total and deterministic.)
fn number_atom(literal: &crate::types::NumberLiteral) -> Atom {
    let mut bytes = Vec::new();
    encode_token(&mut bytes, &Token::Number(literal.clone()));
    Atom::Raw(bytes)
}

/// Serialize a body shape to bytes, encoding each user-word reference with the
/// supplied closure (which differs between the pre-ordering pass and the final
/// pass for recursive components).
fn serialize_shape(atoms: &[Atom], encode_ref: &dyn Fn(&str) -> Vec<u8>) -> Vec<u8> {
    let mut out = Vec::new();
    for atom in atoms {
        match atom {
            Atom::Raw(b) => {
                out.push(0x1f);
                out.extend_from_slice(b);
            }
            Atom::Ref(target) => {
                out.push(0x1e);
                out.extend_from_slice(&encode_ref(target));
            }
        }
    }
    out
}

/// Tarjan's SCC algorithm. Returns components in reverse topological order
/// (dependencies before the words that depend on them), with deterministic
/// node iteration so the result does not depend on hash-map ordering.
fn tarjan_sccs(nodes: &HashSet<String>, adj: &HashMap<String, Vec<String>>) -> Vec<Vec<String>> {
    struct State<'a> {
        adj: &'a HashMap<String, Vec<String>>,
        index: usize,
        indices: HashMap<String, usize>,
        lowlink: HashMap<String, usize>,
        on_stack: HashSet<String>,
        stack: Vec<String>,
        out: Vec<Vec<String>>,
    }

    fn strongconnect(st: &mut State, v: &str) {
        st.indices.insert(v.to_string(), st.index);
        st.lowlink.insert(v.to_string(), st.index);
        st.index += 1;
        st.stack.push(v.to_string());
        st.on_stack.insert(v.to_string());

        if let Some(neighbors) = st.adj.get(v) {
            for w in neighbors.clone() {
                if !st.indices.contains_key(&w) {
                    strongconnect(st, &w);
                    let lw = st.lowlink[&w];
                    let lv = st.lowlink[v];
                    st.lowlink.insert(v.to_string(), lv.min(lw));
                } else if st.on_stack.contains(&w) {
                    let iw = st.indices[&w];
                    let lv = st.lowlink[v];
                    st.lowlink.insert(v.to_string(), lv.min(iw));
                }
            }
        }

        if st.lowlink[v] == st.indices[v] {
            let mut comp = Vec::new();
            loop {
                let w = st.stack.pop().unwrap();
                st.on_stack.remove(&w);
                let is_root = w == v;
                comp.push(w);
                if is_root {
                    break;
                }
            }
            st.out.push(comp);
        }
    }

    let mut st = State {
        adj,
        index: 0,
        indices: HashMap::new(),
        lowlink: HashMap::new(),
        on_stack: HashSet::new(),
        stack: Vec::new(),
        out: Vec::new(),
    };

    let mut sorted_nodes: Vec<&String> = nodes.iter().collect();
    sorted_nodes.sort();
    for v in sorted_nodes {
        if !st.indices.contains_key(v) {
            strongconnect(&mut st, v);
        }
    }
    st.out
}

impl Interpreter {
    /// Content identity of a user word, if it has been computed.
    pub(crate) fn word_identity(&self, name: &str) -> Option<&String> {
        self.word_identities.get(name)
    }

    /// Reclaim content-store bodies no longer referenced by any definition.
    /// An entry whose only remaining strong reference is the store itself
    /// (`strong_count == 1`) is orphaned — deleted or replaced by a redefine —
    /// and is dropped. A body still shared by one or more live definitions has
    /// a higher count and is kept. Run only at quiescent points (after a
    /// definition, deletion, or dependency rebuild), where no transient body
    /// clones are in flight; deferred during bulk operations like the rest of
    /// the content-store maintenance.
    pub(crate) fn gc_body_store(&mut self) {
        if self.defer_identity_recompute {
            return;
        }
        self.body_store
            .retain(|_, body| std::sync::Arc::strong_count(body) > 1);
    }

    fn build_word_shape(&self, def: &WordDefinition, user_set: &HashSet<String>) -> Vec<Atom> {
        let mut atoms = Vec::new();
        for tok in def.body.iter() {
            let atom = match tok {
                Token::Number(literal) => number_atom(literal),
                Token::String(s) => {
                    let mut b = vec![b'S'];
                    push_payload(&mut b, s.as_bytes());
                    Atom::Raw(b)
                }
                Token::Symbol(s) => self.symbol_atom(s, def, user_set),
                Token::VectorStart => structural_atom(b'['),
                Token::VectorEnd => structural_atom(b']'),
                Token::Value(value) => {
                    let mut b = vec![b'V'];
                    b.extend_from_slice(value_identity(value).as_bytes());
                    atoms.push(Atom::Raw(b));
                    // A name inside a value the body carries whole is one the
                    // body can run (`body_symbols`), so the Word it resolves to
                    // is part of this identity exactly as a top-level
                    // Symbol's is — or a redefinition of it would change what
                    // this Word does and leave its identity where it was.
                    let mut names = Vec::new();
                    crate::interpreter::body_symbols::value_symbol_names(value, &mut names);
                    for name in names {
                        atoms.push(self.symbol_atom(&name, def, user_set));
                    }
                    continue;
                }
            };
            atoms.push(atom);
        }
        atoms
    }

    /// The identity atom for one name a body holds.
    fn symbol_atom(&self, s: &str, def: &WordDefinition, user_set: &HashSet<String>) -> Atom {
        let canon = canonical_word_name(s);
        match self.resolve_word_entry(&canon) {
            Some((resolved, rdef)) => {
                if def.dependencies.contains(resolved.as_ref()) {
                    // A user-word dependency fixed at definition time:
                    // encode the recorded target rather than treating a
                    // later same-named word as a fresh capture.
                    Atom::Ref(resolved.to_string())
                } else if rdef.is_builtin {
                    // Core word: stable global vocabulary,
                    // encoded by its canonical resolved name.
                    let mut b = vec![b'G'];
                    push_payload(&mut b, resolved.as_bytes());
                    Atom::Raw(b)
                } else if user_set.contains(resolved.as_ref()) {
                    // A user word not recorded in this definition's
                    // dependency set was not resolved when the word was
                    // authored. Keep it as a free symbol so adding an
                    // unrelated word cannot recapture existing content.
                    let mut b = vec![b'F'];
                    push_payload(&mut b, canon.as_bytes());
                    Atom::Raw(b)
                } else {
                    Atom::Ref(resolved.to_string())
                }
            }
            // Free / unresolved symbol: encoded by canonical name.
            None => {
                let mut b = vec![b'F'];
                push_payload(&mut b, canon.as_bytes());
                Atom::Raw(b)
            }
        }
    }

    /// Recompute content identities for every user word (Section 8.6). Cheap
    /// enough to run after any change to the user-word graph.
    pub(crate) fn recompute_word_identities(&mut self) {
        // Deferred during bulk operations; the batch recomputes once at the end.
        if self.defer_identity_recompute {
            return;
        }

        // 1. Snapshot all user words. One User tier, so a name is the whole
        //    address and there is no dictionary to carry alongside it.
        let words: Vec<(String, Arc<WordDefinition>)> = self
            .user_words
            .iter()
            .map(|(name, def)| (name.clone(), def.clone()))
            .collect();
        let user_set: HashSet<String> = words.iter().map(|(name, _)| name.clone()).collect();
        let reg_order: HashMap<String, u64> = words
            .iter()
            .map(|(name, def)| (name.clone(), def.registration_order))
            .collect();

        // 2. Canonical shape of each body; references to other user words are
        //    captured as Atom::Ref.
        let mut shapes: HashMap<String, Vec<Atom>> = HashMap::new();
        for (name, def) in &words {
            let atoms = self.build_word_shape(def, &user_set);
            shapes.insert(name.clone(), atoms);
        }

        // 3. Dependency graph restricted to user words.
        let mut adj: HashMap<String, Vec<String>> = HashMap::new();
        for (fq, atoms) in &shapes {
            let mut seen = HashSet::new();
            let mut neighbors = Vec::new();
            for atom in atoms {
                if let Atom::Ref(target) = atom {
                    if user_set.contains(target) && seen.insert(target.clone()) {
                        neighbors.push(target.clone());
                    }
                }
            }
            adj.insert(fq.clone(), neighbors);
        }

        // 4. SCCs in reverse topological order (dependencies first).
        let sccs = tarjan_sccs(&user_set, &adj);

        // 5. Assign identities component by component.
        let mut ids: HashMap<String, String> = HashMap::new();
        for scc in &sccs {
            let scc_set: HashSet<String> = scc.iter().cloned().collect();

            // Pre-ordering digest: intra-SCC references blanked (so the ordering
            // does not depend on which member calls which), extra-SCC references
            // by their already-computed identity.
            let mut pre: HashMap<String, String> = HashMap::new();
            for m in scc {
                let bytes = serialize_shape(&shapes[m], &|target: &str| {
                    if scc_set.contains(target) {
                        b"C?".to_vec()
                    } else {
                        let mut v = vec![b'I'];
                        if let Some(id) = ids.get(target) {
                            v.extend_from_slice(id.as_bytes());
                        }
                        v
                    }
                });
                pre.insert(m.clone(), content_digest(&bytes));
            }

            let mut ordered: Vec<String> = scc.clone();
            ordered.sort_by(|a, b| {
                pre[a]
                    .cmp(&pre[b])
                    .then_with(|| reg_order.get(a).cmp(&reg_order.get(b)))
                    .then_with(|| a.cmp(b))
            });
            let mut position: HashMap<String, usize> = HashMap::new();
            for (i, m) in ordered.iter().enumerate() {
                position.insert(m.clone(), i);
            }

            // Final shape bytes: intra-SCC references by position, extra-SCC by id.
            let mut final_bytes: HashMap<String, Vec<u8>> = HashMap::new();
            for m in scc {
                let bytes = serialize_shape(&shapes[m], &|target: &str| {
                    if scc_set.contains(target) {
                        let mut v = vec![b'P'];
                        v.extend_from_slice(position[target].to_string().as_bytes());
                        v
                    } else {
                        let mut v = vec![b'I'];
                        if let Some(id) = ids.get(target) {
                            v.extend_from_slice(id.as_bytes());
                        }
                        v
                    }
                });
                final_bytes.insert(m.clone(), bytes);
            }

            // Combined digest over members in canonical position order.
            let mut combined_input = Vec::new();
            for (i, m) in ordered.iter().enumerate() {
                combined_input.extend_from_slice(i.to_string().as_bytes());
                combined_input.push(0x1d);
                combined_input.extend_from_slice(&final_bytes[m]);
                combined_input.push(0x1c);
            }
            let combined = content_digest(&combined_input);

            for m in scc {
                let pos = position[m];
                let mut id_bytes = combined.clone().into_bytes();
                id_bytes.push(b':');
                id_bytes.extend_from_slice(pos.to_string().as_bytes());
                ids.insert(m.clone(), content_digest(&id_bytes));
            }
        }

        self.word_identities = ids;
    }
}
