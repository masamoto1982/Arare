//! AR-LLR-001. Mirrors `formal/Arare/Sql.lean` (AR-HLR-010, AR-HLR-011, AR-HLR-013 to
//! AR-HLR-016). NIL is `None`.

use crate::truth::Truth;
use crate::wheel::W;

/// `truthOf`: NIL read in truth position is UNKNOWN.
pub fn truth_of(t: Option<Truth>) -> Truth {
    t.unwrap_or(Truth::Unknown)
}

/// `eqT`: equality by denotation, always decided.
pub fn eq_t(x: W, y: W) -> Truth {
    if x == y {
        Truth::True
    } else {
        Truth::False
    }
}

/// `ltT`: order over finite numbers; a wheel point makes it UNKNOWN.
pub fn lt_t(x: W, y: W) -> Truth {
    match (x, y) {
        (W::Fin(a), W::Fin(b)) => {
            if a < b {
                Truth::True
            } else {
                Truth::False
            }
        }
        _ => Truth::Unknown,
    }
}

/// `filterT`: keep exactly the elements the predicate calls TRUE.
pub fn filter_t<T: Clone>(p: impl Fn(&T) -> Option<Truth>, xs: &[T]) -> Vec<T> {
    xs.iter()
        .filter(|x| truth_of(p(x)) == Truth::True)
        .cloned()
        .collect()
}

/// `selectT`: THEN only on TRUE.
pub fn select_t<T>(c: Option<Truth>, a: T, b: T) -> T {
    if truth_of(c) == Truth::True {
        a
    } else {
        b
    }
}

/// `sumT`: skip NIL; NIL when nothing is left. Folds left from the first
/// present element, as the Lean definition does.
pub fn sum_t(xs: &[Option<W>]) -> Option<W> {
    let mut present = xs.iter().flatten().copied();
    let first = present.next()?;
    Some(present.fold(first, W::add))
}
