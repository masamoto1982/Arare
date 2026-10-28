//! Local bindings — naming a value for the rest of the frame that made it.
//!
//! A stack gives a chain, and an expression worth writing down is usually a
//! DAG. A residual feeds both the squared error and the gradient; a mask feeds
//! both a count and a weighted count; a threshold feeds both the comparison and
//! the report. Every one of those needs a value more than once, and the stack
//! answers only for the value on top.
//!
//! Duplicating a value sounds like enough and is not. Copy a value, derive
//! something from the upper copy, and the derived value lands *on* the lower
//! copy — the second use is now buried under the first result and cannot be
//! reached. Three copies do not help: consuming the first buries the second.
//! The workaround is to pack the values into a matrix and recover them with a
//! weighted fold, so `a SUB b` is written as
//! `CP DLS MUL 2 MUL 2 COLLECT [ [ -1 ] [ 1 ] ] MUL 0 { ADD } FOLD`. That is not a
//! program anyone can read, and it is what the third test report was reduced to.
//!
//! So a value gets a name. What makes this a dictionary operation rather than a
//! second name space is where the name lives and how long: a binding belongs to
//! the frame that made it, LANG.SOURCE.FRAME's frame and no other, and it is
//! gone when that frame is. Nothing outside can see it, so no Word's meaning
//! depends on its caller, and content identity is untouched because a binding
//! name is text in the body like every other name.

use crate::error::{AjisaiError, Result};
use crate::types::{Name, Value};
use std::sync::Arc;

use super::Interpreter;

/// One frame's local names.
///
/// `barrier` marks a scope that lookup does not search past: the frame of a
/// User Word call, or of the whole run. A block a Core Word evaluates —
/// `MAP`'s element block, `FOLD`'s, `EXEC`'s block — opens a
/// transparent scope instead, so it reads the names of the frame it was
/// written in. That difference is the whole scoping rule: a name reaches into
/// the blocks written beneath it and never into a Word called from it.
#[derive(Debug, Default, Clone)]
pub(crate) struct BindingScope {
    names: super::binding_table::ScopeNames,
    barrier: bool,
    /// The names the frame's code binds with `'NAME' BIND`, each held in a
    /// slot of `Interpreter::locals` from `base` on (`engine`): read there by
    /// the code itself, and by name by everything that looks the name up.
    slots: Arc<[Name]>,
    base: usize,
}

impl BindingScope {
    pub(crate) fn root() -> Self {
        Self {
            barrier: true,
            ..Self::default()
        }
    }

    /// The value the slot named `name` holds, when one does.
    #[inline]
    fn slot<'a, Q>(&self, locals: &'a [Option<Value>], name: &Q) -> Option<&'a Value>
    where
        Name: std::borrow::Borrow<Q>,
        Q: Eq + ?Sized,
    {
        let at = self
            .slots
            .iter()
            .position(|held| std::borrow::Borrow::<Q>::borrow(held) == name)?;
        locals[self.base + at].as_ref()
    }
}

impl Interpreter {
    /// Open a frame for code that binds `slots` in slots of its own;
    /// `barrier` when the frame is a Word call or the run itself. The
    /// matching [`Interpreter::close_binding_scope`] pops what this pushed.
    #[inline(always)]
    pub(crate) fn open_frame(&mut self, barrier: bool, slots: &Arc<[Name]>) {
        let base = self.locals.len();
        for _ in 0..slots.len() {
            self.locals.push(None);
        }
        match self.binding_scopes.get_mut(self.binding_depth) {
            Some(scope) => {
                scope.barrier = barrier;
                scope.base = base;
                if !Arc::ptr_eq(&scope.slots, slots) {
                    scope.slots = slots.clone();
                }
            }
            None => self.binding_scopes.push(BindingScope {
                names: Default::default(),
                barrier,
                slots: slots.clone(),
                base,
            }),
        }
        self.binding_depth += 1;
    }

    /// Give the current frame `slots`, for a program's own code.
    pub(crate) fn set_frame_slots(&mut self, slots: Arc<[Name]>) {
        let depth = self.binding_depth - 1;
        let base = self.locals.len();
        self.locals.resize(base + slots.len(), None);
        let scope = &mut self.binding_scopes[depth];
        scope.base = base;
        scope.slots = slots;
    }

    /// Give the current frame more slots, `slots` being all it has now.
    pub(crate) fn grow_frame_slots(&mut self, slots: Arc<[Name]>) {
        let scope = &mut self.binding_scopes[self.binding_depth - 1];
        self.locals.resize_with(scope.base + slots.len(), || None);
        scope.slots = slots;
    }

