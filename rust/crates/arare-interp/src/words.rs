//! What each Word does (AR-LLR-015, refining AR-HLR-003 to AR-HLR-016 and
//! AR-HLR-021 to AR-HLR-026). Operands are popped in the order they were
//! pushed; the arithmetic is `arare-kernel`'s.

use crate::lift::{and, lift1, lift2, not, select};
use crate::machine::{data, run_block, Env};
use crate::value::{ErrorKind, Reason, Value};
use arare_front::{Item, Word};
use arare_kernel::{Num, Truth, Width, MAX_SCALE};
use std::rc::Rc;

fn pop(stack: &mut Vec<Value>, n: usize) -> Result<Vec<Value>, ErrorKind> {
    if stack.len() < n {
        return Err(ErrorKind::StackUnderflow {
            needed: n,
            had: stack.len(),
        });
    }
    Ok(stack.split_off(stack.len() - n))
}

fn pop_data(stack: &mut Vec<Value>, n: usize) -> Result<Vec<Value>, ErrorKind> {
    pop(stack, n)?.into_iter().map(data).collect()
}

fn num(v: &Value) -> Result<Num, ErrorKind> {
    match v {
        Value::Num(n) => Ok(*n),
        _ => Err(ErrorKind::Type("a number")),
    }
}

fn block(v: Value) -> Result<Rc<Vec<Item>>, ErrorKind> {
    match v {
        Value::Quote(items) => Ok(items),
        _ => Err(ErrorKind::Type("a block")),
    }
}

/// An operand that must be an integer between `lo` and `hi`, whatever its
/// width and scale.
fn integer(v: &Value, lo: i64, hi: i64, accepts: &'static str) -> Result<i64, ErrorKind> {
    match v {
        Value::Num(n) => match n.exact() {
            (k, 1) if (lo as i128..=hi as i128).contains(&k) => Ok(k as i64),
            _ => Err(ErrorKind::BadOperand(accepts)),
        },
        _ => Err(ErrorKind::BadOperand(accepts)),
    }
}

fn width(v: &Value) -> Result<Width, ErrorKind> {
    let bits = integer(v, 8, 64, "a width of 8, 16, 32 or 64")?;
    Width::ALL
        .into_iter()
        .find(|w| w.bits() as i64 == bits)
        .ok_or(ErrorKind::BadOperand("a width of 8, 16, 32 or 64"))
}

fn scale_overflow<T>(r: Result<T, arare_kernel::ScaleOverflow>) -> Result<T, ErrorKind> {
    r.map_err(|_| ErrorKind::ScaleOverflow)
}

fn arithmetic(w: Word, a: &Value, b: &Value) -> Result<Value, ErrorKind> {
    lift2(a, b, &|x, y| {
        let (x, y) = (num(x)?, num(y)?);
        Ok(Value::Num(match w {
            Word::Add => x.add(y),
            Word::Sub => x.sub(y),
            Word::Mul => scale_overflow(x.mul(y))?,
            _ => x.div(y),
        }))
    })
}

/// LT over numbers; the order of a wheel point is UNKNOWN, a NIL.
fn less(a: &Value, b: &Value) -> Result<Value, ErrorKind> {
    lift2(a, b, &|x, y| {
        Ok(match num(x)?.lt(num(y)?) {
            Truth::True => Value::Bool(true),
            Truth::False => Value::Bool(false),
            Truth::Unknown => Value::Nil(Reason::OrderOfPoint),
        })
    })
}

/// EQ compares whole values by what they denote (AR-HLR-010).
fn equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => Num::eq(*x, *y) == Truth::True,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Nil(_), Value::Nil(_)) => true,
        (Value::Text(x), Value::Text(y)) => x == y,
        (Value::Vector(xs), Value::Vector(ys)) => {
            xs.len() == ys.len() && xs.iter().zip(ys).all(|(x, y)| equal(x, y))
        }
        _ => false,
    }
}

fn elements(v: Value) -> Result<Vec<Value>, ErrorKind> {
    match v {
        Value::Vector(xs) => Ok(xs),
        _ => Err(ErrorKind::Type("a vector")),
    }
}

/// SUM skips NIL and is NIL when nothing is left (AR-HLR-016).
fn sum(v: Value) -> Result<Value, ErrorKind> {
    let mut total: Option<Num> = None;
    for x in elements(v)? {
        match x {
            Value::Nil(_) => {}
            Value::Num(n) => total = Some(total.map_or(n, |t| t.add(n))),
            _ => return Err(ErrorKind::Type("a vector of numbers")),
        }
    }
    Ok(total.map_or(Value::Nil(Reason::EmptyAggregate), Value::Num))
}

/// The names a block reads, its nested blocks included.
fn names_read(items: &[Item], out: &mut Vec<String>) {
    for item in items {
        match item {
            Item::Name(n) => out.push(n.clone()),
            Item::Quote(inner) => names_read(inner, out),
            _ => {}
        }
    }
}

/// Whether binding `value` to `name` would let it read `name`: directly, or
/// through the blocks bound to the names it reads.
fn reads_itself(name: &str, value: &Value, env: &Env) -> bool {
    let Value::Quote(items) = value else {
        return false;
    };
    let mut todo = Vec::new();
    names_read(items, &mut todo);
    let mut seen: Vec<String> = Vec::new();
    while let Some(n) = todo.pop() {
        if n == name {
            return true;
        }
        if seen.contains(&n) {
            continue;
        }
        if let Some(Value::Quote(inner)) = env.get(&n) {
            names_read(inner, &mut todo);
        }
        seen.push(n);
    }
    false
}

