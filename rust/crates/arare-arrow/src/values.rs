//! The interpreter's stack as columns, and columns as stack values
//! (AR-LLR-042).
//!
//! Each stack value is one column, bottom of the stack first. A vector of
//! numbers is a number column: its lanes are aligned to the smallest exponent
//! among them, which is exact, at the narrowest width that holds every lane
//! and is no narrower than any lane's own, so a vector whose lanes share a
//! width and exponent keeps them. A vector of truth values is a truth column.
//! A NIL lane is null in either; a vector of NIL alone, or of nothing, is a
//! truth column. A scalar is a column of one lane. Anything else (a vector
//! mixing numbers and truth values, a nested vector, a block, a name) has no
//! column.

use crate::{Column, Error, NumColumn};
use arare_interp::{Reason, Value};
use arare_kernel::{Num, Truth, Val, Width};

fn lanes(v: &Value) -> Result<&[Value], Error> {
    match v {
        Value::Vector(xs) => Ok(xs),
        Value::Num(_) | Value::Bool(_) | Value::Nil(_) => Ok(core::slice::from_ref(v)),
        other => Err(Error::NotAColumn(format!("{other}"))),
    }
}

fn num_column(xs: &[Value]) -> Result<NumColumn, Error> {
    let nums: Vec<Num> = xs
        .iter()
        .filter_map(|x| match x {
            Value::Num(n) => Some(*n),
            _ => None,
        })
        .collect();
    let scale = nums.iter().map(|n| n.scale()).min().unwrap_or(0);
    let mut width = nums.iter().map(|n| n.width()).max().unwrap_or(Width::W8);
    let mut lanes = Vec::with_capacity(xs.len());
    for x in xs {
        lanes.push(match x {
            Value::Num(n) => Some(match n.val() {
                // At most a 62-bit shift of a code below 2^63: exact in i128.
                Val::Fin(k) => {
                    let aligned = (k as i128) << (n.scale() - scale);
                    let mag = aligned.unsigned_abs();
                    if mag > Width::W64.max_finite() as u128 {
                        return Err(Error::NotAColumn(
                            "a vector whose lanes no one width and exponent hold".into(),
                        ));
                    }
                    width = width.max(Width::narrowest_for(mag));
                    Val::Fin(aligned as i64)
                }
                point => point,
            }),
            Value::Nil(_) => None,
            other => {
                return Err(Error::NotAColumn(format!(
                    "a vector mixing numbers with {other}"
                )))
            }
        });
    }
    Ok(NumColumn {
        width,
        scale,
        lanes,
    })
}

fn column(v: &Value) -> Result<Column, Error> {
    let xs = lanes(v)?;
    if xs.iter().any(|x| matches!(x, Value::Num(_))) {
        return num_column(xs).map(Column::Num);
    }
    xs.iter()
        .map(|x| match x {
            Value::Bool(true) => Ok(Truth::True),
            Value::Bool(false) => Ok(Truth::False),
            Value::Nil(_) => Ok(Truth::Unknown),
            other => Err(Error::NotAColumn(format!("a vector holding {other}"))),
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Column::Truth)
}

/// The stack as columns named by their position from the bottom (AR-HLR-037).
pub fn to_columns(stack: &[Value]) -> Result<Vec<(String, Column)>, Error> {
    stack
        .iter()
        .enumerate()
        .map(|(i, v)| column(v).map(|c| (i.to_string(), c)))
        .collect()
}

/// Columns as vectors to push, first column at the bottom (AR-HLR-038). A
/// null lane is NIL whose reason is that it was imported.
pub fn to_values(columns: &[(String, Column)]) -> Vec<Value> {
    columns
        .iter()
        .map(|(_, c)| {
            Value::Vector(match c {
                Column::Truth(lanes) => lanes
                    .iter()
                    .map(|t| match t {
                        Truth::True => Value::Bool(true),
                        Truth::False => Value::Bool(false),
                        Truth::Unknown => Value::Nil(Reason::Imported),
                    })
                    .collect(),
                Column::Num(n) => n
                    .lanes
                    .iter()
                    .map(|l| match l {
                        Some(v) => Value::Num(
                            Num::new(*v, n.width, n.scale)
                                .expect("a column holds only values of its width and exponent"),
                        ),
                        None => Value::Nil(Reason::Imported),
                    })
                    .collect(),
            })
        })
        .collect()
}
