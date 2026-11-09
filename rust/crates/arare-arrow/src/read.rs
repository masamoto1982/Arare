//! Reading the Arrow IPC stream and file formats (AR-LLR-039, AR-LLR-040,
//! AR-LLR-041).
//!
//! The input is untrusted: every length and offset is checked before it is
//! used, and anything outside ADR-0014's subset is refused, naming the field.
//! A file is read through its footer; a stream message by message.

use crate::columns::{Column, NumColumn, EXPONENT, ONE_OVER_ZERO, WIDTH, ZERO_OVER_ZERO};
use crate::decimal::{code_of, decimal_scale, unbase64, Signed};
use crate::fb::{u32_at, Table};
use crate::ipc::{
    CONTINUATION, HEADER_RECORD_BATCH, HEADER_SCHEMA, MAGIC, TYPE_BOOL, TYPE_DECIMAL,
};
use crate::{Batch, Error};
use arare_kernel::{fit, Num, Truth, Val, Width};

const TYPE_INT: u8 = 2;

/// How to read what Arare did not write (ADR-0004: a scale is written where
/// a value enters).
#[derive(Clone, Copy, Default, Debug)]
pub struct ReadOptions {
    /// The width and exponent a Decimal field without Arare's metadata is
    /// quantized at; such a field is refused without it.
    pub decimal: Option<(Width, i32)>,
}

fn bad(m: impl Into<String>) -> Error {
    Error::Malformed(m.into())
}

enum Kind {
    Bool,
    Int(u32),
    Decimal { scale: i32, bytes: usize },
}

struct FieldDef {
    name: String,
    kind: Kind,
    meta: Vec<(String, String)>,
}

impl FieldDef {
    fn meta(&self, key: &str) -> Option<&str> {
        self.meta
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

fn unsupported(name: &str, what: &str) -> Error {
    Error::Unsupported(format!("field {name}: {what}"))
}

fn field_def(t: &Table) -> Result<FieldDef, Error> {
    let name = t.str(0)?.unwrap_or_default();
    if t.table(4)?.is_some() {
        return Err(unsupported(&name, "dictionary encoding"));
    }
    if t.tables(5)?.is_some_and(|c| !c.is_empty()) {
        return Err(unsupported(&name, "a nested type"));
    }
    let ty = t.table(3)?;
    let kind = match (t.u8(2, 0)?, ty) {
        (TYPE_BOOL, _) => Kind::Bool,
        (TYPE_INT, Some(ty)) => match (ty.i32(0, 0)?, ty.bool(1, false)?) {
            (b @ (8 | 16 | 32 | 64), true) => Kind::Int(b as u32),
            _ => return Err(unsupported(&name, "an unsigned or odd-width integer")),
        },
        (TYPE_DECIMAL, Some(ty)) => match ty.i32(2, 128)? {
            b @ (128 | 256) => Kind::Decimal {
                scale: ty.i32(1, 0)?,
                bytes: b as usize / 8,
            },
            _ => return Err(unsupported(&name, "a decimal of neither 128 nor 256 bits")),
        },
        (n, _) => return Err(unsupported(&name, &format!("Arrow type {n}"))),
    };
    let mut meta = Vec::new();
    for kv in t.tables(6)?.unwrap_or_default() {
        meta.push((
            kv.str(0)?.unwrap_or_default(),
            kv.str(1)?.unwrap_or_default(),
        ));
    }
    Ok(FieldDef { name, kind, meta })
}

fn schema(t: &Table) -> Result<Vec<FieldDef>, Error> {
    if t.i16(0, 0)? != 0 {
        return Err(Error::Unsupported("big-endian data".into()));
    }
    t.tables(1)?
        .unwrap_or_default()
        .iter()
        .map(field_def)
        .collect()
}

fn i64_le(b: &[u8], at: usize) -> i64 {
    i64::from_le_bytes(b[at..at + 8].try_into().unwrap())
}

fn bit(bytes: &[u8], i: usize) -> bool {
    bytes[i / 8] >> (i % 8) & 1 == 1
}

/// One field's lanes from a record batch: its validity (when it has nulls)
/// and its value bytes.
fn buffer<'a>(body: &'a [u8], spec: &[u8], need: usize, name: &str) -> Result<&'a [u8], Error> {
    let (off, len) = (i64_le(spec, 0), i64_le(spec, 8));
    let (off, len) = (usize::try_from(off), usize::try_from(len));
    let (Ok(off), Ok(len)) = (off, len) else {
        return Err(bad(format!("field {name}: a negative buffer")));
    };
    if len < need {
        return Err(bad(format!(
            "field {name}: a buffer shorter than its lanes"
        )));
    }
    off.checked_add(len)
        .and_then(|end| body.get(off..end))
        .ok_or_else(|| bad(format!("field {name}: a buffer outside the body")))
}

