//! The machine (AR-LLR-013): a stack, a program, the position reached, and
//! the names bound so far (AR-LLR-028). Quotes are data or code by what
//! consumes them: a Word that runs a block runs it; any other Word evaluates
//! it into a vector first.

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

/// The names bound so far, scope by scope (AR-LLR-028). The program has
/// the outermost scope; every block a Word runs opens one inside the scope it
/// runs in and closes it when it finishes, so a block reads the names of the
/// code around it and its own bindings end with it, as Ajisai's do.
#[derive(Clone, Debug)]
pub(crate) struct Env {
    scopes: Vec<Vec<(Rc<str>, Value)>>,
}

impl Env {
    pub(crate) fn new() -> Env {
        Env {
            scopes: vec![Vec::new()],
        }
    }

    /// The value bound to `name` in the innermost scope that binds it.
    pub(crate) fn get(&self, name: &str) -> Option<&Value> {
        self.scopes
            .iter()
            .rev()
            .find_map(|s| s.iter().find(|(n, _)| &**n == name).map(|(_, v)| v))
    }

    /// Bind `name` in the innermost scope; binding a name again there
    /// replaces its value.
    pub(crate) fn bind(&mut self, name: Rc<str>, value: Value) {
        let scope = self.scopes.last_mut().expect("the program's scope");
        match scope.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = value,
            None => scope.push((name, value)),
        }
    }
}

/// Runs a program one top-level item at a time.
pub struct Machine {
    stack: Vec<Value>,
    env: Env,
    items: Vec<Item>,
    next: usize,
}

impl Machine {
    pub fn new(program: Program) -> Machine {
        Machine::with_stack(program, Vec::new())
    }

    /// A machine that starts with `stack` already on its stack, bottom
    /// first: how the compiled route hands a program its inputs (AR-LLR-023).
    pub fn with_stack(program: Program, stack: Vec<Value>) -> Machine {
        Machine {
            stack,
            env: Env::new(),
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
        exec(&mut self.stack, &mut self.env, &item)?;
        Ok(true)
    }

    pub fn run(&mut self) -> Result<(), Error> {
        while self.step()? {}
        Ok(())
    }
}

/// Parse and run `source`, answering the final stack.
pub fn run(source: &str) -> Result<Vec<Value>, RunError> {
    run_on(source, Vec::new())
}

/// Parse and run `source` on a stack that already holds `inputs`.
pub fn run_on(source: &str, inputs: Vec<Value>) -> Result<Vec<Value>, RunError> {
    let mut m = Machine::with_stack(parse(source).map_err(RunError::Front)?, inputs);
    m.run().map_err(RunError::Interp)?;
    Ok(m.stack)
}

pub(crate) fn exec(stack: &mut Vec<Value>, env: &mut Env, item: &Item) -> Result<(), Error> {
    match item {
        Item::Int(k) => stack.push(Value::Num(
            Num::int(*k).expect("the front end checked the range"),
        )),
        Item::Num { num, .. } => stack.push(Value::Num(*num)),
        Item::Quote(items) => stack.push(Value::Quote(Rc::new(items.clone()))),
        Item::Text(name) => stack.push(Value::Text(name.as_str().into())),
        Item::Name(name) => match env.get(name) {
            Some(v) => stack.push(v.clone()),
            None => {
                return Err(Error {
                    word: None,
                    kind: ErrorKind::Unbound(name.clone()),
                })
            }
        },
        Item::Word(w) => apply(*w, stack, env).map_err(|kind| Error {
            word: Some(*w),
            kind,
        })?,
    }
    Ok(())
}

/// Run a block on `inputs`, in a scope of its own; it must leave exactly one
/// value.
pub(crate) fn run_block(
    items: &[Item],
    inputs: Vec<Value>,
    env: &mut Env,
) -> Result<Value, ErrorKind> {
    env.scopes.push(Vec::new());
    let mut stack = inputs;
    let ran = items
        .iter()
        .try_for_each(|item| exec(&mut stack, env, item).map_err(|e| e.kind));
    env.scopes.pop();
    ran?;
    match stack.len() {
        1 => Ok(stack.pop().expect("one value")),
        n => Err(ErrorKind::BlockResult(n)),
    }
}

/// A value as data: a quote is evaluated, on a stack of its own, into the
/// vector of what it leaves, nested quotes included. Its names read the
/// scope it is evaluated in.
pub(crate) fn data(v: Value, env: &mut Env) -> Result<Value, ErrorKind> {
    match v {
        Value::Quote(items) => {
            let mut stack = Vec::new();
            for item in items.iter() {
                exec(&mut stack, env, item).map_err(|e| e.kind)?;
            }
            stack
                .into_iter()
                .map(|v| data(v, env))
                .collect::<Result<_, _>>()
                .map(Value::Vector)
        }
        other => Ok(other),
    }
}
