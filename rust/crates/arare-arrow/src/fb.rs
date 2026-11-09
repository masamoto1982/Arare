//! The FlatBuffers subset the Arrow IPC metadata needs (AR-LLR-039, AR-LLR-041).
//!
//! [`Obj`] describes a table, a string or a vector, and [`finish`] lays it out
//! front to back: an object is written before the objects it points to, so
//! every offset points forward, as FlatBuffers' unsigned offsets require.
//! Tables start on 8-byte boundaries and every scalar sits at a multiple of
//! its size, which the reference verifiers check.
//!
//! [`Table`] reads a buffer that may be malformed: every position is checked
//! against the buffer before it is read, and a bad one is an [`Error`], never
//! a panic.

use crate::Error;

/// A scalar or an offset in a table slot.
pub enum Field {
    U8(u8),
    Bool(bool),
    I16(i16),
    I32(i32),
    I64(i64),
    Off(Obj),
}

/// An object an offset can point to.
pub enum Obj {
    /// A table: its slots in any order.
    Table(Vec<(u16, Field)>),
    Str(String),
    /// A vector of offsets to tables.
    Tables(Vec<Obj>),
    /// A vector of 8-byte-aligned structs, given as their bytes.
    Structs {
        count: u32,
        bytes: Vec<u8>,
    },
}

impl Field {
    fn size(&self) -> usize {
        match self {
            Field::U8(_) | Field::Bool(_) => 1,
            Field::I16(_) => 2,
            Field::I32(_) | Field::Off(_) => 4,
            Field::I64(_) => 8,
        }
    }
}

fn pad_to(buf: &mut Vec<u8>, align: usize) {
    while buf.len() % align != 0 {
        buf.push(0);
    }
}

fn patch(buf: &mut [u8], at: usize, target: usize) {
    let rel = (target - at) as u32;
    buf[at..at + 4].copy_from_slice(&rel.to_le_bytes());
}

fn emit(buf: &mut Vec<u8>, obj: Obj) -> usize {
    match obj {
        Obj::Str(s) => {
            pad_to(buf, 4);
            let at = buf.len();
            buf.extend_from_slice(&(s.len() as u32).to_le_bytes());
            buf.extend_from_slice(s.as_bytes());
            buf.push(0);
            at
        }
        Obj::Structs { count, bytes } => {
            while (buf.len() + 4) % 8 != 0 {
                buf.push(0);
            }
            let at = buf.len();
            buf.extend_from_slice(&count.to_le_bytes());
            buf.extend_from_slice(&bytes);
            at
        }
        Obj::Tables(items) => {
            pad_to(buf, 4);
            let at = buf.len();
            buf.extend_from_slice(&(items.len() as u32).to_le_bytes());
            let slots = buf.len();
            buf.resize(slots + 4 * items.len(), 0);
            for (i, item) in items.into_iter().enumerate() {
                let target = emit(buf, item);
                patch(buf, slots + 4 * i, target);
            }
            at
        }
        Obj::Table(mut fields) => emit_table(buf, &mut fields),
    }
}

fn emit_table(buf: &mut Vec<u8>, fields: &mut Vec<(u16, Field)>) -> usize {
    // Larger fields first: after the 4-byte vtable offset this leaves at most
    // 4 bytes of padding.
    fields.sort_by_key(|(slot, f)| (core::cmp::Reverse(f.size()), *slot));
    let mut place = Vec::with_capacity(fields.len());
    let mut size = 4usize;
    for (_, f) in fields.iter() {
        size = size.next_multiple_of(f.size());
        place.push(size);
        size += f.size();
    }
    let slots = fields
        .iter()
        .map(|(s, _)| *s as usize + 1)
        .max()
        .unwrap_or(0);
    let mut vtable = vec![0u16; 2 + slots];
    vtable[0] = (4 + 2 * slots) as u16;
    vtable[1] = size as u16;
    for ((slot, _), at) in fields.iter().zip(&place) {
        vtable[2 + *slot as usize] = *at as u16;
    }
    pad_to(buf, 2);
    let vt = buf.len();
    vtable
        .iter()
        .for_each(|v| buf.extend_from_slice(&v.to_le_bytes()));
    pad_to(buf, 8);
    let at = buf.len();
    buf.extend_from_slice(&((at - vt) as i32).to_le_bytes());
    buf.resize(at + size, 0);
    let mut children = Vec::new();
    for ((_, f), off) in core::mem::take(fields).into_iter().zip(place) {
        let p = at + off;
        match f {
            Field::U8(v) => buf[p] = v,
            Field::Bool(v) => buf[p] = v as u8,
            Field::I16(v) => buf[p..p + 2].copy_from_slice(&v.to_le_bytes()),
            Field::I32(v) => buf[p..p + 4].copy_from_slice(&v.to_le_bytes()),
            Field::I64(v) => buf[p..p + 8].copy_from_slice(&v.to_le_bytes()),
            Field::Off(o) => children.push((p, o)),
        }
    }
    for (p, o) in children {
        let target = emit(buf, o);
        patch(buf, p, target);
    }
    at
}

/// A finished buffer whose root is `root`, padded to a multiple of 8 bytes.
pub fn finish(root: Obj) -> Vec<u8> {
    let mut buf = vec![0u8; 4];
    let at = emit(&mut buf, root);
    patch(&mut buf, 0, at);
    pad_to(&mut buf, 8);
    buf
}

fn bad(what: &str) -> Error {
    Error::Malformed(format!("metadata: {what}"))
}

/// `len` bytes at `pos`, when the buffer holds them.
fn bytes(buf: &[u8], pos: usize, len: usize) -> Result<&[u8], Error> {
    pos.checked_add(len)
        .and_then(|end| buf.get(pos..end))
        .ok_or_else(|| bad("a position outside the buffer"))
}

