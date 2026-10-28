use super::{set_js_prop, stack_to_js, AjisaiInterpreter, UserWordData};
use crate::builtins;
use serde_wasm_bindgen::to_value;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl AjisaiInterpreter {
    #[wasm_bindgen]
    pub fn collect_stack(&self) -> JsValue {
        stack_to_js(self.interpreter.get_stack().as_slice())
    }

    #[wasm_bindgen]
    pub fn collect_user_words_info(&self) -> JsValue {
        let js_array = js_sys::Array::new();

        for name in sorted_user_word_names(&self.interpreter) {
            // Another User Word calls this one, so DEL refuses it until that
            // caller is gone; the host colours it apart.
            let has_dependents = self
                .interpreter
                .dependents
                .get(name)
                .is_some_and(|deps| !deps.is_empty());

            let item = js_sys::Array::new();
            item.push(&name.clone().into());
            item.push(&has_dependents.into());

            js_array.push(&item);
        }

        js_array.into()
    }

    /// Content identity of each user word, as `[name, id]` pairs. The host
    /// uses these to deduplicate identical definitions on import and to key
    /// shared word groups by content rather than by name.
    #[wasm_bindgen]
    pub fn collect_word_identities(&self) -> JsValue {
        let js_array = js_sys::Array::new();
        for name in sorted_user_word_names(&self.interpreter) {
            if let Some(id) = self.interpreter.word_identity(name) {
                let item = js_sys::Array::new();
                item.push(&name.clone().into());
                item.push(&id.clone().into());
                js_array.push(&item);
            }
        }
        js_array.into()
    }

    pub(crate) fn collect_user_words_for_state(&self) -> JsValue {
        let words_info: Vec<UserWordData> = sorted_user_word_names(&self.interpreter)
            .into_iter()
            .map(|name| UserWordData {
                definition: self.interpreter.lookup_word_definition_tokens(name),
                description: self.interpreter.lookup_word_description(name),
                name: name.clone(),
            })
            .collect();
        to_value(&words_info).unwrap_or(JsValue::NULL)
    }

    #[wasm_bindgen]
    pub fn collect_core_words_info(&self) -> JsValue {
        to_value(&builtins::collect_core_builtin_definitions()).unwrap_or(JsValue::NULL)
    }

    #[wasm_bindgen]
    pub fn lookup_word_definition(&self, name: &str) -> JsValue {
        let upper_name = name.to_uppercase();
        self.interpreter
            .lookup_word_definition_tokens(&upper_name)
            .map(|def| JsValue::from_str(&def))
            .unwrap_or(JsValue::NULL)
    }

    /// A User Word's `#:contract`-derived description, for a host affordance
    /// like the Dictionary panel's hover. `execute` refuses a source whose
    /// declaration inference disproves, as `check` does.
    #[wasm_bindgen]
    pub fn lookup_word_description(&self, name: &str) -> JsValue {
        let upper_name = name.to_uppercase();
        self.interpreter
            .lookup_word_description(&upper_name)
            .map(|desc| JsValue::from_str(&desc))
            .unwrap_or(JsValue::NULL)
    }

    /// Answer the host's lookup of `name` against the current dictionary.
    ///
    /// This is a *query*, not a run. Looking a Word up used to be the Word
    /// `LOOKUP`, which meant asking what `ADD` does went through `execute` and
    /// came back on a side channel that no evaluation rule read. The host asks
    /// here instead, so nothing about a lookup touches the stack, the
    /// dictionary, or the output buffer.
    ///
    /// Returns `{ kind: "documentation" | "definition", text }`, or `NULL` for a
    /// name the dictionary does not hold — the caller reports the unknown name
    /// itself, since it is the one that read it off the input.
    #[wasm_bindgen]
    pub fn resolve_host_lookup(&self, name: &str) -> JsValue {
        use crate::interpreter::host_lookup::{resolve_host_lookup, HostLookup};

        let (kind, text) = match resolve_host_lookup(&self.interpreter, name) {
            Ok(HostLookup::Documentation(text)) => ("documentation", text),
            Ok(HostLookup::Definition(text)) => ("definition", text),
            Err(_) => return JsValue::NULL,
        };

        let obj = js_sys::Object::new();
        set_js_prop(&obj, "kind", &JsValue::from_str(kind));
        set_js_prop(&obj, "text", &JsValue::from_str(&text));
        obj.into()
    }

    /// Discard every value on the stack, leaving the dictionary, the output
    /// and every other piece of session state untouched.
    ///
    /// A REPL keeps its stack between runs, which is right, and until now the
    /// only way to get rid of a leftover intermediate was the full reset — and
    /// that takes the User dictionary with it. Clearing values is not a
    /// language operation (no Word does it, and none should: a program's own
    /// values are its own business), so it belongs here, on the host, where the
    /// person at the keyboard is the one asking.
    #[wasm_bindgen]
    pub fn clear_stack(&mut self) {
        self.interpreter.update_stack(crate::types::Stack::new());
    }

    /// The one stack format persistence accepts (LANG.OBSERVATION.FIREWALL). Unlike
    /// `collect_stack`, which serializes the *observation* wire format (a
    /// CodeBlock shows as `nil`, an ExactScalar as a marked rational
    /// approximation), this captures the exact value so `restore_stack_snapshot`
    /// returns identical values. The two surfaces are deliberately distinct:
    /// observation is lossy-but-honest, persistence is lossless. Restoring the
    /// observation format is not offered — it would silently downgrade exact
    /// values. The payload is an opaque JSON string produced by
    /// `crate::types::value_persist`.
    #[wasm_bindgen]
    pub fn snapshot_stack(&self) -> String {
        crate::types::value_persist::encode_stack(self.interpreter.get_stack().iter())
    }

    /// Restore a stack from a `snapshot_stack` payload, reinstating exact
    /// values (CodeBlock, ExactScalar, …).
    #[wasm_bindgen]
    pub fn restore_stack_snapshot(&mut self, snapshot_json: &str) -> Result<(), String> {
        let stack = crate::types::value_persist::decode_stack(snapshot_json)?;
        self.interpreter.update_stack(stack);
        Ok(())
    }

    /// Override the execution step budget (water level, LANG.MACHINE.LIMITS) for
    /// subsequent executions. A runtime safety control, not a language
    /// semantic: the host may raise or lower it; never calling this keeps the
    /// interpreter's own `DEFAULT_MAX_EXECUTION_STEPS`. A zero or non-positive
    /// value is ignored so a malformed host call cannot disable the safety
    /// budget entirely.
    ///
    /// The default's *value* is deliberately not restated here. It is derived
    /// from the host time budget and has already moved once (it was 100,000);
    /// every doc comment that spelled the number out went on claiming the old
    /// one, on both sides of the wasm boundary. The constant is that number's
    /// single representation, so this names it instead.
    #[wasm_bindgen]
    pub fn set_max_execution_steps(&mut self, steps: usize) {
        if steps > 0 {
            self.interpreter.set_max_execution_steps(steps);
        }
    }

    /// Restore saved User Words, and name the entries that could not be
    /// restored as `[name, reason]` pairs.
    ///
    /// Restoring skips an unreadable entry rather than raising, which is what
    /// keeps the rest of a dictionary (`restore_user_word_definitions`); the
    /// skipped entries come back here instead of being thrown, since a throw
    /// would abort the host's own post-restore work and leave the session
    /// holding a half-restored dictionary. The host used to learn only the
    /// *names* that did not arrive, by comparing what it asked for against
    /// the dictionary afterwards — which could not see a refused
    /// redefinition (the old body is still there, so the name is present) and
    /// could not say why anything was left out. The `Err` case is a list that
    /// does not deserialize at all.
    #[wasm_bindgen]
    pub fn restore_user_words(&mut self, words_js: JsValue) -> Result<JsValue, String> {
        let words: Vec<UserWordData> = serde_wasm_bindgen::from_value(words_js)
            .map_err(|e| format!("Failed to deserialize words: {}", e))?;

        // A saved entry from before the dictionary became two tiers may still
        // carry a `dictionary` label; it is ignored like any unknown field, and
        // the definition lands among the User Words with every other.
        let entries = words.into_iter().map(|word| {
            (
                word.name,
                word.definition.unwrap_or_default(),
                word.description,
            )
        });

        let skipped = self
            .interpreter
            .restore_user_word_definitions(entries)
            .map_err(|e| e.to_string())?;

        let _ = self.interpreter.collect_output();

        let js_array = js_sys::Array::new();
        for entry in skipped {
            let item = js_sys::Array::new();
            item.push(&entry.name.into());
            item.push(&entry.reason.into());
            js_array.push(&item);
        }
        Ok(js_array.into())
    }
}

/// The user dictionary's Word names, sorted, the order every dictionary view lists them in.
fn sorted_user_word_names(interp: &crate::interpreter::Interpreter) -> Vec<&String> {
    let mut names: Vec<&String> = interp.user_words.keys().collect();
    names.sort();
    names
}
