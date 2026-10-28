//! `JSON-DECODE` — read JSON text into values and Records
//! (LANG.VALUES.DISJOINT, LANG.RECORDS.STRUCTURE, LANG.VALUES.EXACT).
//!
//! JSON is what structured data from a host arrives as, and its two
//! containers land on the two domains built for them: an object is a Record
//! keyed by its member names in order, an array a Vector. A number becomes
//! the exact rational it spells — the JSON number grammar is a subset of the
//! language's own, and `0.1` is one tenth, never the nearest float. `null` is
//! a NIL, the absent value.
//!
//! The decoder is iterative: an explicit stack of open containers replaces
//! recursion, so nesting is bounded by the text alone rather than by the
//! host's call stack. This is also why the Word is native and not a
//! definition — nesting is input-dependent repetition, and the language
//! repeats only over a Vector that already exists. Text that is not exactly
//! one JSON value projects `invalidEncoding`, `NUM`'s reason for text that
//! spells no number; the outcome is never a guess at what was meant.
//!
//! Nesting is bounded all the same, by the machine rather than by the
//! grammar: a value nested past the nesting ceiling (`nestingDepth`,
//! LANG.MACHINE.LIMITS) is well-formed JSON the machine will not hold (its own
//! value representation walks structure recursively when it is compared,
//! rendered or released), so such text is refused as `resourceLimitExceeded`
//! by the ceiling's name, the outcome every request past a host ceiling
//! reaches, never `invalidEncoding`: the text is not wrong, it is too big
//! for this host, and that is a failure rather than a value.

use num_bigint::BigInt;
use num_traits::Zero;

use super::ordering_ops::{restore, take_operand};
use crate::error::{AjisaiError, NilReason, Result};
use crate::interpreter::collection_meter;
use crate::interpreter::Interpreter;
use crate::types::fraction::Fraction;
use crate::types::{RecordData, Value};

/// Why text did not decode.
#[derive(Debug, PartialEq, Eq)]
enum Reject {
    /// Not exactly one JSON value.
    Malformed,
    /// One JSON value, nested deeper than the decoder's `max_nesting`.
    TooDeep,
    /// One JSON value holding a number of more digits, counting its exponent,
    /// than the decoder's `max_digits` (the numeric-literal ceiling).
    TooManyDigits(u64),
    /// One JSON value whose containers hold more members, all together, than
    /// the decoder's `max_elements` (the materialization ceiling).
    TooLarge,
}

/// An open container on the decoder's own stack.
enum Frame {
    Array(Vec<Value>),
    Object {
        keys: Vec<Value>,
        values: Vec<Value>,
        pending_key: Option<Value>,
    },
}

struct Decoder<'a> {
    bytes: &'a [u8],
    pos: usize,
    /// The nesting ceiling: past it the text is declined as too large, not
    /// refused as malformed.
    max_nesting: usize,
    /// The numeric-literal ceiling, which a number in the text meets as a
    /// number in source does.
    max_digits: usize,
    /// The materialization ceiling, met by the members of every container in
    /// the text together, as `RANGE` meets it by the elements it builds.
    max_elements: usize,
    /// Members placed in a container so far.
    elements: usize,
}

