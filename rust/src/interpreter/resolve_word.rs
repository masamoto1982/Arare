use crate::types::{Token, WordDefinition};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use super::Interpreter;

impl Interpreter {
    /// The definition an **already canonical** name resolves to, against the
    /// dictionary LANG.DICTIONARY.RESOLUTION describes: "The dictionary has two
    /// tiers. **Core** holds the canonical Words and is sealed ... **User**
    /// holds definitions made by `DEF`. Resolution is a deterministic function
    /// of the normalized name and the current dictionary, and User never shadows
    /// Core." And: "Those two tiers are the whole dictionary." Core is probed
    /// first because of that last clause, and that is the whole of resolution.
    ///
    /// It used to be more than two. A `user_dictionaries` map held named user
    /// dictionaries; an `active_user_dictionary` decided which one `DEF` wrote
    /// to; an `owning_dictionary_context` gave a word's own dictionary priority;
    /// and a bare name fell through three stages, collapsing cross-dictionary
    /// matches by content identity and reporting an `Ambiguous` outcome when
    /// they disagreed. `DICT@WORD`, `USER@D@WORD` and `DICT@USER@D@WORD` paths
    /// addressed those tiers. None of it was reachable from the language: no
    /// Word changes the active dictionary, so every `DEF` wrote to the same one
    /// ("EXAMPLE"), and `user_words` was already maintained as a flat mirror of
    /// it. The tiers were structure without a way to observe them, and the
    /// clause says there are two.
    ///
    /// No name comes back, and that is the point: the resolved name is the
    /// canonical name the caller passed in (gated by
    /// `a_resolution_answers_with_the_canonical_name`), so returning one means
    /// handing the caller a copy of what it already holds. `resolve_short_name`
    /// used to do exactly that, and to `to_uppercase` a name already uppercase
    /// to build it. This is what a word dispatch calls, once per element of a
    /// `MAP`.
    pub(crate) fn definition_of(&self, canonical_name: &str) -> Option<Arc<WordDefinition>> {
        if let Some(def) = self.core_vocabulary.get(canonical_name) {
            return Some(def.clone());
        }
        self.user_words.get(canonical_name).cloned()
    }

    /// Resolve a name to the Word it names, and the canonical name it resolved
    /// under.
    ///
    /// There used to be two of these, and a cache between them. The caching one
    /// consulted a `HashMap<String, ResolveCacheEntry>` keyed by the canonical
    /// name, and on a hit went on to look the definition up in the vocabulary
    /// anyway — because the entry stored a resolved *name*, and the definition
    /// had to come from the live dictionary or a redefinition could be served
    /// from under it. So the cache memoized one hashmap probe behind another
    /// hashmap probe, which is not a saving; and it could not memoize the thing
    /// that would have been one. The engine's call path keeps such a cache now
    /// (`Interpreter::user_word`): no definition is replaced behind the
    /// dictionary epoch any more, so an `Arc` cached under the epoch is the
    /// definition in force.
    ///
    /// What is left is the lookup the cache was in front of. `resolve_short_name`
    /// is one probe of Core and then one of User, and
    /// LANG.DICTIONARY.RESOLUTION makes that the whole of resolution: "a name
    /// resolves in Core or in User", deterministically in the current
    /// dictionary. There is nothing in that to remember.
    ///
    /// The name comes back as `Arc<str>` so a dispatch can share it rather than
    /// copy it; the callers that need an owned `String` (the call stack, a
    /// failure record, a recursion-limit report) run per *User* Word call rather
    /// than per dispatch and ask for one there.
    pub(crate) fn resolve_word_entry(&self, name: &str) -> Option<(Arc<str>, Arc<WordDefinition>)> {
        let canonical_name = crate::word_name::canonical_word_name(name);
        let def = self.definition_of(canonical_name.as_ref())?;
        Some((Arc::from(canonical_name.as_ref()), def))
    }

    pub(crate) fn resolve_word(&self, name: &str) -> Option<Arc<WordDefinition>> {
        self.resolve_word_entry(name).map(|(_, def)| def)
    }

