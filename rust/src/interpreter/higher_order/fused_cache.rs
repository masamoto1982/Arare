//! The fused lowering of a code block, kept on the block for its next walk.

use std::sync::Arc;

use super::ExecutableCode;
use crate::interpreter::fused_block::FusedBlock;
use crate::interpreter::Interpreter;

/// What a fused lowering read besides the block: the operand count it was
/// lowered for, the call depth it was bounded from, and the dictionary.
pub(super) type FusedKey = (usize, usize, u64);

impl ExecutableCode {
    /// The block lowered for a fused walk that starts it on `inputs` values,
    /// when it is inside the fused subset.
    ///
    /// A block walked again — the inner `FOLD` of `[ [ 0 ] [ ADD ] FOLD ]
    /// MAP`, once per row — is lowered once: a lowering that depends only on
    /// the block, the operand count, the call depth and the dictionary is
    /// kept and handed out again while those are unchanged. One that read a
    /// binding from outside the block depends on more, and is lowered afresh
    /// every time.
    pub(crate) fn fused(&self, interp: &Interpreter, inputs: usize) -> Option<Arc<FusedBlock>> {
        let key = (inputs, interp.call_depth, interp.dictionary_epoch);
        let mut kept = self
            .fused
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some((kept_key, block)) = kept.as_ref() {
            if *kept_key == key {
                return Some(block.clone());
            }
        }
        let block = Arc::new(FusedBlock::compile(&self.code, interp, inputs)?);
        if !block.reads_outer {
            *kept = Some((key, block.clone()));
        }
        Some(block)
    }
}
