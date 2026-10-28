import Arare.Wheel
import Arare.Truth

/-!
# Order, selection and aggregation, the SQL way

Comparison of numbers into three-valued truth (AR-HLR-010, AR-HLR-011), the
reading of NIL in truth position (AR-HLR-013), and the three places Arare
follows SQL: `FILTER` keeps the elements its predicate calls TRUE
(AR-HLR-014), a branch on UNKNOWN takes the ELSE side (AR-HLR-015), and an
aggregate skips NIL (AR-HLR-016). NIL is `none` throughout.

Verification case AR-VER-106.
-/

namespace Arare

/-- AR-HLR-013: NIL read in truth position is UNKNOWN. -/
def truthOf : Option Truth → Truth
  | some t => t
  | none => .unknown

theorem truthOf_nil : truthOf none = .unknown := rfl

/-- AR-HLR-010: equality is decided everywhere, by denotation. -/
def eqT (x y : W) : Truth := if x = y then .true else .false

/-- AR-HLR-011: order is decided over the finite numbers; asking the order of
a wheel point is UNKNOWN. -/
def ltT : W → W → Truth
  | .fin a, .fin b => if a < b then .true else .false
  | _, _ => .unknown

theorem eqT_bot : eqT .bot .bot = .true := rfl
theorem eqT_inf : eqT .inf .inf = .true := rfl
theorem eqT_decided (x y : W) : eqT x y ≠ .unknown := by
  unfold eqT; split <;> simp

theorem ltT_point_left (x y : W) (h : x.isPoint = true) : ltT x y = .unknown := by
  cases x <;> cases y <;> simp_all [W.isPoint, ltT]
theorem ltT_point_right (x y : W) (h : y.isPoint = true) : ltT x y = .unknown := by
  cases x <;> cases y <;> simp_all [W.isPoint, ltT]
theorem ltT_fin_decided (a b : Int) : ltT (.fin a) (.fin b) ≠ .unknown := by
  simp only [ltT]; split <;> simp

/-- AR-HLR-014: `FILTER` keeps exactly the elements its predicate calls TRUE;
FALSE, UNKNOWN and NIL all drop the element, as SQL's WHERE does. -/
def filterT {α : Type} (p : α → Option Truth) (xs : List α) : List α :=
  xs.filter (fun x => truthOf (p x) == .true)

theorem mem_filterT {α : Type} (p : α → Option Truth) (xs : List α) (x : α) :
    x ∈ filterT p xs ↔ x ∈ xs ∧ truthOf (p x) = .true := by
  simp [filterT]

/-- AR-HLR-015: a branch takes its THEN side only on TRUE; on FALSE, UNKNOWN
or NIL it takes the ELSE side, as SQL's CASE does. -/
def selectT {α : Type} (c : Option Truth) (a b : α) : α :=
  if truthOf c = .true then a else b

theorem selectT_true {α : Type} (a b : α) : selectT (some .true) a b = a := rfl
theorem selectT_false {α : Type} (a b : α) : selectT (some .false) a b = b := rfl
theorem selectT_unknown {α : Type} (a b : α) : selectT (some .unknown) a b = b := rfl
theorem selectT_nil {α : Type} (a b : α) : selectT none a b = b := rfl

/-- AR-HLR-016: `SUM` skips NIL; with nothing left to add it is NIL. -/
def sumT (xs : List (Option W)) : Option W :=
  match xs.filterMap id with
  | [] => none
  | v :: vs => some (vs.foldl W.add v)

theorem sumT_all_nil (n : Nat) : sumT (List.replicate n none) = none := by
  induction n with
  | zero => rfl
  | succ n ih => simp [sumT, List.replicate_succ]

/-- AR-HLR-016: a NIL anywhere in the input does not change the sum. -/
theorem sumT_skip_nil (xs ys : List (Option W)) :
    sumT (xs ++ none :: ys) = sumT (xs ++ ys) := by
  simp [sumT, List.filterMap_append]

theorem sumT_two (a b : W) : sumT [some a, some b] = some (W.add a b) := rfl

end Arare