pub fn u32_at(buf: &[u8], pos: usize) -> Result<u32, Error> {
    Ok(u32::from_le_bytes(bytes(buf, pos, 4)?.try_into().unwrap()))
}

fn follow(buf: &[u8], pos: usize) -> Result<usize, Error> {
    pos.checked_add(u32_at(buf, pos)? as usize)
        .filter(|p| *p < buf.len())
        .ok_or_else(|| bad("an offset outside the buffer"))
}

/// A table in a buffer that has not been trusted.
#[derive(Clone, Copy)]
pub struct Table<'a> {
    buf: &'a [u8],
    pos: usize,
    vt: usize,
    vt_len: usize,
    size: usize,
}

impl<'a> Table<'a> {
    /// The root table of `buf`.
    pub fn root(buf: &'a [u8]) -> Result<Table<'a>, Error> {
        Table::at(buf, follow(buf, 0)?)
    }

    fn at(buf: &'a [u8], pos: usize) -> Result<Table<'a>, Error> {
        let soff = i32::from_le_bytes(bytes(buf, pos, 4)?.try_into().unwrap()) as i64;
        let vt =
            usize::try_from(pos as i64 - soff).map_err(|_| bad("a vtable before the buffer"))?;
        let head = bytes(buf, vt, 4)?;
        let vt_len = u16::from_le_bytes([head[0], head[1]]) as usize;
        let size = u16::from_le_bytes([head[2], head[3]]) as usize;
        if vt_len < 4 || vt_len % 2 != 0 || size < 4 {
            return Err(bad("a malformed vtable"));
        }
        bytes(buf, vt, vt_len)?;
        bytes(buf, pos, size)?;
        Ok(Table {
            buf,
            pos,
            vt,
            vt_len,
            size,
        })
    }

    /// Where slot `slot`'s `len` bytes are, when the table has it.
    fn slot(&self, slot: u16, len: usize) -> Result<Option<usize>, Error> {
        let entry = 4 + 2 * slot as usize;
        if entry + 2 > self.vt_len {
            return Ok(None);
        }
        let b = bytes(self.buf, self.vt + entry, 2)?;
        let off = u16::from_le_bytes([b[0], b[1]]) as usize;
        if off == 0 {
            return Ok(None);
        }
        if off + len > self.size {
            return Err(bad("a field outside its table"));
        }
        Ok(Some(self.pos + off))
    }

    fn scalar<const N: usize>(&self, slot: u16) -> Result<Option<[u8; N]>, Error> {
        Ok(match self.slot(slot, N)? {
            Some(p) => Some(bytes(self.buf, p, N)?.try_into().unwrap()),
            None => None,
        })
    }

    pub fn u8(&self, slot: u16, default: u8) -> Result<u8, Error> {
        Ok(self.scalar::<1>(slot)?.map_or(default, |b| b[0]))
    }

    pub fn bool(&self, slot: u16, default: bool) -> Result<bool, Error> {
        Ok(self.u8(slot, default as u8)? != 0)
    }

    pub fn i16(&self, slot: u16, default: i16) -> Result<i16, Error> {
        Ok(self.scalar(slot)?.map_or(default, i16::from_le_bytes))
    }

    pub fn i32(&self, slot: u16, default: i32) -> Result<i32, Error> {
        Ok(self.scalar(slot)?.map_or(default, i32::from_le_bytes))
    }

    pub fn i64(&self, slot: u16, default: i64) -> Result<i64, Error> {
        Ok(self.scalar(slot)?.map_or(default, i64::from_le_bytes))
    }

    fn target(&self, slot: u16) -> Result<Option<usize>, Error> {
        match self.slot(slot, 4)? {
            Some(p) => Ok(Some(follow(self.buf, p)?)),
            None => Ok(None),
        }
    }

    pub fn table(&self, slot: u16) -> Result<Option<Table<'a>>, Error> {
        match self.target(slot)? {
            Some(p) => Ok(Some(Table::at(self.buf, p)?)),
            None => Ok(None),
        }
    }

    /// A vector's elements of `elem` bytes each: its first byte and count.
    fn vector(&self, slot: u16, elem: usize) -> Result<Option<(usize, usize)>, Error> {
        let Some(p) = self.target(slot)? else {
            return Ok(None);
        };
        let n = u32_at(self.buf, p)? as usize;
        let len = n
            .checked_mul(elem)
            .ok_or_else(|| bad("a vector too long"))?;
        bytes(self.buf, p + 4, len)?;
        Ok(Some((p + 4, n)))
    }

    pub fn str(&self, slot: u16) -> Result<Option<String>, Error> {
        match self.vector(slot, 1)? {
            Some((p, n)) => String::from_utf8(self.buf[p..p + n].to_vec())
                .map(Some)
                .map_err(|_| bad("a string that is not UTF-8")),
            None => Ok(None),
        }
    }

    /// A vector of structs of `elem` bytes, as their bytes.
    pub fn structs(&self, slot: u16, elem: usize) -> Result<Vec<&'a [u8]>, Error> {
        Ok(match self.vector(slot, elem)? {
            Some((p, n)) => (0..n)
                .map(|i| &self.buf[p + i * elem..p + (i + 1) * elem])
                .collect(),
            None => Vec::new(),
        })
    }

    /// A vector of tables; `None` when the slot is absent.
    pub fn tables(&self, slot: u16) -> Result<Option<Vec<Table<'a>>>, Error> {
        let Some((p, n)) = self.vector(slot, 4)? else {
            return Ok(None);
        };
        (0..n)
            .map(|i| Table::at(self.buf, follow(self.buf, p + 4 * i)?))
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
    }
}
