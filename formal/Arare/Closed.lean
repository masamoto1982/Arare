import Arare.Codes

/-!
# Closed code computes the integers

A small expression language over the arithmetic Words, evaluated two ways:
`evalW` in the exact domain, and `evalM` the way a machine of half-range `M`
does, fitting every intermediate result into its widest width
(AR-HLR-004). An expression is `closed` when it calls no Word that can leave
the field (`DIV`) and writes no wheel point; `NARROW` is left out of this
language, so the only width limit is the widest one.

The theorems say what `field=closed` buys (AR-HLR-017): closed code computes
the exact integer, and a machine either computes that same integer or answers
`1/0` or `0/0` — never a wrong finite number. That the static analysis which
assigns `field=closed` is sound is Phase 3's theorem, not this one.

Verification case AR-VER-105.
-/

namespace Arare

inductive Expr where
  | lit (k : Int)
  | point (x : W)
  | add (a b : Expr)
  | sub (a b : Expr)
  | mul (a b : Expr)
  | neg (a : Expr)
  | div (a b : Expr)

namespace Expr

def evalW : Expr → W
  | lit k => .fin k
  | point x => x
  | add a b => W.add (evalW a) (evalW b)
  | sub a b => W.sub (evalW a) (evalW b)
  | mul a b => W.mul (evalW a) (evalW b)
  | neg a => W.neg (evalW a)
  | div a b => W.div (evalW a) (evalW b)

/-- The integer an expression denotes; meaningful on closed expressions. -/
def evalZ : Expr → Int
  | lit k => k
  | point _ => 0
  | add a b => evalZ a + evalZ b
  | sub a b => evalZ a - evalZ b
  | mul a b => evalZ a * evalZ b
  | neg a => -evalZ a
  | div _ _ => 0

def closed : Expr → Bool
  | lit _ => true
  | point _ => false
  | add a b => closed a && closed b
  | sub a b => closed a && closed b
  | mul a b => closed a && closed b
  | neg a => closed a
  | div _ _ => false

/-- AR-HLR-003, AR-HLR-017: closed code computes the exact integer. -/
theorem closed_evalW (e : Expr) (h : e.closed = true) : e.evalW = .fin e.evalZ := by
  induction e with
  | lit k => rfl
  | point _ => simp [closed] at h
  | add a b iha ihb =>
    simp only [closed, Bool.and_eq_true] at h
    simp [evalW, evalZ, iha h.1, ihb h.2, W.add]
  | sub a b iha ihb =>
    simp only [closed, Bool.and_eq_true] at h
    simp [evalW, evalZ, iha h.1, ihb h.2, W.fin_sub]
  | mul a b iha ihb =>
    simp only [closed, Bool.and_eq_true] at h
    simp [evalW, evalZ, iha h.1, ihb h.2, W.mul]
  | neg a iha =>
    simp only [closed] at h
    simp [evalW, evalZ, iha h, W.neg]
  | div _ _ => simp [closed] at h

/-- The machine: every intermediate result is fitted into the widest width. -/
def evalM (M : Int) : Expr → W
  | lit k => fit M (.fin k)
  | point x => x
  | add a b => fit M (W.add (evalM M a) (evalM M b))
  | sub a b => fit M (W.sub (evalM M a) (evalM M b))
  | mul a b => fit M (W.mul (evalM M a) (evalM M b))
  | neg a => fit M (W.neg (evalM M a))
  | div a b => fit M (W.div (evalM M a) (evalM M b))

/-- Every intermediate result of `e` lies in the finite range. -/
def fits (M : Int) : Expr → Prop
  | lit k => inRange M k
  | point _ => True
  | add a b => fits M a ∧ fits M b ∧ inRange M (evalZ a + evalZ b)
  | sub a b => fits M a ∧ fits M b ∧ inRange M (evalZ a - evalZ b)
  | mul a b => fits M a ∧ fits M b ∧ inRange M (evalZ a * evalZ b)
  | neg a => fits M a ∧ inRange M (-evalZ a)
  | div _ _ => True

