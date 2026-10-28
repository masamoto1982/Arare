import Arare.Rounding

/-!
# The value domain and its wheel points

`W` is the exact value domain of Arare's numbers: an integer code at a common
scale, or one of the two wheel points, `1/0` (`inf`) and `0/0` (`bot`)
(AR-HLR-005, AR-HLR-006, AR-HLR-007). It is the part of Ajisai's wheel of
fractions over ℤ that fixed-point codes can reach: `n/1`, `1/0` and `0/0`.
That part is closed under `+`, `·` and negation, but not under the
reciprocal, so the reciprocal-free wheel axioms are proved here and the
reciprocal ones are not claimed (docs/quality/FORMAL_VERIFICATION.md).

Verification case AR-VER-102.
-/

-- The case-split proofs below hand `simp_all` one lemma set for every case;
-- not every case uses every lemma.
set_option linter.unusedSimpArgs false

namespace Arare

inductive W where
  | fin (k : Int)
  | inf
  | bot
  deriving DecidableEq, Repr

namespace W

/-- AR-HLR-005: `0/0` absorbs; `1/0` plus a finite number is `1/0`; `1/0`
plus `1/0` is `0/0`. -/
def add : W → W → W
  | bot, _ => bot
  | _, bot => bot
  | inf, inf => bot
  | inf, fin _ => inf
  | fin _, inf => inf
  | fin a, fin b => fin (a + b)

/-- AR-HLR-005: `1/0` has no sign, so it is its own negation. -/
def neg : W → W
  | fin a => fin (-a)
  | inf => inf
  | bot => bot

def sub (x y : W) : W := add x (neg y)

/-- AR-HLR-006: `1/0` times a nonzero number or `1/0` is `1/0`; `1/0` times
zero is `0/0`; `0/0` absorbs. -/
def mul : W → W → W
  | bot, _ => bot
  | _, bot => bot
  | inf, inf => inf
  | inf, fin b => if b = 0 then bot else inf
  | fin a, inf => if a = 0 then bot else inf
  | fin a, fin b => fin (a * b)

/-- AR-HLR-007: division by zero gives `1/0`, or `0/0` for zero; a finite
number over `1/0` is zero; `1/0` over anything but `1/0` is `1/0`; two finite
numbers give the quotient rounded half to even. -/
def div : W → W → W
  | bot, _ => bot
  | _, bot => bot
  | inf, inf => bot
  | inf, fin _ => inf
  | fin _, inf => fin 0
  | fin a, fin b => if b = 0 then (if a = 0 then bot else inf) else fin (roundDiv a b)

/-- A wheel point, as opposed to a finite number. -/
def isPoint : W → Bool
  | fin _ => false
  | _ => true

/-! ## The reciprocal-free wheel axioms (AR-HLR-005, AR-HLR-006) -/

theorem add_comm (x y : W) : add x y = add y x := by
  cases x <;> cases y <;> simp [add, Int.add_comm]

theorem add_assoc (x y z : W) : add (add x y) z = add x (add y z) := by
  cases x <;> cases y <;> cases z <;> simp [add, Int.add_assoc]

theorem add_zero (x : W) : add x (fin 0) = x := by
  cases x <;> simp [add]

theorem mul_comm (x y : W) : mul x y = mul y x := by
  cases x <;> cases y <;> simp [mul, Int.mul_comm]

theorem mul_assoc (x y z : W) : mul (mul x y) z = mul x (mul y z) := by
  rcases x with a | _ | _ <;> rcases y with b | _ | _ <;> rcases z with c | _ | _ <;>
    (try by_cases ha : a = 0) <;> (try by_cases hb : b = 0) <;> (try by_cases hc : c = 0) <;>
    simp_all [mul, Int.mul_assoc, Int.mul_eq_zero]

theorem mul_one (x : W) : mul x (fin 1) = x := by
  cases x <;> simp [mul]

/-- Carlström's distributive law for wheels: `(x+y)z + 0z = xz + yz`. -/
theorem wheel_distrib (x y z : W) :
    add (mul (add x y) z) (mul (fin 0) z) = add (mul x z) (mul y z) := by
  rcases x with a | _ | _ <;> rcases y with b | _ | _ <;> rcases z with c | _ | _ <;>
    (try by_cases ha : a = 0) <;> (try by_cases hb : b = 0) <;> (try by_cases hc : c = 0) <;>
    (try by_cases hab : a + b = 0) <;>
    simp_all [add, mul, Int.add_mul] <;>
    (rw [← Int.add_mul]; simp [hab])