fn column(
    f: &FieldDef,
    len: usize,
    valid: Option<&[u8]>,
    data: &[u8],
    opts: ReadOptions,
) -> Result<Column, Error> {
    let present = |i: usize| valid.is_none_or(|v| bit(v, i));
    match f.kind {
        Kind::Bool => Ok(Column::Truth(
            (0..len)
                .map(|i| match (present(i), bit(data, i)) {
                    (false, _) => Truth::Unknown,
                    (true, true) => Truth::True,
                    (true, false) => Truth::False,
                })
                .collect(),
        )),
        Kind::Int(bits) => {
            let (w, n) = (
                Width::ALL[bits.trailing_zeros() as usize - 3],
                bits as usize / 8,
            );
            let lanes = (0..len)
                .map(|i| {
                    present(i).then(|| {
                        let mut le = [0u8; 8];
                        le[..n].copy_from_slice(&data[i * n..(i + 1) * n]);
                        let shift = 64 - bits;
                        fit(w, ((i64::from_le_bytes(le) << shift) >> shift) as i128)
                    })
                })
                .collect();
            Ok(Column::Num(NumColumn {
                width: w,
                scale: 0,
                lanes,
            }))
        }
        Kind::Decimal { scale, bytes } => {
            let value = |i: usize| Signed::from_le(&data[i * bytes..(i + 1) * bytes]);
            match (f.meta(WIDTH), f.meta(EXPONENT)) {
                (Some(w), Some(e)) => own_decimal(f, w, e, scale, len, &present, value),
                _ => foreign_decimal(f, scale, len, &present, value, opts),
            }
        }
    }
}

/// A Decimal field Arare wrote: every code recovered exactly.
fn own_decimal(
    f: &FieldDef,
    w: &str,
    e: &str,
    scale: i32,
    len: usize,
    present: &dyn Fn(usize) -> bool,
    value: impl Fn(usize) -> Signed,
) -> Result<Column, Error> {
    let not_ours = |what: &str| unsupported(&f.name, &format!("Arare's metadata, but {what}"));
    let width = Width::ALL
        .into_iter()
        .find(|x| w == x.bits().to_string())
        .ok_or_else(|| not_ours("an unknown width"))?;
    let e: i32 = e.parse().map_err(|_| not_ours("an unreadable exponent"))?;
    if Num::new(Val::Fin(0), width, e).is_none() || decimal_scale(e) != scale {
        return Err(not_ours("an exponent its decimal scale does not match"));
    }
    let points = |key: &str| -> Result<Option<Vec<u8>>, Error> {
        match f.meta(key) {
            None => Ok(None),
            Some(t) => match unbase64(t) {
                Some(b) if b.len() * 8 >= len => Ok(Some(b)),
                _ => Err(not_ours("an unreadable bitmap of wheel points")),
            },
        }
    };
    let (inf, bot) = (points(ONE_OVER_ZERO)?, points(ZERO_OVER_ZERO)?);
    let mut lanes = Vec::with_capacity(len);
    for i in 0..len {
        let at = |b: &Option<Vec<u8>>| b.as_ref().is_some_and(|b| bit(b, i));
        lanes.push(match (present(i), at(&inf), at(&bot)) {
            (true, false, false) => match code_of(value(i), e) {
                Some(k) if k.unsigned_abs() <= width.max_finite() as u64 => Some(Val::Fin(k)),
                _ => return Err(not_ours("a lane no code of its width and exponent gives")),
            },
            (false, false, false) => None,
            (false, true, false) => Some(Val::Inf),
            (false, false, true) => Some(Val::Bot),
            _ => return Err(not_ours("a wheel point in a lane that is not null")),
        });
    }
    Ok(Column::Num(NumColumn {
        width,
        scale: e,
        lanes,
    }))
}