    /// Slot `slot` of the current frame.
    #[inline]
    pub(crate) fn local(&self, slot: u32) -> Option<&Value> {
        let scope = &self.binding_scopes[self.binding_depth - 1];
        self.locals[scope.base + slot as usize].as_ref()
    }

    /// Set slot `slot` of the current frame.
    #[inline]
    pub(crate) fn set_local(&mut self, slot: u32, value: Value) {
        let at = self.binding_scopes[self.binding_depth - 1].base + slot as usize;
        self.locals[at] = Some(value);
    }

    /// Close the innermost scope. Its emptied table stays where it was, for
    /// the next scope opened at that depth: a block a higher-order Word
    /// applies opens and closes one per element, and a Word body one per
    /// call, and a `BIND` in either would otherwise allocate a fresh table
    /// every time — or, as when emptied tables were kept aside, move one in
    /// and out of a list of spares. Only small tables near the root are
    /// kept; any other is let go of.
    #[inline(always)]
    pub(crate) fn close_binding_scope(&mut self) {
        let Some(depth) = self.binding_depth.checked_sub(1) else {
            return;
        };
        self.binding_depth = depth;
        self.locals.truncate(self.binding_scopes[depth].base);
        let names = &mut self.binding_scopes[depth].names;
        if depth < 16 && names.capacity() <= 64 {
            names.clear();
        } else {
            *names = Default::default();
        }
    }

    /// The open scopes, outermost first.
    #[inline]
    fn open_scopes(&self) -> &[BindingScope] {
        &self.binding_scopes[..self.binding_depth]
    }

    /// Discard every scope and open a fresh root. A run's bindings are the
    /// run's, so nothing survives into the next one — a binding that outlived
    /// its run would be a second, mutable name space competing with the
    /// dictionary, which is the thing this design exists not to be.
    pub(crate) fn reset_binding_scopes(&mut self) {
        self.binding_scopes.clear();
        self.binding_scopes.push(BindingScope::root());
        self.binding_depth = 1;
        self.locals.clear();
    }

    /// The value bound to `name` in the frames this one can see. `name` is a
    /// canonical name's text, or the canonical [`Name`] itself, which the
    /// engine's ops hold and which compares without reading it as text.
    #[inline]
    pub(crate) fn lookup_binding<Q>(&self, name: &Q) -> Option<Value>
    where
        Name: std::borrow::Borrow<Q>,
        Q: std::hash::Hash + Eq + ?Sized,
    {
        for scope in self.open_scopes().iter().rev() {
            if let Some(bound) = scope
                .slot(&self.locals, name)
                .or_else(|| scope.names.get(name))
            {
                // A truth column read onto the stack must be seen there
                // (`truth_columns`).
                if matches!(bound.data, crate::types::ValueData::Truths(_)) {
                    self.packed_truths.set(true);
                }
                return Some(bound.clone());
            }
            if scope.barrier {
                break;
            }
        }
        None
    }

    /// Whether `name` is bound anywhere at all, including past a barrier. Used
    /// to tell a reader why a name they can see in the source does not
    /// resolve where they used it, and by `DEF` to keep a Word from taking a
    /// name a caller's frame still holds.
    pub(crate) fn binding_exists_beyond_barrier(&self, name: &str) -> bool {
        self.open_scopes()
            .iter()
            .any(|scope| scope.names.contains_key(name) || scope.slot(&self.locals, name).is_some())
    }

    /// Bind `name` in the innermost scope, replacing any binding it already
    /// holds there.
    ///
    /// Rebinding is deliberate. A block a higher-order Word applies opens a
    /// scope per evaluation, so a `BIND` written inside one is a fresh name
    /// every element rather than a collision on the second; but a body that
    /// binds the same scratch name twice is ordinary enough that refusing it
    /// would buy nothing. This is a local, not a definition: the rule that
    /// protects a referenced Word from being redefined is about names other
    /// code can reach, and nothing outside this frame can reach this one.
    pub(crate) fn bind_local(&mut self, name: Name, value: Value) {
        if let Some(depth) = self.binding_depth.checked_sub(1) {
            let scope = &mut self.binding_scopes[depth];
            match scope.slots.iter().position(|held| *held == name) {
                Some(at) => self.locals[scope.base + at] = Some(value),
                None => scope.names.insert(name, value),
            }
        }
    }