/-- AR-HLR-004, AR-HLR-017: when nothing overflows, the machine computes the
exact integer. -/
theorem closed_evalM_of_fits (M : Int) (e : Expr) (h : e.closed = true) (hf : e.fits M) :
    e.evalM M = .fin e.evalZ := by
  induction e with
  | lit k => simp_all [evalM, evalZ, fits, fit]
  | point _ => simp [closed] at h
  | add a b iha ihb =>
    simp only [closed, Bool.and_eq_true] at h
    simp only [fits] at hf
    simp [evalM, evalZ, iha h.1 hf.1, ihb h.2 hf.2.1, W.add, fit, hf.2.2]
  | sub a b iha ihb =>
    simp only [closed, Bool.and_eq_true] at h
    simp only [fits] at hf
    simp [evalM, evalZ, iha h.1 hf.1, ihb h.2 hf.2.1, W.fin_sub, fit, hf.2.2]
  | mul a b iha ihb =>
    simp only [closed, Bool.and_eq_true] at h
    simp only [fits] at hf
    simp [evalM, evalZ, iha h.1 hf.1, ihb h.2 hf.2.1, W.mul, fit, hf.2.2]
  | neg a iha =>
    simp only [closed] at h
    simp only [fits] at hf
    simp [evalM, evalZ, iha h hf.1, W.neg, fit, hf.2]
  | div _ _ => simp [closed] at h

/-- AR-HLR-004, AR-HLR-017: overflow is never silent. On closed code a machine
answers either the exact integer or a wheel point; whenever it answers a
finite number, that number is the exact one. -/
theorem closed_evalM_sound (M : Int) (e : Expr) (h : e.closed = true) (k : Int)
    (hk : e.evalM M = .fin k) : k = e.evalZ := by
  induction e generalizing k with
  | lit j =>
    simp only [evalM, fit] at hk
    split at hk <;> simp_all [evalZ]
  | point _ => simp [closed] at h
  | add a b iha ihb =>
    simp only [closed, Bool.and_eq_true] at h
    simp only [evalM] at hk
    generalize hx : evalM M a = x at hk
    generalize hy : evalM M b = y at hk
    cases x with
    | fin i =>
      cases y with
      | fin j =>
        have := iha h.1 i hx; have := ihb h.2 j hy
        simp only [W.add, fit] at hk; split at hk <;> simp_all [evalZ]
      | inf => simp [W.add, fit] at hk
      | bot => simp [W.add, fit] at hk
    | inf => cases y <;> simp [W.add, fit] at hk
    | bot => cases y <;> simp [W.add, fit] at hk
  | sub a b iha ihb =>
    simp only [closed, Bool.and_eq_true] at h
    simp only [evalM] at hk
    generalize hx : evalM M a = x at hk
    generalize hy : evalM M b = y at hk
    cases x with
    | fin i =>
      cases y with
      | fin j =>
        have := iha h.1 i hx; have := ihb h.2 j hy
        simp only [W.fin_sub, fit] at hk; split at hk <;> simp_all [evalZ]
      | inf => simp [W.sub, W.add, W.neg, fit] at hk
      | bot => simp [W.sub, W.add, W.neg, fit] at hk
    | inf => cases y <;> simp [W.sub, W.add, W.neg, fit] at hk
    | bot => cases y <;> simp [W.sub, W.add, fit] at hk
  | mul a b iha ihb =>
    simp only [closed, Bool.and_eq_true] at h
    simp only [evalM] at hk
    generalize hx : evalM M a = x at hk
    generalize hy : evalM M b = y at hk
    cases x with
    | fin i =>
      cases y with
      | fin j =>
        have := iha h.1 i hx; have := ihb h.2 j hy
        simp only [W.mul, fit] at hk; split at hk <;> simp_all [evalZ]
      | inf => by_cases hi : i = 0 <;> simp [W.mul, fit, hi] at hk
      | bot => simp [W.mul, fit] at hk
    | inf =>
      cases y with
      | fin j => by_cases hj : j = 0 <;> simp [W.mul, fit, hj] at hk
      | inf => simp [W.mul, fit] at hk
      | bot => simp [W.mul, fit] at hk
    | bot => cases y <;> simp [W.mul, fit] at hk
  | neg a iha =>
    simp only [closed] at h
    simp only [evalM] at hk
    generalize hx : evalM M a = x at hk
    cases x with
    | fin i =>
      have := iha h i hx
      simp only [W.neg, fit] at hk; split at hk <;> simp_all [evalZ]
    | inf => simp [W.neg, fit] at hk
    | bot => simp [W.neg, fit] at hk
  | div _ _ => simp [closed] at h

/-- Associativity, which the exact domain has (`W.add_assoc`), does not
survive the widest width: a sum that overflows on the way is `1/0` even when
the final sum would fit. -/
theorem machine_add_not_assoc :
    evalM 4 (.add (.add (.lit 2) (.lit 1)) (.lit (-1))) ≠
      evalM 4 (.add (.lit 2) (.add (.lit 1) (.lit (-1)))) := by
  decide

end Expr
end Arare