impl<'a> Decoder<'a> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, expected: u8) -> Option<()> {
        (self.peek() == Some(expected)).then(|| self.pos += 1)
    }

    fn literal(&mut self, word: &[u8], value: Value) -> Option<Value> {
        if self.bytes[self.pos..].starts_with(word) {
            self.pos += word.len();
            Some(value)
        } else {
            None
        }
    }

    fn digits(&mut self) -> Option<&'a [u8]> {
        let start = self.pos;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        (self.pos > start).then(|| &self.bytes[start..self.pos])
    }

    /// `-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?`, as an exact rational.
    fn number(&mut self) -> std::result::Result<Value, Reject> {
        let negative = self.eat(b'-').is_some();
        let int_digits = self.digits().ok_or(Reject::Malformed)?;
        if int_digits.len() > 1 && int_digits[0] == b'0' {
            return Err(Reject::Malformed);
        }
        let mut mantissa = int_digits.to_vec();
        let mut scale: u32 = 0;
        if self.eat(b'.').is_some() {
            let frac = self.digits().ok_or(Reject::Malformed)?;
            scale = u32::try_from(frac.len()).map_err(|_| Reject::Malformed)?;
            mantissa.extend_from_slice(frac);
        }
        let mut magnitude: u64 = 0;
        let mut negative_exponent = false;
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            match self.peek() {
                Some(b'-') => {
                    self.pos += 1;
                    negative_exponent = true;
                }
                Some(b'+') => self.pos += 1,
                _ => {}
            }
            let exp_digits = self.digits().ok_or(Reject::Malformed)?;
            // Saturates: an exponent past `u64` is past any ceiling anyway.
            magnitude = std::str::from_utf8(exp_digits)
                .map_err(|_| Reject::Malformed)?
                .parse()
                .unwrap_or(u64::MAX);
        }
        // Checked before anything is built: `1e99999999` is eleven bytes of
        // text and a hundred million digits of integer.
        // Zero is one digit at any scale, and nothing is built for it.
        let is_zero = mantissa.iter().all(|&d| d == b'0');
        let denoted = if is_zero {
            1
        } else {
            (mantissa.len() as u64).saturating_add(magnitude)
        };
        if denoted > self.max_digits as u64 {
            return Err(Reject::TooManyDigits(denoted));
        }
        let mut numerator = BigInt::parse_bytes(&mantissa, 10).ok_or(Reject::Malformed)?;
        if negative {
            numerator = -numerator;
        }
        let mut denominator = BigInt::from(10).pow(scale);
        if numerator.is_zero() {
            // Zero at any scale; skips the (possibly enormous) power, and an
            // exponent past `u32` with it.
        } else {
            let shift = u32::try_from(magnitude).map_err(|_| Reject::Malformed)?;
            if !negative_exponent {
                numerator *= BigInt::from(10).pow(shift);
            } else {
                denominator *= BigInt::from(10).pow(shift);
            }
        }
        Ok(Value::from_fraction(Fraction::new(numerator, denominator)))
    }

    fn hex4(&mut self) -> Option<u32> {
        let slice = self.bytes.get(self.pos..self.pos + 4)?;
        let code = u32::from_str_radix(std::str::from_utf8(slice).ok()?, 16).ok()?;
        self.pos += 4;
        Some(code)
    }

    /// A JSON string, the opening quote already seen.
    fn string(&mut self) -> Option<String> {
        self.eat(b'"')?;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let byte = self.peek()?;
            self.pos += 1;
            match byte {
                b'"' => break,
                b'\\' => {
                    let escaped = self.peek()?;
                    self.pos += 1;
                    match escaped {
                        b'"' => out.push(b'"'),
                        b'\\' => out.push(b'\\'),
                        b'/' => out.push(b'/'),
                        b'b' => out.push(0x08),
                        b'f' => out.push(0x0C),
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'u' => {
                            let mut code = self.hex4()?;
                            if (0xD800..0xDC00).contains(&code) {
                                self.eat(b'\\')?;
                                self.eat(b'u')?;
                                let low = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&low) {
                                    return None;
                                }
                                code = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                            }
                            let ch = char::from_u32(code)?;
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                        _ => return None,
                    }
                }
                0x00..=0x1F => return None,
                other => out.push(other),
            }
        }
        String::from_utf8(out).ok()
    }

    /// One scalar value, or the opening of a container (pushed onto `frames`).
    fn open_value(
        &mut self,
        frames: &mut Vec<Frame>,
    ) -> std::result::Result<Option<Value>, Reject> {
        self.skip_whitespace();
        let max_nesting = self.max_nesting;
        let open = |frames: &mut Vec<Frame>, frame: Frame| {
            if frames.len() >= max_nesting {
                return Err(Reject::TooDeep);
            }
            frames.push(frame);
            Ok(None)
        };
        Ok(match self.peek().ok_or(Reject::Malformed)? {
            b'{' => {
                self.pos += 1;
                return open(
                    frames,
                    Frame::Object {
                        keys: Vec::new(),
                        values: Vec::new(),
                        pending_key: None,
                    },
                );
            }
            b'[' => {
                self.pos += 1;
                return open(frames, Frame::Array(Vec::new()));
            }
            b'"' => Some(Value::from_string(&self.string().ok_or(Reject::Malformed)?)),
            b't' => Some(
                self.literal(b"true", Value::from_bool(true))
                    .ok_or(Reject::Malformed)?,
            ),
            b'f' => Some(
                self.literal(b"false", Value::from_bool(false))
                    .ok_or(Reject::Malformed)?,
            ),
            b'n' => Some(
                self.literal(b"null", Value::nil())
                    .ok_or(Reject::Malformed)?,
            ),
            b'-' | b'0'..=b'9' => Some(self.number()?),
            _ => return Err(Reject::Malformed),
        })
    }

    /// Parse the whole text as exactly one JSON value.
    fn decode(&mut self) -> std::result::Result<Value, Reject> {
        let mut frames: Vec<Frame> = Vec::new();
        // A completed value waiting to be attached to its container, or to be
        // the answer when no container is open.
        let mut completed: Option<Value> = self.open_value(&mut frames)?;
        loop {
            if let Some(value) = completed.take() {
                let Some(frame) = frames.last_mut() else {
                    self.skip_whitespace();
                    return if self.pos == self.bytes.len() {
                        Ok(value)
                    } else {
                        Err(Reject::Malformed)
                    };
                };
                self.elements += 1;
                if self.elements > self.max_elements {
                    return Err(Reject::TooLarge);
                }
                match frame {
                    Frame::Array(items) => items.push(value),
                    Frame::Object {
                        keys,
                        values,
                        pending_key,
                    } => {
                        keys.push(pending_key.take().ok_or(Reject::Malformed)?);
                        values.push(value);
                    }
                }
                self.skip_whitespace();
                let closer = match frame {
                    Frame::Array(_) => b']',
                    Frame::Object { .. } => b'}',
                };
                match self.peek().ok_or(Reject::Malformed)? {
                    b',' => {
                        self.pos += 1;
                        completed = self.open_member(&mut frames)?;
                    }
                    byte if byte == closer => {
                        self.pos += 1;
                        completed = Some(Self::close(frames.pop().ok_or(Reject::Malformed)?)?);
                    }
                    _ => return Err(Reject::Malformed),
                }
                continue;
            }
            // A container was just opened: it is empty, or its first member
            // begins here.
            self.skip_whitespace();
            let closer = match frames.last().ok_or(Reject::Malformed)? {
                Frame::Array(_) => b']',
                Frame::Object { .. } => b'}',
            };
            if self.peek().ok_or(Reject::Malformed)? == closer {
                self.pos += 1;
                completed = Some(Self::close(frames.pop().ok_or(Reject::Malformed)?)?);
            } else {
                completed = self.open_member(&mut frames)?;
            }
        }
    }

    /// The next member of the innermost container: a key and a value for an
    /// object, a value for an array.
    fn open_member(
        &mut self,
        frames: &mut Vec<Frame>,
    ) -> std::result::Result<Option<Value>, Reject> {
        if let Some(Frame::Object { pending_key, .. }) = frames.last_mut() {
            self.skip_whitespace();
            let key = self.string().ok_or(Reject::Malformed)?;
            *pending_key = Some(Value::from_string(&key));
            self.skip_whitespace();
            self.eat(b':').ok_or(Reject::Malformed)?;
        }
        self.open_value(frames)
    }

    fn close(frame: Frame) -> std::result::Result<Value, Reject> {
        Ok(match frame {
            Frame::Array(items) => Value::from_vector(items),
            Frame::Object { keys, values, .. } => {
                Value::from_record(RecordData::new(keys, values).map_err(|_| Reject::Malformed)?)
            }
        })
    }
}

