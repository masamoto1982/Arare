//! Writing the Arrow IPC stream and file formats (AR-LLR-039).
//!
//! A message is the continuation marker `0xFFFFFFFF`, the metadata length,
//! a FlatBuffers `Message` padded to 8 bytes, and the body. The stream is the
//! schema message, one record batch and the end-of-stream marker; the file
//! is `ARROW1`, two bytes of padding, that stream, the `Footer`, its length
//! and `ARROW1` again. Buffers in the body start on 64-byte boundaries.

use crate::columns::{layout, Column, Layout};
use crate::fb::{finish, Field, Obj};
use crate::{Batch, Error};

pub const MAGIC: &[u8; 6] = b"ARROW1";
pub const CONTINUATION: [u8; 4] = [0xff; 4];
/// `MetadataVersion.V5`.
pub const V5: i16 = 4;
pub const HEADER_SCHEMA: u8 = 1;
pub const HEADER_RECORD_BATCH: u8 = 3;
pub const TYPE_BOOL: u8 = 6;
pub const TYPE_DECIMAL: u8 = 7;

fn key_values(pairs: &[(String, String)]) -> Obj {
    Obj::Tables(
        pairs
            .iter()
            .map(|(k, v)| {
                Obj::Table(vec![
                    (0, Field::Off(Obj::Str(k.clone()))),
                    (1, Field::Off(Obj::Str(v.clone()))),
                ])
            })
            .collect(),
    )
}

fn field(name: &str, l: &Layout) -> Obj {
    let (type_type, ty) = match l.decimal {
        None => (TYPE_BOOL, Obj::Table(Vec::new())),
        Some((precision, scale, bits)) => (
            TYPE_DECIMAL,
            Obj::Table(vec![
                (0, Field::I32(precision)),
                (1, Field::I32(scale)),
                (2, Field::I32(bits)),
            ]),
        ),
    };
    let mut slots = vec![
        (0, Field::Off(Obj::Str(name.to_string()))),
        (1, Field::Bool(true)),
        (2, Field::U8(type_type)),
        (3, Field::Off(ty)),
        // The C++ reader refuses a field without a children vector.
        (5, Field::Off(Obj::Tables(Vec::new()))),
    ];
    if !l.metadata.is_empty() {
        slots.push((6, Field::Off(key_values(&l.metadata))));
    }
    Obj::Table(slots)
}

fn schema(names: &[&str], layouts: &[Layout]) -> Obj {
    let fields = names
        .iter()
        .zip(layouts)
        .map(|(n, l)| field(n, l))
        .collect();
    Obj::Table(vec![
        (0, Field::I16(0)),
        (1, Field::Off(Obj::Tables(fields))),
    ])
}

fn message(header_type: u8, header: Obj, body_len: usize) -> Vec<u8> {
    finish(Obj::Table(vec![
        (0, Field::I16(V5)),
        (1, Field::U8(header_type)),
        (2, Field::Off(header)),
        (3, Field::I64(body_len as i64)),
    ]))
}

fn structs(items: &[[i64; 2]]) -> Obj {
    Obj::Structs {
        count: items.len() as u32,
        bytes: items
            .iter()
            .flat_map(|[a, b]| a.to_le_bytes().into_iter().chain(b.to_le_bytes()))
            .collect(),
    }
}

/// The record batch message's metadata and its body.
fn record_batch(columns: &[&Column], layouts: &[Layout], len: usize) -> (Vec<u8>, Vec<u8>) {
    let mut body = Vec::new();
    let mut buffers = Vec::new();
    for l in layouts {
        for b in [&l.validity, &l.values] {
            buffers.push([body.len() as i64, b.len() as i64]);
            body.extend_from_slice(b);
            body.resize(body.len().next_multiple_of(64), 0);
        }
    }
    let nodes: Vec<[i64; 2]> = columns
        .iter()
        .map(|c| [len as i64, c.null_count() as i64])
        .collect();
    let header = Obj::Table(vec![
        (0, Field::I64(len as i64)),
        (1, Field::Off(structs(&nodes))),
        (2, Field::Off(structs(&buffers))),
    ]);
    (message(HEADER_RECORD_BATCH, header, body.len()), body)
}

/// Appends a framed message; answers its offset and metadata length.
fn frame(out: &mut Vec<u8>, meta: &[u8], body: &[u8]) -> (usize, usize) {
    let at = out.len();
    out.extend_from_slice(&CONTINUATION);
    out.extend_from_slice(&(meta.len() as i32).to_le_bytes());
    out.extend_from_slice(meta);
    out.extend_from_slice(body);
    (at, 8 + meta.len())
}

struct Parts {
    names: Vec<String>,
    layouts: Vec<Layout>,
    batch: (Vec<u8>, Vec<u8>),
}

fn parts(batch: &Batch) -> Result<Parts, Error> {
    let len = batch.columns.first().map_or(0, |(_, c)| c.len());
    if let Some((name, c)) = batch.columns.iter().find(|(_, c)| c.len() != len) {
        return Err(Error::NotAColumn(format!(
            "{name}: {} lanes beside columns of {len}",
            c.len()
        )));
    }
    let columns: Vec<&Column> = batch.columns.iter().map(|(_, c)| c).collect();
    let layouts: Vec<Layout> = columns.iter().map(|c| layout(c)).collect();
    let rb = record_batch(&columns, &layouts, len);
    Ok(Parts {
        names: batch.columns.iter().map(|(n, _)| n.clone()).collect(),
        layouts,
        batch: rb,
    })
}

fn schema_of(p: &Parts) -> Obj {
    let names: Vec<&str> = p.names.iter().map(String::as_str).collect();
    schema(&names, &p.layouts)
}

/// The batch in the IPC stream format: schema, one record batch, end.
pub fn write_stream(batch: &Batch) -> Result<Vec<u8>, Error> {
    let p = parts(batch)?;
    let mut out = Vec::new();
    frame(&mut out, &message(HEADER_SCHEMA, schema_of(&p), 0), &[]);
    frame(&mut out, &p.batch.0, &p.batch.1);
    out.extend_from_slice(&CONTINUATION);
    out.extend_from_slice(&[0; 4]);
    Ok(out)
}

/// The batch in the IPC file format, with a footer locating its record batch.
pub fn write_file(batch: &Batch) -> Result<Vec<u8>, Error> {
    let p = parts(batch)?;
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&[0, 0]);
    frame(&mut out, &message(HEADER_SCHEMA, schema_of(&p), 0), &[]);
    let (at, meta_len) = frame(&mut out, &p.batch.0, &p.batch.1);
    out.extend_from_slice(&CONTINUATION);
    out.extend_from_slice(&[0; 4]);
    // Block { offset: long, metaDataLength: int, (padding), bodyLength: long }.
    let mut block = (at as i64).to_le_bytes().to_vec();
    block.extend_from_slice(&(meta_len as i32).to_le_bytes());
    block.extend_from_slice(&[0; 4]);
    block.extend_from_slice(&(p.batch.1.len() as i64).to_le_bytes());
    let footer = finish(Obj::Table(vec![
        (0, Field::I16(V5)),
        (1, Field::Off(schema_of(&p))),
        (
            2,
            Field::Off(Obj::Structs {
                count: 0,
                bytes: Vec::new(),
            }),
        ),
        (
            3,
            Field::Off(Obj::Structs {
                count: 1,
                bytes: block,
            }),
        ),
    ]));
    out.extend_from_slice(&footer);
    out.extend_from_slice(&(footer.len() as i32).to_le_bytes());
    out.extend_from_slice(MAGIC);
    Ok(out)
}
