use crate::error::{AjisaiError, Result};
use crate::interpreter::{HostEffect, Interpreter};
use crate::types::Value;
use std::fmt::Write;

pub fn op_print(interp: &mut Interpreter) -> Result<()> {
    interp.run_effect_schema(|interp| {
        let val: Value = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;
        // Rendering walks the whole value, charged before it runs; a refusal
        // puts the operand back.
        if let Err(e) = crate::interpreter::collection_meter::charge_walk_of(interp, &val) {
            interp.stack.push(val);
            return Err(e);
        }
        // PRINT is an output boundary: a String is emitted as its raw
        // character content, without the `'...'` quotes the Stack projection
        // uses to mark it as a string (LANG.EFFECTS.OUTPUT).
        let payload = crate::types::display::format_for_output(&val);
        // One PRINT, one line. LANG.EFFECTS.OUTPUT makes the output stream an
        // *ordered sequence of emissions* and leaves rendering to the host, and
        // the structured `HostEffect::Print` channel below already carries one
        // item per call — this buffer is the host-readable rendering of that
        // same sequence. Joining the items with a space instead collapsed every
        // emission into one long line, so nine `PRINT`s of a Pascal's triangle
        // row produced a single row of numbers and the language had no way at
        // all to produce multi-line output. The native CLI has always rendered
        // one payload per line; this brings the buffer that the browser host
        // reads in line with it.
        // `fmt::Write` for a String is infallible; the Result is the trait's.
        writeln!(&mut interp.output_buffer, "{}", payload)
            .expect("writing to a String cannot fail");
        Ok(HostEffect::Print(payload))
    })
}

#[cfg(test)]
mod tests {
    use crate::interpreter::Interpreter;

    /// A string is shown on the stack as `'TEST'` but printed as its raw
    /// content `TEST`: the surrounding quotes are a Stack affordance only
    /// (LANG.EFFECTS.OUTPUT).
    #[tokio::test]
    async fn test_print_strips_display_quotes_from_string() {
        let mut interp = Interpreter::new();
        interp.execute("'TEST' PRINT").await.unwrap();
        let output = interp.collect_output();
        assert_eq!(output.trim(), "TEST", "unexpected output: {:?}", output);
    }

    /// Quote characters that are part of the string content survive: the
    /// content `T'ES'T` (shown as `'T'ES'T'` on the stack) prints unchanged.
    #[tokio::test]
    async fn test_print_keeps_content_quote_characters() {
        let mut interp = Interpreter::new();
        interp.execute("'T'ES'T' PRINT").await.unwrap();
        let output = interp.collect_output();
        assert_eq!(output.trim(), "T'ES'T", "unexpected output: {:?}", output);
    }

    /// A host that reads the text buffer run by run drains the structured
    /// effect log with it: the second run's log holds the second run's
    /// emissions only, and nothing of the first run's stays behind.
    #[tokio::test]
    async fn test_take_host_effects_drains_the_log_per_run() {
        use crate::interpreter::HostEffect;
        let mut interp = Interpreter::new();
        interp.execute("1 PRINT").await.unwrap();
        let _ = interp.collect_output();
        assert_eq!(
            interp.take_host_effects(),
            vec![HostEffect::Print("1/1".to_string())]
        );
        assert!(interp.host_effects().is_empty());

        interp.execute("2 PRINT").await.unwrap();
        assert_eq!(interp.collect_output(), "2/1\n");
        assert_eq!(
            interp.take_host_effects(),
            vec![HostEffect::Print("2/1".to_string())]
        );
    }

    /// Non-text values print exactly as they render on the stack.
    #[tokio::test]
    async fn test_print_numbers_and_booleans_unchanged() {
        let mut interp = Interpreter::new();
        interp.execute("[ 42 ] PRINT").await.unwrap();
        assert_eq!(interp.collect_output().trim(), "[ 42/1 ]");

        interp.execute("TRUE PRINT").await.unwrap();
        assert_eq!(interp.collect_output().trim(), "TRUE");
    }

    /// A string nested inside a collection stays a string: printing a vector
    /// of strings shows them quoted, never as their codepoint fractions. Only
    /// the outer `'...'` of a top-level string is a display affordance.
    #[tokio::test]
    async fn test_print_vector_of_strings_keeps_them_as_strings() {
        let mut interp = Interpreter::new();
        interp.execute("[ 'AB' 'CD' ] PRINT").await.unwrap();
        assert_eq!(interp.collect_output().trim(), "[ 'AB' 'CD' ]");
    }

    /// A mixed collection renders each element by its domain: strings
    /// quoted, numbers as fractions.
    #[tokio::test]
    async fn test_print_mixed_vector_renders_each_domain() {
        let mut interp = Interpreter::new();
        interp.execute("[ 'mix' 42 ] PRINT").await.unwrap();
        assert_eq!(interp.collect_output().trim(), "[ 'mix' 42/1 ]");
    }
}
