use crate::error::{AjisaiError, Result};
use crate::interpreter::value_extraction_helpers::extract_word_name_from_value;
use crate::interpreter::{Interpreter, WordDefinition};
use crate::types::{Token, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Scan raw source for `#:contract NAME ...` lines, keyed by the upper-cased
/// name. `#:contract` is a tooling-only directive (an ordinary comment to the
/// interpreter, stripped before tokenization — `docs/dev/cost-contract-
/// design.md`); this does not parse or check its fields the way the CLI's
/// `agent::contract` does (that module is `std`-only and unavailable to
/// the browser build). It only recovers the line's text, verbatim, so `DEF`
/// can attach it to the Word it names as a `description` for the host to
/// show — e.g. the Dictionary panel's hover — never as a checked contract.
pub(crate) fn extract_pending_word_descriptions(source: &str) -> HashMap<String, String> {
    let mut descriptions = HashMap::new();
    for line in source.lines() {
        let Some(rest) = line.trim_start().strip_prefix("#:contract") else {
            continue;
        };
        let rest = rest.trim();
        let Some(name_end) = rest.find(char::is_whitespace) else {
            continue;
        };
        let name = &rest[..name_end];
        let detail = rest[name_end..].trim();
        if name.is_empty() || detail.is_empty() {
            continue;
        }
        descriptions.insert(name.to_uppercase(), detail.to_string());
    }
    descriptions
}

/// Overwrite an existing Word's `description` in place. Used both when `DEF`
/// consumes a pending `#:contract` line for the Word it just defined, and
/// when a restored Word (`restore_user_words`) carries a saved description
/// alongside its body. The Word was just inserted with a fresh `Arc` in
/// every caller, so `Arc::get_mut` succeeds; the clone-and-replace fallback
/// only guards a future caller that might not hold sole ownership.
pub(crate) fn set_word_description(
    interp: &mut Interpreter,
    name: &str,
    description: Option<String>,
) {
    let upper_name = name.to_uppercase();
    let Some(arc_def) = interp.user_words.get_mut(&upper_name) else {
        return;
    };
    if let Some(def) = Arc::get_mut(arc_def) {
        def.description = description;
    } else {
        let mut cloned = (**arc_def).clone();
        cloned.description = description;
        *arc_def = Arc::new(cloned);
    }
}

/// DEF is strictly two positional arguments: `[ body ] 'NAME' DEF`.
///
/// The top of the stack is the name (a string), and directly below it is the
/// body — any Vector, since code and data share one Vector domain — usually
/// written as a literal `[ ]` right there, but not required to be: a Vector
/// built, stored, or passed through any other means defines just as well. No
/// value types are inspected to *guess* roles — position alone determines them
/// — which is why a leftover string-like value on the stack can no longer shift
/// argument interpretation.
pub fn op_def(interp: &mut Interpreter) -> Result<()> {
    if interp.stack.len() < 2 {
        return Err(AjisaiError::stack_underflow());
    }

    // Every refusal below puts both operands back as they were written (the
    // ERROR discipline of every Core Word, LANG.STACK.CONSUMPTION): a Word
    // consumes its operands only once it has answered.
    let name_val = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;
    let name_str = match extract_word_name_from_value(&name_val) {
        Ok(name) => name,
        Err(e) => {
            interp.stack.push(name_val);
            return Err(e);
        }
    };

    let def_val = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;

    // Prefer the body's own written tokens when the operand was a literal
    // right here (`execution_loop.rs`'s `def_body_tokens_if_literal_precedes_def`,
    // see `pending_def_body_tokens`'s doc comment): re-deriving them from
    // `def_val` through `value_as_code.rs` always re-expands a nested Vector
    // as `[ ]`, which is fine for *running* the body (either spelling
    // executes identically) but loses exactly the bracket-spelling fact the
    // contract inference's code-or-data reading (`word_contract.rs`) reads to
    // tell code from data. `None` here just means the body came from a
    // computed Vector rather than a literal, and the bridge is the only way
    // to get tokens from it — the source-writing bridge, since a definition
    // is kept as its source and a value the Vector carries whole has to be
    // written as what builds it.
    let tokens = match interp.pending_def_body_tokens.take() {
        Some(tokens) => Ok(tokens),
        None => match def_val.as_vector_view() {
            // `as_vector_view` (Tensor-aware) — see control.rs's EXEC for why.
            Some(elements) => {
                crate::interpreter::value_as_code::value_elements_to_source_tokens(&elements)
                    .and_then(|tokens| {
                        check_source_radicands_within_budget(interp, &elements)?;
                        Ok(tokens)
                    })
            }
            None => Err(AjisaiError::declared(
                "invalidDefinitionBody",
                format!(
                    "expected a Vector [ ... ] definition body, got {}",
                    def_val.domain_name()
                ),
            )),
        },
    };
    let outcome = tokens.and_then(|tokens| op_def_inner(interp, &name_str, &tokens));
    if let Err(e) = outcome {
        interp.stack.push(def_val);
        interp.stack.push(name_val);
        return Err(e);
    }
    if let Some(description) = interp
        .pending_word_descriptions
        .remove(&name_str.to_uppercase())
    {
        set_word_description(interp, &name_str, Some(description));
    }
    Ok(())
}

/// The source written for an irrational takes the root of each radicand of
/// its normal form (`m SQRT`, `value_as_code::push_source_expression`), and
/// `SQRT` reduces a radicand to its square-free part by factoring it, against
/// the run's numeric-work ceiling (`radicand_budget.rs`). The value in hand
/// was not necessarily built by that root: `MUL` makes √p·√q into √(pq) with
/// no factoring, since a product of coprime square-free radicands is
/// square-free, so the radicand can be one the ceiling cannot factor — and
/// the source would then define a Word that neither runs nor restores. So
/// `DEF` takes each root now, charging the run exactly as `SQRT` would, and
/// the ceiling that would have fired on the first call fires here instead,
/// with both operands put back.
fn check_source_radicands_within_budget(
    interp: &mut Interpreter,
    elements: &[crate::types::Value],
) -> Result<()> {
    let mut radicands = Vec::new();
    crate::interpreter::value_as_code::algebraic_radicands(elements, &mut radicands);
    radicands.retain(|monomial| !num_traits::One::is_one(monomial));
    if radicands.is_empty() {
        return Ok(());
    }
    let budget = crate::interpreter::arithmetic_meter::RadicandBudget::of(interp);
    let mut outcome = Ok(());
    for monomial in radicands {
        let radicand = crate::types::fraction::Fraction::new(monomial, num_bigint::BigInt::from(1));
        if let Err(e) = budget.sqrt(radicand) {
            outcome = Err(e);
            break;
        }
    }
    budget.settle(interp).and(outcome)
}

pub(crate) fn op_def_inner(interp: &mut Interpreter, name: &str, tokens: &[Token]) -> Result<()> {
    // Every refusal comes first, and every change to the dictionary after: a
    // definition commits whole or not at all (LANG.DICTIONARY.MUTATION). The
    // old order removed the word's previous dependency edges before the empty-
    // body and cycle checks, so a refused redefinition left the old definition
    // in place with its edges gone — and `DEL` then deleted a word it still
    // called.
    // A definition is kept as its source: the only bridge that carries a
    // value whole into a body, `op_def`'s, writes it back as the source that
    // builds it (`value_elements_to_source_tokens`), so the body every check
    // below sees, and the one the dictionary keeps, is the body a saved
    // session gets back.
    debug_assert!(
        !tokens.iter().any(|token| matches!(token, Token::Value(_))),
        "a definition body is source: no value is carried whole"
    );
    crate::tokenizer::validate_code_tokens(tokens).map_err(AjisaiError::MalformedSource)?;
    interp.check_source_numeric_literals(tokens)?;
    // A Word is reached by writing its name as one token, so a name that
    // cannot be written is not a name: `DEF` took one anyway, and the entry it
    // made could be listed, hovered and exported but never called. That splits
    // exactly what LANG.DICTIONARY.RESOLUTION joins — "the host's lookup,
    // hover, the Reference, and execution must identify the same canonical
    // entry" — so refuse it at the one moment the name is chosen. `BIND`
    // already refuses the same names, and says a binding is named like a Word;
    // this is the Word half of that sentence.
    //
    // `DEL` deliberately keeps no such check: a name saved before this rule, or
    // before a lexical rule changed under it, must stay removable.
    if !crate::tokenizer::is_symbol_token_lexeme(name) {
        return Err(AjisaiError::declared(
            "invalidName",
            format!(
                "Cannot define '{}': it is not a name. A Word is called by writing its name as one token, and this cannot be written — the definition could never be reached (LANG.DICTIONARY.RESOLUTION).",
                name
            ),
        ));
    }

    let upper_name = name.to_uppercase();

    if interp.core_vocabulary.contains_key(&upper_name) {
        return Err(AjisaiError::declared(
            "protectedWord",
            format!("Cannot redefine Core Word '{}'", upper_name),
        ));
    }

    // The other half of `BIND`'s refusal to take a Word's name. Together they
    // keep the two name spaces disjoint at every moment, so a reader never has
    // to know which of the two a name resolved through. Every live frame
    // counts, not only the one `DEF` runs in: a binding in a caller past a
    // User Word's barrier is in scope again the moment the call returns.
    if interp.binding_exists_beyond_barrier(&upper_name) {
        return Err(AjisaiError::declared("nameConflict", format!(
            "Cannot define '{}': the name is bound in a live frame. A binding and a Word may not share a name.",
            upper_name
        )));
    }

    // A referenced word is not redefinable. There is no force modifier: the
    // vocabulary has no Word that overrides this, so the refusal is final and
    // the caller's only route is to delete the dependents first. A word's own
    // self-reference does not lock it: see `collect_external_dependents`.
    if interp.user_words.contains_key(&upper_name) {
        let dependents = if interp.defer_identity_recompute {
            // In a bulk restore the reverse index is rebuilt once at the end
            // (`rebuild_dependencies`) rather than kept current per word, so
            // the bodies' own references are the ground truth here. Same
            // answer as the index gives outside a restore: a word that names
            // this one has the edge the moment both exist.
            interp
                .user_words
                .iter()
                .filter(|(referrer, def)| {
                    *referrer != &upper_name && def.text_references.contains(&upper_name)
                })
                .map(|(referrer, _)| referrer.clone())
                .collect()
        } else {
            interp.collect_external_dependents(&upper_name)
        };
        if !dependents.is_empty() {
            return Err(AjisaiError::declared(
                "definitionConflict",
                format!(
                    "Cannot redefine '{}': referenced by {}. Delete those words first.",
                    upper_name,
                    sorted_names(&dependents)
                ),
            ));
        }
    }

    if tokens.is_empty() {
        return Err(AjisaiError::declared(
            "invalidDefinitionBody",
            "expected a non-empty definition body, got an empty body",
        ));
    }

    // The edges this body draws (`body_edges`): `text_references` is what
    // the acyclicity check below walks, since it alone sees a forward
    // reference to a word that does not exist yet.
    let (new_dependencies, new_text_references) = interp.body_edges(tokens);

    // LANG.DICTIONARY.ACYCLIC: the User dictionary's reference graph is
    // acyclic — no Word may name itself, directly or through any chain of
    // other User words. Repetition is expressed only through the bounded
    // higher-order Words (`MAP`, `FILTER`, `FOLD`, `SCAN`) over an
    // already-finite Vector, never through a Word calling itself: every
    // evaluation is then structurally finite, not merely bounded by a runtime
    // step budget.
    if let Some(cycle) = interp.find_reference_cycle(&upper_name, &new_text_references) {
        return Err(AjisaiError::declared(
            "selfReferentialDefinition",
            format!(
                "Cannot define '{}': the body names itself ({})",
                upper_name,
                cycle.join(" -> ")
            ),
        ));
    }

    // The work below walks the whole dictionary; it is paid for here, the
    // last refusal, so a refused definition changes nothing.
    crate::interpreter::collection_meter::charge_dictionary_change(interp, tokens.len())?;

    // Nothing below refuses.

    if let Some(warning) =
        crate::interpreter::naming_convention_checker::check_word_name_convention(name)
    {
        interp.output_buffer.push_str(&format!("{}\n", warning));
    }

    // A redefinition drops the edges of the body it replaces.
    if let Some(existing) = interp.user_words.get(&upper_name) {
        for dep_name in &existing.dependencies {
            if let Some(dependents) = interp.dependents.get_mut(dep_name) {
                dependents.remove(&upper_name);
            }
        }
    }

    // Content store: share one stored body across textually identical
    // definitions so copying a word group does not duplicate its code. A
    // stored body is shared only when it is these very tokens: the key is a
    // digest, and a body carrying a value whole keys it by a value digest
    // that does not separate every pair of algebraic values.
    let body_key = crate::interpreter::word_identity::body_content_key(tokens);
    let body: Arc<[Token]> = match interp.body_store.get(&body_key) {
        Some(shared) if shared[..] == tokens[..] => shared.clone(),
        Some(_) => tokens.into(),
        None => {
            let arc: Arc<[Token]> = tokens.into();
            interp.body_store.insert(body_key, arc.clone());
            arc
        }
    };

    for dep_name in &new_dependencies {
        interp
            .dependents
            .entry(dep_name.clone())
            .or_default()
            .insert(upper_name.clone());
    }

    let new_def = WordDefinition {
        body,
        is_builtin: false,
        description: None,
        dependencies: new_dependencies,
        text_references: new_text_references,
        registration_order: interp.next_registration_order(),
        code: Default::default(),
        // A User Word has no registry entry: `DEF` cannot define a Core Word
        // (LANG.DICTIONARY.RESOLUTION seals Core), so this is `None` by
        // construction rather than by omission.
        generated: None,
    };

    interp
        .user_words
        .insert(upper_name.clone(), Arc::new(new_def));

    // A bulk restore (`restore_user_word_definitions`) defers everything a
    // single DEF does for the dictionary as a whole: the referrer scan below
    // is a pass over every word, so running it per restored word made the
    // restore quadratic, and `rebuild_dependencies` derives every edge, every
    // identity and the epoch once at the end anyway. `recompute_word_identities`
    // and `gc_body_store` already stand down under the same flag.
    if !interp.defer_identity_recompute {
        // A name resolves at call time against the dictionary as it is then,
        // so a word written before this one and naming it calls it from now
        // on: that word depends on this one. Its `text_references` kept the
        // name while it resolved to nothing; the dependency and the reverse
        // edge are recorded now, or `DEL` would delete a word still called
        // and the caller's identity would not see what it calls
        // (LANG.DICTIONARY.MUTATION).
        let referrers: Vec<String> = interp
            .user_words
            .iter()
            .filter(|(referrer, def)| {
                *referrer != &upper_name && def.text_references.contains(&upper_name)
            })
            .map(|(referrer, _)| referrer.clone())
            .collect();
        for referrer in referrers {
            if let Some(def) = interp.user_words.get_mut(&referrer) {
                Arc::make_mut(def).dependencies.insert(upper_name.clone());
            }
            interp
                .dependents
                .entry(upper_name.clone())
                .or_default()
                .insert(referrer);
        }

        interp.recompute_word_identities();
        interp.gc_body_store();
    }
    interp
        .output_buffer
        .push_str(&format!("Defined word: {}\n", upper_name));
    interp.dictionary_changes_this_run.push(upper_name.clone());

    if !interp.defer_identity_recompute {
        interp.bump_dictionary_epoch();
    }
    Ok(())
}

/// A refusal names the words that lock a definition in one order every time.
pub(crate) fn sorted_names(names: &HashSet<String>) -> String {
    let mut names: Vec<&str> = names.iter().map(String::as_str).collect();
    names.sort_unstable();
    names.join(", ")
}

pub fn op_del(interp: &mut Interpreter) -> Result<()> {
    let val = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;

    // A refusal puts the name back as it was written (the ERROR discipline of
    // every Core Word, LANG.STACK.CONSUMPTION): the operand is consumed only
    // once the Word is gone from the dictionary.
    let outcome = delete_named(interp, &val);
    if outcome.is_err() {
        interp.stack.push(val);
    }
    outcome
}

fn delete_named(interp: &mut Interpreter, val: &Value) -> Result<()> {
    let name = extract_word_name_from_value(val)?;

    let word_name = name.to_uppercase();

    if interp.core_vocabulary.contains_key(&word_name) {
        return Err(AjisaiError::declared(
            "protectedWord",
            format!("Cannot delete Core Word '{}'", word_name),
        ));
    }

    // A name that no User Word holds is `wordNotFound`: with two tiers the
    // name is the whole address, so the question is simply whether the User
    // tier holds it.
    if !interp.user_words.contains_key(&word_name) {
        return Err(AjisaiError::declared(
            "wordNotFound",
            format!("Word '{}' is not defined", word_name),
        ));
    }

    // A referenced word is not deletable. There is no force modifier: the
    // vocabulary has no Word that overrides this, so the refusal is final and
    // the caller's only route is to delete the dependents first. A word's own
    // self-reference does not lock it: see `collect_external_dependents`.
    let dependents = interp.collect_external_dependents(&word_name);
    if !dependents.is_empty() {
        // The same dependency-graph rule DEF declares as `definitionConflict`:
        // the dictionary's existing bindings refuse this change.
        return Err(AjisaiError::declared(
            "definitionConflict",
            format!(
                "Cannot delete '{}': referenced by {}. Delete those words first.",
                word_name,
                super::execute_def::sorted_names(&dependents)
            ),
        ));
    }

    crate::interpreter::collection_meter::charge_dictionary_change(interp, 0)?;

    // The index holds exactly the edges the definitions hold (`DEF` records
    // both directions, a forward reference included once its target is
    // defined), so the word's own edges are the ones to drop. Its dependents
    // entry is empty by the check above; the words that still *name* it keep
    // the name in `text_references` and will depend on it again if it is
    // defined again.
    if let Some(removed_def) = interp.user_words.remove(&word_name) {
        for dep_name in &removed_def.dependencies {
            if let Some(deps) = interp.dependents.get_mut(dep_name) {
                deps.remove(&word_name);
            }
        }
        interp.dependents.remove(&word_name);
    }

    interp
        .output_buffer
        .push_str(&format!("Deleted word: {}\n", word_name));
    interp
        .dictionary_changes_this_run
        .push(word_name.to_string());

    interp.recompute_word_identities();
    interp.gc_body_store();
    interp.bump_dictionary_epoch();
    Ok(())
}