    /// The edges a body draws in the dictionary graph: the User Words it
    /// resolves to today (`dependencies`) and every name it holds, resolved
    /// or not (`text_references`). `DEF` records them for the word it
    /// defines and `rebuild_dependencies` derives them again for every word,
    /// so the one walk lives here rather than once in each.
    ///
    /// Every name the body holds, a Symbol inside a Record it carries whole
    /// included (`body_symbols`): one it could reach at run time is one the
    /// acyclicity check has to see. `text_references` keeps every one,
    /// resolved or not — the check needs to see a forward reference to a word
    /// that does not exist yet, which `dependencies` cannot represent. Only
    /// User Words are dependencies: Core is sealed, so nothing can invalidate
    /// a reference to it.
    pub(crate) fn body_edges(&self, tokens: &[Token]) -> (HashSet<String>, HashSet<String>) {
        let mut dependencies = HashSet::new();
        let mut text_references = HashSet::new();
        for s in crate::interpreter::body_symbols::body_symbol_names(tokens) {
            let upper_s = crate::word_name::canonical_word_name(&s);
            text_references.insert(upper_s.to_string());
            if let Some((resolved_name, resolved_def)) = self.resolve_word_entry(&upper_s) {
                if !resolved_def.is_builtin {
                    dependencies.insert(resolved_name.to_string());
                }
            }
        }
        (dependencies, text_references)
    }

    pub fn rebuild_dependencies(&mut self) -> crate::error::Result<()> {
        // A quiescent recompute point, reached after a bulk restore: every
        // edge is derived again from the bodies, and the epoch moves so no
        // cached Word or fused block from before the restore is used after it.
        self.bump_dictionary_epoch();

        self.dependents.clear();

        let all_words: Vec<(String, Arc<WordDefinition>)> = self
            .user_words
            .iter()
            .map(|(name, def)| (name.clone(), Arc::clone(def)))
            .collect();

        for (word_name, word_def) in &all_words {
            let (dependencies, text_references) = self.body_edges(&word_def.body);
            for dependency in &dependencies {
                self.dependents
                    .entry(dependency.clone())
                    .or_default()
                    .insert(word_name.clone());
            }
            if let Some(def) = self.user_words.get_mut(word_name) {
                let def = Arc::make_mut(def);
                def.dependencies = dependencies;
                def.text_references = text_references;
            }
        }

        self.recompute_word_identities();
        self.gc_body_store();
        Ok(())
    }

    /// Words that directly reference `word_name`.
    ///
    /// This reads the maintained reverse-dependency index (`self.dependents`) —
    /// an inverted index from a word to the set of words that depend on it,
    /// which `DEF`, `DEL`, and `rebuild_dependencies` keep in sync — so a
    /// redefinition or deletion that touches a word referenced across a large
    /// dictionary is a single map probe rather than a walk of every body.
    ///
    /// In debug builds a `debug_assert_eq!` cross-checks the index against the
    /// authoritative full scan (`collect_dependents_by_scan`) on every call, so
    /// any drift between the maintained index and ground truth is caught by the
    /// existing test suite at zero release-build cost.
    pub fn collect_dependents(&self, word_name: &str) -> HashSet<String> {
        let from_index = self.dependents.get(word_name).cloned().unwrap_or_default();
        debug_assert_eq!(
            from_index,
            self.collect_dependents_by_scan(word_name),
            "dependents index diverged from full scan for {}",
            word_name
        );
        from_index
    }

    /// The direct dependents of `word_name` *other than itself*.
    ///
    /// This is the set that decides whether a word may be redefined or deleted.
    /// The refusal exists to protect *other* words from losing the definition
    /// they call, and a word cannot be its own such victim: redefining it
    /// replaces the body a self-call would resolve through, and deleting it
    /// removes caller and callee together. LANG.DICTIONARY.ACYCLIC refuses a
    /// self-referential definition at `DEF`, so no self-edge exists today; the
    /// exclusion states the rule rather than relying on that.
    pub fn collect_external_dependents(&self, word_name: &str) -> HashSet<String> {
        let mut dependents = self.collect_dependents(word_name);
        dependents.remove(word_name);
        dependents
    }

