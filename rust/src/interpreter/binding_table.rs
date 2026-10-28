//! The table one frame's local names live in (`bindings::BindingScope`).
//!
//! A frame binds a handful of names — a Word body one or two, a block one —
//! and reads them many times. A hash table answered each read by hashing the
//! name, probing a group of slots and comparing the text it found there; a
//! short list answers it by comparing the name against each entry, which for
//! a canonical name held inline (`types::Name`) is a few word compares.
//! Opening and closing a frame is also cheaper: there is no table of control
//! bytes to reset. A frame that binds more than a few names keeps an index
//! beside the list as well, so a long one still finds a name in constant
//! time.
//!
//! The table holds the same correspondence the hash table did: one value per
//! name, a rebinding replacing the value in place. Nothing reads its order.

use crate::types::{Name, Value};
use std::borrow::Borrow;
use std::hash::Hash;

/// How many names a frame binds before an index is kept beside them.
const LINEAR: usize = 8;

#[derive(Debug, Default, Clone)]
pub(crate) struct ScopeNames {
    entries: Vec<(Name, Value)>,
    /// Where each name sits in `entries`, kept only while there are more
    /// than [`LINEAR`] of them.
    index: crate::fast_hash::FastMap<Name, usize>,
}

impl ScopeNames {
    /// The position of `name` in `entries`.
    #[inline(always)]
    fn position<Q>(&self, name: &Q) -> Option<usize>
    where
        Name: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        if self.entries.len() <= LINEAR {
            self.entries
                .iter()
                .position(|(held, _)| held.borrow() == name)
        } else {
            self.index.get(name).copied()
        }
    }

    #[inline]
    pub(crate) fn get<Q>(&self, name: &Q) -> Option<&Value>
    where
        Name: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.position(name).map(|at| &self.entries[at].1)
    }

    pub(crate) fn contains_key<Q>(&self, name: &Q) -> bool
    where
        Name: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.position(name).is_some()
    }

    /// Bind `name` to `value`, replacing the value it is bound to already.
    pub(crate) fn insert(&mut self, name: Name, value: Value) {
        if let Some(at) = self.position(&name) {
            self.entries[at].1 = value;
            return;
        }
        self.entries.push((name, value));
        let len = self.entries.len();
        if len == LINEAR + 1 {
            for (at, (held, _)) in self.entries.iter().enumerate() {
                self.index.insert(held.clone(), at);
            }
        } else if len > LINEAR + 1 {
            self.index.insert(self.entries[len - 1].0.clone(), len - 1);
        }
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.entries.clear();
        self.index.clear();
    }

    pub(crate) fn capacity(&self) -> usize {
        self.entries.capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::{ScopeNames, LINEAR};
    use crate::types::{Name, Value};

    /// Past the point where the index is kept, every name is still found,
    /// a rebinding still replaces, and clearing forgets them all.
    #[test]
    fn names_are_found_on_both_sides_of_the_index() {
        let mut table = ScopeNames::default();
        let count = LINEAR * 3;
        for i in 0..count {
            table.insert(Name::from(format!("N{i}")), Value::from_int(i as i64));
        }
        table.insert(Name::from("N3"), Value::from_int(-3));
        for i in 0..count {
            let expected = if i == 3 { -3 } else { i as i64 };
            assert_eq!(
                table.get(format!("N{i}").as_str()),
                Some(&Value::from_int(expected))
            );
            assert!(table.contains_key(&Name::from(format!("N{i}"))));
        }
        assert!(!table.contains_key("N999"));
        table.clear();
        assert!(!table.contains_key("N0"));
        table.insert(Name::from("A"), Value::from_int(1));
        assert_eq!(table.get("A"), Some(&Value::from_int(1)));
    }
}
