/-!
# Three-valued truth

The strong Kleene logic SQL uses (AR-HLR-012): `AND` is the meet of the truth
order `FALSE < UNKNOWN < TRUE` and `NOT` reverses it. The tables are Ajisai's
LANG.VALUES.TRUTH, so the definitions below are those tables written as
pattern matches, and the theorems are the laws the tables obey.

Verification case AR-VER-104.
-/

namespace Arare

inductive Truth where
  | false
  | unknown
  | true
  deriving DecidableEq, Repr

namespace Truth

def rank : Truth → Nat
  | .false => 0
  | .unknown => 1
  | .true => 2

def and : Truth → Truth → Truth
  | .false, _ => .false
  | _, .false => .false
  | .unknown, _ => .unknown
  | _, .unknown => .unknown
  | .true, .true => .true

def not : Truth → Truth
  | .false => .true
  | .unknown => .unknown
  | .true => .false

/-- A disjunction is written from `AND` and `NOT`, as in Ajisai. -/
def or (a b : Truth) : Truth := not (and (not a) (not b))

/-- AR-HLR-012: `AND` is the meet of the truth order. -/
theorem and_rank (a b : Truth) : (and a b).rank = min a.rank b.rank := by
  cases a <;> cases b <;> decide

/-- AR-HLR-012: `OR` is the join. -/
theorem or_rank (a b : Truth) : (or a b).rank = max a.rank b.rank := by
  cases a <;> cases b <;> decide

theorem not_rank (a : Truth) : (not a).rank = 2 - a.rank := by
  cases a <;> rfl

theorem and_comm (a b : Truth) : and a b = and b a := by cases a <;> cases b <;> rfl
theorem and_assoc (a b c : Truth) : and (and a b) c = and a (and b c) := by
  cases a <;> cases b <;> cases c <;> rfl
theorem and_idem (a : Truth) : and a a = a := by cases a <;> rfl
theorem and_true (a : Truth) : and a .true = a := by cases a <;> rfl
theorem and_false (a : Truth) : and a .false = .false := by cases a <;> rfl
theorem or_comm (a b : Truth) : or a b = or b a := by cases a <;> cases b <;> rfl
theorem or_assoc (a b c : Truth) : or (or a b) c = or a (or b c) := by
  cases a <;> cases b <;> cases c <;> rfl
theorem not_not (a : Truth) : not (not a) = a := by cases a <;> rfl
theorem de_morgan_and (a b : Truth) : not (and a b) = or (not a) (not b) := by
  cases a <;> cases b <;> rfl
theorem absorb (a b : Truth) : and a (or a b) = a := by cases a <;> cases b <;> rfl
theorem distrib (a b c : Truth) : and a (or b c) = or (and a b) (and a c) := by
  cases a <;> cases b <;> cases c <;> rfl

/-- Kleene's law, which separates strong Kleene logic from an arbitrary
De Morgan algebra: `a ∧ ¬a ≤ b ∨ ¬b`. -/
theorem kleene (a b : Truth) : (and a (not a)).rank ≤ (or b (not b)).rank := by
  cases a <;> cases b <;> decide

/-- UNKNOWN is not a third Boolean: excluded middle fails exactly there. -/
theorem excluded_middle_fails : or .unknown (not .unknown) = .unknown := rfl

end Truth
end Arare