    /// Authoritative full-scan computation of the direct dependents of
    /// `word_name`. This is the ground truth the maintained `dependents` index
    /// mirrors; it is retained only as the debug cross-check for
    /// `collect_dependents` and is dead-code-eliminated from the release hot
    /// path.
    fn collect_dependents_by_scan(&self, word_name: &str) -> HashSet<String> {
        let mut result = HashSet::new();
        for (name, def) in &self.user_words {
            if def.dependencies.contains(word_name) {
                result.insert(name.clone());
            }
        }
        result
    }

    /// LANG.DICTIONARY.ACYCLIC's acyclicity check: would naming `referenced` from the body
    /// of `defining` (a word not yet in `user_words`, or about to replace its
    /// current entry) close a cycle back onto `defining`?
    ///
    /// Walks `text_references` — not `dependencies` — because a forward
    /// reference to a not-yet-defined word never resolves at its own DEF time
    /// and so is invisible to `dependencies`; a later definition of that word
    /// naming `defining` back would otherwise complete an undetected mutual
    /// recursion. `text_references` records every referenced name regardless
    /// of whether it currently resolves, so the graph it forms is sound for
    /// this check even though it over-approximates the real call graph (it
    /// also contains Core word names and names that resolve to nothing,
    /// which simply appear as dead ends below).
    ///
    /// Returns the closing chain (`defining` .. `defining`) on the first
    /// cycle found, or `None` if `referenced` cannot reach `defining`.
    pub(crate) fn find_reference_cycle(
        &self,
        defining: &str,
        referenced: &HashSet<String>,
    ) -> Option<Vec<String>> {
        // Breadth-first, each name keeping only the name that first reached
        // it; the chain is rebuilt once, when the walk closes. Carrying the
        // whole path on every queued step made one DEF quadratic in the depth
        // of the chain it extends, and a program of such DEFs cubic.
        let mut reached_from: HashMap<String, String> = HashMap::new();
        let mut queue: VecDeque<String> = VecDeque::new();
        let chain_to = |reached_from: &HashMap<String, String>, last: &str| {
            let mut chain = vec![defining.to_string(), last.to_string()];
            let mut at = last;
            while let Some(from) = reached_from.get(at).filter(|from| *from != defining) {
                chain.push(from.clone());
                at = from;
            }
            chain.push(defining.to_string());
            chain.reverse();
            chain
        };

        for name in referenced {
            if name == defining {
                return Some(vec![defining.to_string(), defining.to_string()]);
            }
            if !reached_from.contains_key(name) {
                reached_from.insert(name.clone(), defining.to_string());
                queue.push_back(name.clone());
            }
        }
        while let Some(current) = queue.pop_front() {
            let Some(def) = self.user_words.get(&current) else {
                continue;
            };
            for next in &def.text_references {
                if next == defining {
                    return Some(chain_to(&reached_from, &current));
                }
                if !reached_from.contains_key(next) {
                    reached_from.insert(next.clone(), current.clone());
                    queue.push_back(next.clone());
                }
            }
        }
        None
    }

    /// Transitive closure of `collect_dependents`: every word that depends on
    /// `word_name` directly or through a chain of intermediate words, by
    /// breadth-first traversal of the reverse-dependency index. The impact set
    /// a redefinition or deletion of `word_name` can affect.
    pub fn collect_transitive_dependents(&self, word_name: &str) -> HashSet<String> {
        let mut result = HashSet::new();
        let mut queue: VecDeque<String> = self
            .dependents
            .get(word_name)
            .into_iter()
            .flatten()
            .cloned()
            .collect();
        while let Some(current) = queue.pop_front() {
            if !result.insert(current.clone()) {
                continue;
            }
            if let Some(next) = self.dependents.get(&current) {
                for dep in next {
                    if !result.contains(dep) {
                        queue.push_back(dep.clone());
                    }
                }
            }
        }
        result
    }
}
