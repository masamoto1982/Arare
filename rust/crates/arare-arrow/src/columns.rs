//! Columns and how each is laid out in Arrow (AR-LLR-040).

use crate::decimal::{base64, bit_width, decimal_scale, precision, unscaled};
use arare_kernel::{Truth, TruthColumn, Val, Width};

/// A column of numbers of one width and exponent; `None` is a NIL lane.
#[derive(Clone, PartialEq, Debug)]
pub struct NumColumn {
    pub width: Width,
    pub scale: i32,
    pub lanes: Vec<Option<Val>>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Column {
    /// Truth values; UNKNOWN is a NIL lane.
    Truth(Vec<Truth>),
    Num(NumColumn),
}

impl Column {
    pub fn len(&self) -> usize {
        match self {
            Column::Truth(lanes) => lanes.len(),
            Column::Num(c) => c.lanes.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The nulls Arrow counts: NIL lanes, and for numbers the wheel points.
    pub fn null_count(&self) -> usize {
        match self {
            Column::Truth(lanes) => lanes.iter().filter(|t| **t == Truth::Unknown).count(),
            Column::Num(c) => c
                .lanes
                .iter()
                .filter(|l| !matches!(l, Some(Val::Fin(_))))
                .count(),
        }
    }
}

/// Arrow's bitmap of `bits`: lane i at bit i % 8 of byte i / 8, least
/// significant bit first, padded with zero bits to `pad` bytes.
pub fn bitmap(bits: impl Iterator<Item = bool>, len: usize, pad: usize) -> Vec<u8> {
    let mut out = vec![0u8; len.div_ceil(8).next_multiple_of(pad.max(1))];
    for (i, b) in bits.enumerate() {
        if b {
            out[i / 8] |= 1 << (i % 8);
        }
    }
    out
}

/// What a column writes: its Arrow type, its field metadata and its two
/// buffers (validity, then values).
pub struct Layout {
    pub decimal: Option<(i32, i32, i32)>,
    pub metadata: Vec<(String, String)>,
    pub validity: Vec<u8>,
    pub values: Vec<u8>,
}

/// Metadata keys (ADR-0014).
pub const WIDTH: &str = "arare:width";
pub const EXPONENT: &str = "arare:exponent";
pub const ONE_OVER_ZERO: &str = "arare:one-over-zero";
pub const ZERO_OVER_ZERO: &str = "arare:zero-over-zero";

pub fn layout(column: &Column) -> Layout {
    match column {
        // AR-LLR-009's bitmaps, written as they are.
        Column::Truth(lanes) => {
            let t = TruthColumn::from_lanes(lanes);
            Layout {
                decimal: None,
                metadata: Vec::new(),
                validity: t.validity_bytes(),
                values: t.values_bytes(),
            }
        }
        Column::Num(c) => {
            let p = precision(c.width, c.scale);
            let bytes = bit_width(p) as usize / 8;
            let mut values = Vec::with_capacity((c.lanes.len() * bytes).next_multiple_of(64));
            for lane in &c.lanes {
                let code = match lane {
                    Some(Val::Fin(k)) => *k,
                    _ => 0,
                };
                values.extend(unscaled(code, c.scale).to_le(bytes));
            }
            values.resize(values.len().next_multiple_of(64), 0);
            let mut metadata = vec![
                (WIDTH.to_string(), c.width.bits().to_string()),
                (EXPONENT.to_string(), c.scale.to_string()),
            ];
            for (key, point) in [(ONE_OVER_ZERO, Val::Inf), (ZERO_OVER_ZERO, Val::Bot)] {
                if c.lanes.contains(&Some(point)) {
                    let bits = c.lanes.iter().map(|l| *l == Some(point));
                    metadata.push((key.to_string(), base64(&bitmap(bits, c.lanes.len(), 1))));
                }
            }
            Layout {
                decimal: Some((p as i32, decimal_scale(c.scale), bit_width(p))),
                metadata,
                validity: bitmap(
                    c.lanes.iter().map(|l| matches!(l, Some(Val::Fin(_)))),
                    c.lanes.len(),
                    64,
                ),
                values,
            }
        }
    }
}
