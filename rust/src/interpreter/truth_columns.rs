//! Where a truth column may live (`types::truth_column`).
//!
//! A truth column is a representation of a flat Vector of truth values, and
//! only a few Words read it as it is: `AND`, `NOT` and `SELECT`, which work
//! on its bytes, and `BIND`, which stores its subject untouched. Every other
//! Word is handed the Vector the column stands for, so no Word's law, error
//! or charge can tell the two apart (LANG.AUTHORITY.FREEDOM). The column is
//! unpacked on the stack just before such a Word runs — and only when one
//! may be there, which `packed_truths` records: a Word whose answer is a
//! column leaves it on top, and a binding read that pushes one says so.

use crate::interpreter::Interpreter;
use crate::kernel::generated::WordId;
use crate::types::ValueData;

impl Interpreter {
    /// Unpack every truth column on the stack that the Word `id` would not
    /// read as one.
    #[inline]
    pub(crate) fn unpack_truth_columns_for(&mut self, id: WordId) {
        if !self.packed_truths.get() {
            return;
        }
        match id {
            WordId::And | WordId::Not | WordId::Select => {}
            // The subject is stored as it is; the names must be read as the
            // Vector they would be.
            WordId::Bind => self.unpack_from(self.stack.len().saturating_sub(1)),
            _ => {
                self.unpack_from(0);
                self.packed_truths.set(false);
            }
        }
    }

    /// Unpack the truth columns at or above slot `from`.
    fn unpack_from(&mut self, from: usize) {
        for slot in from..self.stack.len() {
            if let ValueData::Truths(column) = &self.stack[slot].data {
                let vector = column.to_vector();
                self.stack[slot] = vector;
            }
        }
    }

    /// Record a truth column a Word left on top of the stack.
    #[inline]
    pub(crate) fn note_truth_column_on_top(&self) {
        if matches!(
            self.stack.last().map(|v| &v.data),
            Some(ValueData::Truths(_))
        ) {
            self.packed_truths.set(true);
        }
    }
}