/// `JSON-DECODE ( [ text ] -> [ value ] )`: projects `invalidEncoding`.
pub(crate) fn op_json_decode(interp: &mut Interpreter) -> Result<()> {
    let operand = take_operand(interp)?;
    let Some(text) = operand.as_text().map(str::to_owned) else {
        restore(interp, operand);
        return Err(AjisaiError::declared(
            "nonText",
            "expected a String of JSON text",
        ));
    };
    // One pass over the text, charged before it runs.
    if let Err(e) = collection_meter::charge(interp, text.len() as u64 + 1) {
        restore(interp, operand);
        return Err(e);
    }
    let max_nesting = interp.runtime_limits.max_nesting_depth;
    let max_digits = interp.runtime_limits.max_numeric_literal_digits;
    let max_elements = interp.runtime_limits.max_materialized_elements;
    let mut decoder = Decoder {
        bytes: text.as_bytes(),
        pos: 0,
        max_nesting,
        max_digits,
        max_elements,
        elements: 0,
    };
    match decoder.decode() {
        Ok(value) => {
            interp.stack.push(value);
        }
        Err(Reject::Malformed) => interp
            .stack
            .push(Value::nil_with_reason(NilReason::InvalidEncoding)),
        // A ceiling crossed while reading is a refusal, never a value
        // (LANG.MACHINE.LIMITS): the text goes back and the run stops by name.
        Err(Reject::TooManyDigits(digits)) => {
            restore(interp, operand);
            return Err(
                crate::interpreter::ceiling_refusal::numeric_literal_refused(max_digits, digits),
            );
        }
        Err(Reject::TooDeep) => {
            restore(interp, operand);
            return Err(crate::interpreter::ceiling_refusal::nesting_refused(
                max_nesting,
                max_nesting + 1,
            ));
        }
        // Counted as it is built, so the whole count is not known; one past
        // the ceiling is, as `TooDeep` reports one level past it.
        Err(Reject::TooLarge) => {
            restore(interp, operand);
            return Err(
                crate::interpreter::ceiling_refusal::materialization_refused(
                    max_elements,
                    Some(max_elements as u128 + 1),
                ),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "json_decode_tests.rs"]
mod tests;