    /// Reject a name a binding may not take.
    ///
    /// A binding is looked up before the dictionary, so a binding that took a
    /// Word's name would shadow it — and LANG.DICTIONARY.RESOLUTION makes Core
    /// unshadowable and resolution a function of the name and the dictionary
    /// alone. Refusing the overlap outright keeps that true and spares a reader
    /// the question of which one they got: within a program, a name is a Word
    /// or a binding and never both.
    pub(crate) fn check_bindable_name(&self, name: &str) -> Result<()> {
        // The name check mirrors `op_def_inner`'s, which declares it
        // `invalidName`; a Core or User Word's name is `nameConflict` below.
        if !crate::tokenizer::is_symbol_token_lexeme(name) {
            return Err(AjisaiError::declared(
                "invalidName",
                format!(
                    "Cannot bind '{}': it is not a name. A binding is looked up by writing its name as one token, and this cannot be written.",
                    name
                ),
            ));
        }
        let upper = crate::word_name::canonical_word_name(name);
        if self.core_vocabulary.contains_key(upper.as_ref()) {
            return Err(AjisaiError::declared(
                "nameConflict",
                format!(
                    "Cannot bind '{}': it is a Core Word, and a binding may not shadow one.",
                    upper
                ),
            ));
        }
        if self.user_words.contains_key(upper.as_ref()) {
            return Err(AjisaiError::declared(
                "nameConflict",
                format!(
                    "Cannot bind '{}': it is a User Word. Delete it first, or bind another name.",
                    upper
                ),
            ));
        }
        Ok(())
    }
}

/// `BIND ( x [ 'NAME' ] -> )`, and `BIND ( [ a b ] [ 'A' 'B' ] -> )`.
///
/// One name takes the whole value. Several names destructure a Vector of the
/// same length, position by position — the generalization `GET` makes for
/// reading, made once more for naming, because a state vector that arrives as
/// one value and wants to be read as its parts is the shape most of this comes
/// in.
///
/// Both operands are consumed and nothing is pushed. That is the point: the
/// value leaves the stack and becomes reachable by name for the rest of the
/// frame, however many times and in whatever order, which is exactly what a
/// stack cannot offer and what a DAG-shaped expression needs.
pub(crate) fn op_bind(interp: &mut Interpreter) -> Result<()> {
    if interp.stack.len() < 2 {
        return Err(AjisaiError::stack_underflow());
    }

    let name_value = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;
    // One String name — what nearly every `BIND` is given — is checked and
    // bound from its own text: the list of owned copies the general route
    // builds is two allocations made only to be read once and freed.
    if let crate::types::ValueData::Text(text) = &name_value.data {
        let text = Arc::clone(text);
        return bind_one(interp, name_value, text);
    }
    let names = match binding_names(&name_value).and_then(|names| {
        for name in &names {
            interp.check_bindable_name(name)?;
        }
        Ok(names)
    }) {
        Ok(names) => names,
        Err(error) => {
            interp.stack.push(name_value);
            return Err(error);
        }
    };

    let subject = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;

    if let Err(error) = check_binding_acyclic(interp, &names, &subject) {
        interp.stack.push(subject);
        interp.stack.push(name_value);
        return Err(error);
    }

    match names.as_slice() {
        [name] => {
            // The name, uppercased, as the key (a Text name was bound above).
            let key: Arc<str> = match crate::word_name::canonical_word_name(name) {
                std::borrow::Cow::Owned(folded) => folded.into(),
                std::borrow::Cow::Borrowed(name) => name.into(),
            };
            interp.bind_local(key.into(), subject);
        }
        several => {
            // Destructuring is exact. A Vector longer than the name list would
            // otherwise drop its tail silently, and a shorter one would bind a
            // name to nothing; both are the caller having said something
            // different from what they meant. A non-Vector is never
            // destructured, not even by an empty name list.
            let is_vector = subject.is_vector();
            let width = if is_vector { subject.len() } else { 0 };
            if !is_vector || width != several.len() {
                interp.stack.push(subject);
                interp.stack.push(name_value);
                // `shapeMismatch`, with a message about destructuring rather
                // than the broadcast wording of `AjisaiError::shape_mismatch`,
                // which would describe a failure BIND never performs.
                let what = if is_vector {
                    format!("a Vector of {} elements", width)
                } else {
                    "a non-Vector value".to_string()
                };
                return Err(AjisaiError::declared(
                    "shapeMismatch",
                    format!("{} names cannot destructure {}", several.len(), what),
                ));
            }
            for (position, name) in several.iter().enumerate() {
                let part = subject
                    .child(position)
                    .expect("the element count was checked against the name count");
                interp.bind_local(name.to_uppercase().into(), part);
            }
        }
    }
    Ok(())
}