/// BIND (AR-LLR-028): `value 'X' BIND` names the value; `vector [ 'A' 'B' ]
/// BIND` names each element of a vector of the same length. Both operands
/// are consumed and nothing is pushed.
fn bind(stack: &mut Vec<Value>, env: &mut Env) -> Result<(), ErrorKind> {
    let mut o = pop(stack, 2)?;
    let names = o.pop().expect("two operands");
    let value = o.pop().expect("two operands");
    if matches!(value, Value::Text(_)) {
        return Err(ErrorKind::BadOperand("a value to bind that is not a name"));
    }
    match names {
        Value::Text(name) => {
            if reads_itself(&name, &value, env) {
                return Err(ErrorKind::SelfReference(name.to_string()));
            }
            env.bind(name, value);
        }
        Value::Quote(_) => {
            let mut texts = Vec::new();
            for n in elements(data(names)?)? {
                let Value::Text(n) = n else {
                    return Err(ErrorKind::BadOperand("a name, or a block of names"));
                };
                texts.push(n);
            }
            let names = texts;
            let values = elements(data(value)?)?;
            if values.len() != names.len() {
                return Err(ErrorKind::Shape {
                    left: values.len(),
                    right: names.len(),
                });
            }
            for (n, v) in names.into_iter().zip(values) {
                env.bind(n, v);
            }
        }
        _ => return Err(ErrorKind::BadOperand("a name, or a block of names")),
    }
    Ok(())
}

pub(crate) fn apply(w: Word, stack: &mut Vec<Value>, env: &mut Env) -> Result<(), ErrorKind> {
    let result = match w {
        Word::True => Value::Bool(true),
        Word::False => Value::Bool(false),
        Word::Nil => Value::Nil(Reason::Literal),
        Word::Add | Word::Sub | Word::Mul | Word::Div => {
            let o = pop_data(stack, 2)?;
            arithmetic(w, &o[0], &o[1])?
        }
        Word::Neg => {
            let o = pop_data(stack, 1)?;
            lift1(&o[0], &|x| Ok(Value::Num(num(x)?.neg())))?
        }
        Word::Lt => {
            let o = pop_data(stack, 2)?;
            less(&o[0], &o[1])?
        }
        Word::Eq => {
            let o = pop_data(stack, 2)?;
            match (&o[0], &o[1]) {
                (Value::Nil(r), _) | (_, Value::Nil(r)) => Value::Nil(*r),
                (a, b) => Value::Bool(equal(a, b)),
            }
        }
        Word::And => {
            let o = pop_data(stack, 2)?;
            and(&o[0], &o[1])?
        }
        Word::Not => not(&pop_data(stack, 1)?[0])?,
        Word::Rescale => {
            let o = pop_data(stack, 2)?;
            let s = integer(&o[1], 0, 62, "a shift from 0 to 62")? as u32;
            lift1(&o[0], &|x| {
                Ok(Value::Num(scale_overflow(num(x)?.rescale(s))?))
            })?
        }
        Word::Narrow => {
            let o = pop_data(stack, 2)?;
            let w = width(&o[1])?;
            lift1(&o[0], &|x| Ok(Value::Num(num(x)?.narrow(w))))?
        }
        Word::Quantize => {
            let o = pop_data(stack, 3)?;
            let (w, e) = (
                width(&o[1])?,
                integer(
                    &o[2],
                    -(MAX_SCALE as i64),
                    MAX_SCALE as i64,
                    "an exponent within ±31",
                )?,
            );
            lift1(&o[0], &|x| {
                Ok(Value::Num(scale_overflow(num(x)?.quantize(w, e as i32))?))
            })?
        }
        Word::Select => {
            let o = pop_data(stack, 3)?;
            select(&o[0], &o[1], &o[2])?
        }
        Word::Sum => match data(pop(stack, 1)?.remove(0))? {
            Value::Nil(r) => Value::Nil(r),
            v => sum(v)?,
        },
        Word::Map | Word::Filter => {
            let mut o = pop(stack, 2)?;
            let code = block(o.pop().expect("two operands"))?;
            match data(o.pop().expect("two operands"))? {
                Value::Nil(r) => Value::Nil(r),
                v => {
                    let mut out = Vec::new();
                    for x in elements(v)? {
                        let r = run_block(&code, vec![x.clone()], env)?;
                        match (w, r) {
                            (Word::Map, r) => out.push(r),
                            (_, Value::Bool(true)) => out.push(x),
                            (_, Value::Bool(false) | Value::Nil(_)) => {}
                            _ => {
                                return Err(ErrorKind::Type("a predicate answering a truth value"))
                            }
                        }
                    }
                    Value::Vector(out)
                }
            }
        }
        Word::Bind => return bind(stack, env),
        Word::Fold => {
            let mut o = pop(stack, 3)?;
            let code = block(o.pop().expect("three operands"))?;
            let init = data(o.pop().expect("three operands"))?;
            match data(o.pop().expect("three operands"))? {
                Value::Nil(r) => Value::Nil(r),
                v => elements(v)?
                    .into_iter()
                    .try_fold(init, |acc, x| run_block(&code, vec![acc, x], env))?,
            }
        }
    };
    stack.push(result);
    Ok(())
}
