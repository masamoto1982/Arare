//! The machine (AR-LLR-013): a stack, a program, and the position reached.
//! Quotes are data or code by what consumes them: a Word that runs a block
//! runs it; any other Word evaluates it into a vector first.

use crate::value::{Error, ErrorKind, Value};
use crate::words::apply;
use arare_front::{parse, FrontError, Item, Program};
use arare_kernel::Num;
use std::rc::Rc;

/// A program that cannot run (front end) or stopped on malformed use.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RunError {
    Front(FrontError),
    Interp(Error),
}

impl core::fmt::Display for RunError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RunError::Front(e) => write!(f, "{e}"),
            RunError::Interp(e) => write!(f, "{e}"),
        }
    }
}

/// Runs a program one top-level item at a time.
pub struct Machine {
    stack: Vec<Value>,
    items: Vec<Item>,
    next: usize,
}

impl Machine {
    pub fn new(program: Program) -> Machine {
        Machine {
            stack: Vec::new(),
            items: program.items,
            next: 0,
        }
    }

    pub fn stack(&self) -> &[Value] {
        &self.stack
    }

    pub fn is_done(&self) -> bool {
        self.next == self.items.len()
    }

    /// Execute the next top-level item. `Ok(false)` when there was none.
    pub fn step(&mut self) -> Result<bool, Error> {
        let Some(item) = self.items.get(self.next).cloned() else {
            return Ok(false);
        };
        self.next += 1;
        exec(&mut self.stack, &item)?;
        Ok(true)
    }

    pub fn run(&mut self) -> Result<(), Error> {
        while self.step()? {}
        Ok(())
    }
}

/// Parse and run `source`, answering the final stack.
pub fn run(source: &str) -> Result<Vec<Value>, RunError> {
    let mut m = Machine::new(parse(source).map_err(RunError::Front)?);
    m.run().map_err(RunError::Interp)?;
    Ok(m.stack)
}

pub(crate) fn exec(stack: &mut Vec<Value>, item: &Item) -> Result<(), Error> {
    match item {
        Item::Int(k) => stack.push(Value::Num(
            Num::int(*k).expect("the front end checked the range"),
        )),
        Item::Num { num, .. } => stack.push(Value::Num(*num)),
        Item::Quote(items) => stack.push(Value::Quote(Rc::new(items.clone()))),
        Item::Word(w) => apply(*w, stack).map_err(|kind| Error { word: *w, kind })?,
    }
    Ok(())
}

/// Run a block on `inputs`; it must leave exactly one value.
pub(crate) fn run_block(items: &[Item], inputs: Vec<Value>) -> Result<Value, ErrorKind> {
    let mut stack = inputs;
    for item in items {
        exec(&mut stack, item).map_err(|e| e.kind)?;
    }
    match stack.len() {
        1 => Ok(stack.pop().expect("one value")),
        n => Err(ErrorKind::BlockResult(n)),
    }
}

/// A value as data: a quote is evaluated, on a stack of its own, into the
/// vector of what it leaves, nested quotes included.
pub(crate) fn data(v: Value) -> Result<Value, ErrorKind> {
    match v {
        Value::Quote(items) => {
            let mut stack = Vec::new();
            for item in items.iter() {
                exec(&mut stack, item).map_err(|e| e.kind)?;
            }
            stack
                .into_iter()
                .map(data)
                .collect::<Result<_, _>>()
                .map(Value::Vector)
        }
        other => Ok(other),
    }
}