/// `op_bind` for one String name: the same checks, in the same order, with
/// the same operands restored on a refusal, as the general route makes for a
/// one-name list.
fn bind_one(interp: &mut Interpreter, name_value: Value, text: Arc<str>) -> Result<()> {
    if let Err(error) = interp.check_bindable_name(&text) {
        interp.stack.push(name_value);
        return Err(error);
    }
    let subject = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;
    if let Err(error) = check_binding_acyclic(interp, &[&*text], &subject) {
        interp.stack.push(subject);
        interp.stack.push(name_value);
        return Err(error);
    }
    // The key is the name uppercased: the text in hand already when it has
    // no lowercase letter to fold.
    let key = match crate::word_name::canonical_word_name(&text) {
        std::borrow::Cow::Owned(folded) => Name::from(folded),
        std::borrow::Cow::Borrowed(_) => Name::from(text),
    };
    interp.bind_local(key, subject);
    Ok(())
}

/// Refuse a binding whose value reaches its own name.
///
/// A binding is looked up before the dictionary, and a Symbol inside a bound
/// Vector resolves when that Vector runs, not when it is bound. So
/// `[ F EXEC ] 'F' BIND F EXEC` would call itself with no Word in between, and
/// the DEF-time acyclicity check (LANG.DICTIONARY.ACYCLIC) never sees it: it
/// reads definitions, and a binding is not one. The same rule is applied here,
/// at the same moment — when the name is given — over the same graph: the
/// Symbols the value holds, followed through the bindings this frame can see
/// (a User Word's body cannot see them, so the walk stops at the dictionary).
/// The names being bound together count as bound already, so destructuring
/// cannot close a cycle between its own parts either.
fn check_binding_acyclic<S: AsRef<str>>(
    interp: &Interpreter,
    names: &[S],
    subject: &Value,
) -> Result<()> {
    use super::body_symbols::value_symbol_names;
    use crate::types::ValueData;
    use std::collections::HashSet;
    // Only a Symbol names anything, and only a Symbol, a Vector or a Record
    // can hold one (`value_symbol_names`): a number, a truth value, a String
    // or a Tensor binds with nothing to follow.
    if !matches!(
        subject.data,
        ValueData::Symbol(_) | ValueData::Vector(_) | ValueData::Record(_)
    ) {
        return Ok(());
    }
    let pending: Vec<(String, Value)> = match names {
        [only] => vec![(only.as_ref().to_uppercase(), subject.clone())],
        several => several
            .iter()
            .enumerate()
            .filter_map(|(position, name)| {
                Some((name.as_ref().to_uppercase(), subject.child(position)?))
            })
            .collect(),
    };
    let bound_value = |name: &str| -> Option<Value> {
        match pending
            .iter()
            .find(|(pending_name, _)| pending_name == name)
        {
            Some((_, value)) => Some(value.clone()),
            None => interp.lookup_binding(name),
        }
    };
    for (target, value) in &pending {
        let mut visited: HashSet<String> = HashSet::new();
        let mut frontier = Vec::new();
        value_symbol_names(value, &mut frontier);
        while let Some(symbol) = frontier.pop() {
            let symbol = symbol.to_uppercase();
            if symbol == *target {
                return Err(AjisaiError::declared(
                    "selfReferentialDefinition",
                    format!(
                        "Cannot bind '{}': the value names '{}' itself, directly or through other bindings, so running it would never end.",
                        target, target
                    ),
                ));
            }
            if !visited.insert(symbol.clone()) {
                continue;
            }
            if let Some(next) = bound_value(&symbol) {
                value_symbol_names(&next, &mut frontier);
            }
        }
    }
    Ok(())
}

/// The names an operand gives. A String is one name; a Vector of Strings is
/// that many, and a one-element Vector is the single-name case, so `[ 'W' ]`
/// and `'W'` mean the same thing.
pub(crate) fn binding_names(value: &Value) -> Result<Vec<String>> {
    // A String's name is its text.
    if let crate::types::ValueData::Text(text) = &value.data {
        return Ok(vec![String::from(&**text)]);
    }
    let Some(children) = value.as_vector() else {
        return Err(AjisaiError::declared(
            "nonText",
            format!(
                "expected a name (String) or a Vector of names, got {}",
                value.domain_name()
            ),
        ));
    };
    children
        .iter()
        .map(|child| {
            if let crate::types::ValueData::Text(text) = &child.data {
                Ok(String::from(&**text))
            } else {
                Err(AjisaiError::declared(
                    "nonText",
                    format!(
                        "expected each name to be a String, got {}",
                        child.domain_name()
                    ),
                ))
            }
        })
        .collect()
}
