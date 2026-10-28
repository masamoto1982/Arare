//! The execution stack.
//!
//! A slot holds a value and nothing else: every observation of a slot is
//! derived from the value it holds (LANG.VALUES.DENOTATION), so there is no
//! per-slot state beside it for a Word to set or a surface to read.
//!
//! Reads flow through `Deref` to `Vec<Value>`. Mutation goes through the
//! inherent methods below; there is deliberately no `DerefMut`.

use super::Value;
use std::ops::{Deref, Index, IndexMut, RangeBounds};

/// The interpreter's working stack.
///
/// Besides its values it remembers the lowest slot written since the last
/// [`Stack::take_fresh_start`], so a check over the values a Word just
/// produced (the nesting ceiling of LANG.MACHINE.LIMITS) reads only those,
/// never the whole stack. That mark is bookkeeping, not state a slot has:
/// equality compares the values alone.
#[derive(Debug, Clone, Default)]
pub struct Stack {
    values: Vec<Value>,
    fresh_from: usize,
}

impl PartialEq for Stack {
    fn eq(&self, other: &Self) -> bool {
        self.values == other.values
    }
}

impl Stack {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_values(values: Vec<Value>) -> Self {
        Self {
            values,
            fresh_from: 0,
        }
    }

    /// Note that slot `index` may hold a value written since the last check.
    #[inline]
    fn mark_fresh(&mut self, index: usize) {
        self.fresh_from = self.fresh_from.min(index);
    }

    /// The first slot written since the last check — every slot from there
    /// to the top holds a value no check has seen yet — and forget it.
    #[inline]
    pub fn take_fresh_start(&mut self) -> usize {
        let start = self.fresh_from.min(self.values.len());
        self.fresh_from = self.values.len();
        start
    }

    #[inline]
    pub fn push(&mut self, value: Value) {
        self.mark_fresh(self.values.len());
        self.values.push(value);
    }

    #[inline]
    pub fn pop(&mut self) -> Option<Value> {
        self.values.pop()
    }

    #[inline]
    pub fn truncate(&mut self, len: usize) {
        self.values.truncate(len);
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }

    pub fn insert(&mut self, index: usize, value: Value) {
        self.mark_fresh(index);
        self.values.insert(index, value);
    }

    pub fn remove(&mut self, index: usize) -> Value {
        // The values above `index` move down one slot, a fresh one among them.
        self.mark_fresh(index);
        self.values.remove(index)
    }

    pub fn split_off(&mut self, at: usize) -> Stack {
        Stack::from_values(self.values.split_off(at))
    }

    pub fn extend<I: IntoIterator<Item = Value>>(&mut self, iter: I) {
        self.mark_fresh(self.values.len());
        self.values.extend(iter);
    }

    /// Drain a range of values. Mirrors `Vec::drain`.
    pub fn drain<R>(&mut self, range: R) -> std::vec::Drain<'_, Value>
    where
        R: RangeBounds<usize>,
    {
        // Values above the range move down into it, a fresh one among them.
        let start = match range.start_bound() {
            std::ops::Bound::Included(&i) => i,
            std::ops::Bound::Excluded(&i) => i + 1,
            std::ops::Bound::Unbounded => 0,
        };
        self.mark_fresh(start);
        self.values.drain(range)
    }

    /// The values as a slice. Equivalent to dereferencing to `&[Value]`, but
    /// spelled out for range-indexing call sites (`stack.as_slice()[a..b]`),
    /// which the inherent `Index<usize>` would otherwise shadow.
    #[inline]
    pub fn as_slice(&self) -> &[Value] {
        &self.values
    }

    /// Consume the stack into its values.
    pub fn into_values(self) -> Vec<Value> {
        self.values
    }
}

impl Deref for Stack {
    type Target = Vec<Value>;
    fn deref(&self) -> &Vec<Value> {
        &self.values
    }
}

impl Index<usize> for Stack {
    type Output = Value;
    fn index(&self, index: usize) -> &Value {
        &self.values[index]
    }
}

// In-place value mutation.
impl IndexMut<usize> for Stack {
    fn index_mut(&mut self, index: usize) -> &mut Value {
        self.mark_fresh(index);
        &mut self.values[index]
    }
}

impl IntoIterator for Stack {
    type Item = Value;
    type IntoIter = std::vec::IntoIter<Value>;
    fn into_iter(self) -> Self::IntoIter {
        self.values.into_iter()
    }
}

impl<'a> IntoIterator for &'a Stack {
    type Item = &'a Value;
    type IntoIter = std::slice::Iter<'a, Value>;
    fn into_iter(self) -> Self::IntoIter {
        self.values.iter()
    }
}

impl From<Vec<Value>> for Stack {
    fn from(values: Vec<Value>) -> Self {
        Stack::from_values(values)
    }
}

impl FromIterator<Value> for Stack {
    fn from_iter<I: IntoIterator<Item = Value>>(iter: I) -> Self {
        Stack::from_values(iter.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bulk_mutation_keeps_values_in_order() {
        let mut stack = Stack::from_values(vec![Value::from_int(1), Value::from_int(2)]);
        stack.extend(vec![Value::from_int(3), Value::from_int(4)]);
        let tail = stack.split_off(1);
        assert_eq!(stack.len(), 1);
        assert_eq!(tail.len(), 3);
        let drained: Vec<Value> = stack.drain(..).collect();
        assert_eq!(drained, vec![Value::from_int(1)]);
        assert!(stack.is_empty());
    }

    #[test]
    fn fresh_start_covers_every_slot_written_since_the_last_take() {
        let mut stack = Stack::from_values(vec![Value::from_int(1), Value::from_int(2)]);
        assert_eq!(stack.take_fresh_start(), 0, "a new stack is all fresh");
        assert_eq!(stack.take_fresh_start(), 2, "nothing written since");
        stack.push(Value::from_int(3));
        assert_eq!(stack.take_fresh_start(), 2);
        // A value pushed and then moved down by a removal below it is still
        // found: the removal marks where the values above it landed.
        stack.push(Value::from_int(4));
        stack.remove(0);
        assert_eq!(stack.take_fresh_start(), 0);
        stack.push(Value::from_int(5));
        stack.drain(1..2);
        assert_eq!(stack.take_fresh_start(), 1);
        stack[0] = Value::from_int(6);
        assert_eq!(stack.take_fresh_start(), 0);
        stack.pop();
        assert_eq!(stack.take_fresh_start(), stack.len());
    }
}
