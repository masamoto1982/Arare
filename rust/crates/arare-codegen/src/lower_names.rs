//! Names and blocks in lowering (AR-LLR-029, refining AR-HLR-030 and
//! AR-HLR-032): BIND binds a name to a symbol in the innermost scope, a name
//! pushes the symbol it is bound to, and every block runs in a scope of its
//! own — as the interpreter's `Env` does with values (AR-LLR-028).

use crate::lower::{no, Lower, Sym, Unsupported};
use arare_front::Item;

impl Lower {
    /// Run a block on `inputs` in a scope of its own, as the interpreter runs
    /// one; what it leaves.
    pub(crate) fn block(
        &mut self,
        code: &[Item],
        inputs: Vec<Sym>,
    ) -> Result<Vec<Sym>, Unsupported> {
        self.scopes.push(Vec::new());
        let mut stack = inputs;
        let ran = code.iter().try_for_each(|item| self.exec(&mut stack, item));
        self.scopes.pop();
        ran.map(|()| stack)
    }

    pub(crate) fn lookup(&self, name: &str) -> Option<&Sym> {
        self.scopes
            .iter()
            .rev()
            .find_map(|s| s.iter().find(|(n, _)| &**n == name).map(|(_, v)| v))
    }

    /// Whether a block bound to `name` would read `name`, directly or through
    /// the blocks bound to the names it reads: the interpreter refuses such a
    /// BIND, so the fast path must not answer it.
    fn reads_itself(&self, name: &str, code: &[Item]) -> bool {
        fn names(items: &[Item], out: &mut Vec<String>) {
            for item in items {
                match item {
                    Item::Name(n) => out.push(n.clone()),
                    Item::Quote(inner) => names(inner, out),
                    _ => {}
                }
            }
        }
        let mut todo = Vec::new();
        names(code, &mut todo);
        let mut seen: Vec<String> = Vec::new();
        while let Some(n) = todo.pop() {
            if n == name {
                return true;
            }
            if !seen.contains(&n) {
                if let Some(Sym::Quote(inner)) = self.lookup(&n) {
                    names(inner, &mut todo);
                }
                seen.push(n);
            }
        }
        false
    }

    /// BIND of one name; a block of names is not compiled.
    pub(crate) fn bind(&mut self, stack: &mut Vec<Sym>) -> Result<(), Unsupported> {
        let mut o = self.pop(stack, 2)?;
        let names = o.pop().expect("two operands");
        let value = o.pop().expect("two operands");
        let Sym::Text(name) = names else {
            return no("BIND of several names");
        };
        match &value {
            Sym::Text(_) => return no("BIND of a name"),
            Sym::Quote(code) if self.reads_itself(&name, code) => {
                return no("BIND of a block that reads its own name")
            }
            _ => {}
        }
        let scope = self.scopes.last_mut().expect("the program's scope");
        match scope.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = value,
            None => scope.push((name, value)),
        }
        Ok(())
    }
}