/-- Carlström: `0·0 = 0`. -/
theorem zero_mul_zero : mul (fin 0) (fin 0) = fin 0 := rfl

/-- Carlström: `(x + 0y)z = xz + 0y`. -/
theorem wheel_zero_term (x y z : W) :
    mul (add x (mul (fin 0) y)) z = add (mul x z) (mul (fin 0) y) := by
  rcases x with a | _ | _ <;> rcases y with b | _ | _ <;> rcases z with c | _ | _ <;>
    (try by_cases ha : a = 0) <;> (try by_cases hc : c = 0) <;>
    simp_all [add, mul]

/-- Carlström: `0/0 + x = 0/0`, and `0/0` absorbs multiplication too. -/
theorem bot_add (x : W) : add bot x = bot := by cases x <;> rfl
theorem bot_mul (x : W) : mul bot x = bot := by cases x <;> rfl

theorem neg_neg (x : W) : neg (neg x) = x := by
  cases x <;> simp [neg]

/-! ## Where the ring laws give way (AR-HLR-005, AR-HLR-006)

These are the laws an implementation must not restore by special case. -/

theorem inf_sub_inf : sub inf inf = bot := rfl
theorem zero_mul_inf : mul (fin 0) inf = bot := rfl
theorem not_mul_zero : ¬ ∀ x, mul (fin 0) x = fin 0 := fun h => by
  have := h inf
  simp [mul] at this
theorem not_sub_self : ¬ ∀ x, sub x x = fin 0 := fun h => by
  have := h inf
  simp [sub, add, neg] at this

/-! ## Finite numbers are the integers (AR-HLR-003) -/

theorem fin_add (a b : Int) : add (fin a) (fin b) = fin (a + b) := rfl
theorem fin_mul (a b : Int) : mul (fin a) (fin b) = fin (a * b) := rfl
theorem fin_neg (a : Int) : neg (fin a) = fin (-a) := rfl
theorem fin_sub (a b : Int) : sub (fin a) (fin b) = fin (a - b) := by
  simp [sub, add, neg, Int.sub_eq_add_neg]

/-- AR-HLR-003: under `+`, `−`, `·` and negation a wheel point never turns
back into a finite number, so a point that appears is never lost. -/
theorem add_point_left (x y : W) (h : x.isPoint = true) : (add x y).isPoint = true := by
  cases x <;> cases y <;> simp_all [isPoint, add]
theorem add_point_right (x y : W) (h : y.isPoint = true) : (add x y).isPoint = true := by
  rw [add_comm]; exact add_point_left y x h
theorem mul_point_left (x y : W) (h : x.isPoint = true) : (mul x y).isPoint = true := by
  rcases x with a | _ | _ <;> rcases y with b | _ | _ <;> (try by_cases hb : b = 0) <;>
    simp_all [isPoint, mul]
theorem mul_point_right (x y : W) (h : y.isPoint = true) : (mul x y).isPoint = true := by
  rw [mul_comm]; exact mul_point_left y x h
theorem neg_point (x : W) (h : x.isPoint = true) : (neg x).isPoint = true := by
  cases x <;> simp_all [isPoint, neg]

/-! ## Division (AR-HLR-007) -/

theorem div_by_zero (a : Int) (h : a ≠ 0) : div (fin a) (fin 0) = inf := by
  simp [div, h]
theorem zero_div_zero : div (fin 0) (fin 0) = bot := rfl
theorem fin_div_inf (a : Int) : div (fin a) inf = fin 0 := rfl
theorem inf_div_inf : div inf inf = bot := rfl

/-- AR-HLR-007: dividing two finite numbers rounds at most half a unit. -/
theorem div_error (a b : Int) (hb : b ≠ 0) :
    ∃ q, div (fin a) (fin b) = fin q ∧
      -(if 0 < b then b else -b) ≤ 2 * (a - b * q) ∧
      2 * (a - b * q) ≤ (if 0 < b then b else -b) :=
  ⟨roundDiv a b, by simp [div, hb], roundDiv_error a b hb⟩

end W
end Arare