/// A Decimal field from elsewhere, quantized as QUANTIZE does (AR-HLR-026).
fn foreign_decimal(
    f: &FieldDef,
    scale: i32,
    len: usize,
    present: &dyn Fn(usize) -> bool,
    value: impl Fn(usize) -> Signed,
    opts: ReadOptions,
) -> Result<Column, Error> {
    let Some((width, e)) = opts.decimal else {
        return Err(unsupported(
            &f.name,
            "a decimal without a width and exponent to read it at",
        ));
    };
    let too_big = || unsupported(&f.name, "a decimal lane beyond 63 bits after reduction");
    let lanes = (0..len)
        .map(|i| {
            if !present(i) {
                return Ok(None);
            }
            // value / 10^scale, or value × 10^−scale, reduced by 2s and 5s.
            let x = value(i);
            let (mut p, mut q) = (x.mag, crate::decimal::U256::from_u128(1));
            for _ in 0..scale.unsigned_abs() {
                if scale > 0 {
                    q = q.mul_small(10).ok_or_else(too_big)?;
                } else {
                    p = p.mul_small(10).ok_or_else(too_big)?;
                }
            }
            for d in [2, 5] {
                while !p.is_zero() && p.divrem_small(d).1 == 0 && q.divrem_small(d).1 == 0 {
                    p = p.divrem_small(d).0;
                    q = q.divrem_small(d).0;
                }
            }
            let small = |u: crate::decimal::U256| u.to_u128().and_then(|v| i64::try_from(v).ok());
            let (p, q) = (small(p).ok_or_else(too_big)?, small(q).ok_or_else(too_big)?);
            let p = if x.negative { -p } else { p };
            let n = Num::from_ratio(p, q, width, e)
                .map_err(|_| unsupported(&f.name, "an exponent outside ±31"))?;
            Ok(Some(n.val()))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(Column::Num(NumColumn {
        width,
        scale: e,
        lanes,
    }))
}

/// Appends a record batch's lanes to `out`, one column per field.
fn record_batch(
    rb: &Table,
    body: &[u8],
    fields: &[FieldDef],
    out: &mut [Vec<Column>],
    opts: ReadOptions,
) -> Result<(), Error> {
    if rb.table(3)?.is_some() {
        return Err(Error::Unsupported("compressed record batches".into()));
    }
    let nodes = rb.structs(1, 16)?;
    let buffers = rb.structs(2, 16)?;
    if nodes.len() != fields.len() || buffers.len() != 2 * fields.len() {
        return Err(bad(
            "a record batch whose nodes or buffers do not match its schema",
        ));
    }
    for (i, f) in fields.iter().enumerate() {
        let len = usize::try_from(i64_le(nodes[i], 0)).map_err(|_| bad("a negative length"))?;
        let nulls = i64_le(nodes[i], 8);
        let width = match f.kind {
            Kind::Bool => 0,
            Kind::Int(b) => b as usize / 8,
            Kind::Decimal { bytes, .. } => bytes,
        };
        let need = if width == 0 {
            len.div_ceil(8)
        } else {
            len.checked_mul(width)
                .ok_or_else(|| bad("a length too large"))?
        };
        let valid = match nulls {
            0 => None,
            _ => Some(buffer(body, buffers[2 * i], len.div_ceil(8), &f.name)?),
        };
        let data = buffer(body, buffers[2 * i + 1], need, &f.name)?;
        out[i].push(column(f, len, valid, data, opts)?);
    }
    Ok(())
}

/// A message at `at`: its metadata table and where its body starts.
fn message(bytes: &[u8], at: usize) -> Result<Option<(Table<'_>, usize)>, Error> {
    let first = bytes
        .get(at..at + 4)
        .ok_or_else(|| bad("a message cut short"))?;
    let (len_at, start) = if first == CONTINUATION {
        (at + 4, at + 8)
    } else {
        (at, at + 4)
    };
    let len = u32_at(bytes, len_at)? as usize;
    if len == 0 {
        return Ok(None);
    }
    let meta = start
        .checked_add(len)
        .and_then(|end| bytes.get(start..end))
        .ok_or_else(|| bad("message metadata cut short"))?;
    let t = Table::root(meta)?;
    if t.i16(0, 0)? < 3 {
        return Err(Error::Unsupported("IPC metadata before version 4".into()));
    }
    Ok(Some((t, start + len)))
}

fn body_of(bytes: &[u8], at: usize, len: i64) -> Result<&[u8], Error> {
    usize::try_from(len)
        .ok()
        .and_then(|n| at.checked_add(n))
        .and_then(|end| bytes.get(at..end))
        .ok_or_else(|| bad("a message body cut short"))
}

fn concat(fields: Vec<FieldDef>, parts: Vec<Vec<Column>>) -> Batch {
    let columns = fields
        .into_iter()
        .zip(parts)
        .map(|(f, cs)| {
            let mut it = cs.into_iter();
            let first = it.next();
            let joined = it.fold(first, |acc, c| match (acc, c) {
                (Some(Column::Truth(mut a)), Column::Truth(b)) => {
                    a.extend(b);
                    Some(Column::Truth(a))
                }
                (Some(Column::Num(mut a)), Column::Num(b)) => {
                    a.lanes.extend(b.lanes);
                    Some(Column::Num(a))
                }
                (acc, _) => acc,
            });
            let empty = match f.kind {
                Kind::Bool => Column::Truth(Vec::new()),
                _ => Column::Num(NumColumn {
                    width: Width::W8,
                    scale: 0,
                    lanes: Vec::new(),
                }),
            };
            (f.name, joined.unwrap_or(empty))
        })
        .collect();
    Batch { columns }
}

/// Reads an Arrow IPC file or stream (AR-HLR-038).
pub fn read(bytes: &[u8], opts: ReadOptions) -> Result<Batch, Error> {
    if bytes.starts_with(MAGIC) {
        return read_file(bytes, opts);
    }
    let Some((t, mut at)) = message(bytes, 0)? else {
        return Err(bad("a stream without a schema"));
    };
    let schema_t = match (t.u8(1, 0)?, t.table(2)?) {
        (HEADER_SCHEMA, Some(s)) => s,
        _ => return Err(bad("a stream that does not start with its schema")),
    };
    let fields = schema(&schema_t)?;
    let mut parts: Vec<Vec<Column>> = fields.iter().map(|_| Vec::new()).collect();
    at = at
        .checked_add(body_of(bytes, at, t.i64(3, 0)?)?.len())
        .unwrap();
    while at < bytes.len() {
        let Some((t, start)) = message(bytes, at)? else {
            break;
        };
        let body = body_of(bytes, start, t.i64(3, 0)?)?;
        match (t.u8(1, 0)?, t.table(2)?) {
            (HEADER_RECORD_BATCH, Some(rb)) => record_batch(&rb, body, &fields, &mut parts, opts)?,
            (HEADER_RECORD_BATCH, None) => {
                return Err(bad("a record batch message without its header"))
            }
            (n, _) => return Err(Error::Unsupported(format!("IPC message type {n}"))),
        }
        at = start + body.len();
    }
    Ok(concat(fields, parts))
}

fn read_file(bytes: &[u8], opts: ReadOptions) -> Result<Batch, Error> {
    let n = bytes.len();
    if n < 18 || !bytes.ends_with(MAGIC) {
        return Err(bad("a file without its closing magic"));
    }
    let flen = u32_at(bytes, n - 10)? as usize;
    let start = (n - 10)
        .checked_sub(flen)
        .filter(|s| *s >= 8)
        .ok_or_else(|| bad("a footer longer than the file"))?;
    let footer = Table::root(&bytes[start..n - 10])?;
    let schema_t = footer
        .table(1)?
        .ok_or_else(|| bad("a footer without a schema"))?;
    let fields = schema(&schema_t)?;
    if !footer.structs(2, 24)?.is_empty() {
        return Err(Error::Unsupported("dictionaries".into()));
    }
    let mut parts: Vec<Vec<Column>> = fields.iter().map(|_| Vec::new()).collect();
    for block in footer.structs(3, 24)? {
        let at = usize::try_from(i64_le(block, 0)).map_err(|_| bad("a negative block offset"))?;
        let meta_len = i32::from_le_bytes(block[8..12].try_into().unwrap());
        let meta_len = usize::try_from(meta_len).map_err(|_| bad("a negative metadata length"))?;
        let Some((t, _)) = message(bytes, at)? else {
            return Err(bad("a block that points at the end of the stream"));
        };
        let body = body_of(
            bytes,
            at.checked_add(meta_len)
                .ok_or_else(|| bad("a block too long"))?,
            i64_le(block, 16),
        )?;
        match (t.u8(1, 0)?, t.table(2)?) {
            (HEADER_RECORD_BATCH, Some(rb)) => record_batch(&rb, body, &fields, &mut parts, opts)?,
            _ => return Err(bad("a block that is not a record batch")),
        }
    }
    Ok(concat(fields, parts))
}
