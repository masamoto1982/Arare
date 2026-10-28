import Arare.Wheel

/-!
# Value codes, NARROW and overflow

A width of `N` bits has half-range `M = 2^(N-1)`: codes run over `[−M, M−1]`.
Code `−M` is `0/0`, code `M−1` is `1/0`, the finite numbers are the symmetric
range `[−(M−2), M−2]`, and code `−(M−1)` is never produced (AR-HLR-002). The
proofs hold for every `M ≥ 2`; the widths Arare has are `M = 2^7, 2^15, 2^31,
2^63` (AR-HLR-001).

Verification case AR-VER-101.
-/

namespace Arare

/-- The finite range of a width with half-range `M`. -/
def inRange (M k : Int) : Prop := -(M - 2) ≤ k ∧ k ≤ M - 2

instance (M k : Int) : Decidable (inRange M k) := by unfold inRange; infer_instance

/-- A value a width with half-range `M` can hold. -/
def valid (M : Int) : W → Prop
  | .fin k => inRange M k
  | _ => True

/-- AR-HLR-002: the code of a value. -/
def encode (M : Int) : W → Int
  | .fin k => k
  | .inf => M - 1
  | .bot => -M

/-- AR-HLR-002: the value of a code. -/
def decode (M : Int) (c : Int) : W :=
  if c = -M then .bot else if c = M - 1 then .inf else .fin c

/-- AR-HLR-002: every value a width holds comes back from its code. -/
theorem decode_encode (M : Int) (hM : 2 ≤ M) (x : W) (hx : valid M x) :
    decode M (encode M x) = x := by
  cases x with
  | fin k =>
    simp only [valid, inRange] at hx
    have h1 : ¬ k = -M := by omega
    have h2 : ¬ k = M - 1 := by omega
    simp [encode, decode, h1, h2]
  | inf =>
    have h : ¬ M - 1 = -M := by omega
    simp [encode, decode, h]
  | bot => simp [encode, decode]

/-- AR-HLR-002: every code lies in the `N`-bit code space `[−M, M−1]` and is
never the unused code `−(M−1)`. -/
theorem encode_range (M : Int) (hM : 2 ≤ M) (x : W) (hx : valid M x) :
    -M ≤ encode M x ∧ encode M x ≤ M - 1 ∧ encode M x ≠ -(M - 1) := by
  cases x <;> simp_all [valid, inRange, encode] <;> omega

/-- AR-HLR-002: every code but the unused one is the code of a value. -/
theorem encode_decode (M : Int) (hM : 2 ≤ M) (c : Int)
    (hc : -M ≤ c ∧ c ≤ M - 1) (hu : c ≠ -(M - 1)) :
    valid M (decode M c) ∧ encode M (decode M c) = c := by
  by_cases h1 : c = -M
  · simp [decode, h1, valid, encode]
  · by_cases h2 : c = M - 1
    · have h : ¬ M - 1 = -M := by omega
      simp [decode, h2, h, valid, encode]
    · simp only [decode, h1, h2, ite_false, valid, inRange, encode, and_true]
      omega

/-- AR-HLR-002: the finite range is symmetric, so negation stays inside it. -/
theorem neg_valid (M : Int) (x : W) (hx : valid M x) : valid M (W.neg x) := by
  cases x <;> simp_all [valid, inRange, W.neg] <;> omega

/-- The int8 codes, for the record: `0/0` is −128 and `1/0` is 127. -/
example : encode 128 .bot = -128 := rfl
example : encode 128 .inf = 127 := rfl

/-- AR-HLR-004, AR-HLR-009: fitting a value into a width. A finite number
outside the range becomes `1/0`; nothing wraps around. -/
def fit (M : Int) : W → W
  | .fin k => if inRange M k then .fin k else .inf
  | x => x

theorem fit_valid (M : Int) (x : W) : valid M (fit M x) := by
  cases x with
  | fin k => by_cases h : inRange M k <;> simp [fit, valid, h]
  | inf => simp [fit, valid]
  | bot => simp [fit, valid]

theorem fit_of_valid (M : Int) (x : W) (hx : valid M x) : fit M x = x := by
  cases x <;> simp_all [fit, valid]

theorem fit_idem (M : Int) (x : W) : fit M (fit M x) = fit M x :=
  fit_of_valid M _ (fit_valid M x)

/-- AR-HLR-004: overflow is never silent. Whatever `fit` answers is either the
exact value or `1/0`. -/
theorem fit_exact_or_inf (M : Int) (x : W) : fit M x = x ∨ fit M x = .inf := by
  cases x with
  | fin k => by_cases h : inRange M k <;> simp [fit, h]
  | inf => simp [fit]
  | bot => simp [fit]

/-- `NARROW` to a width with half-range `M` (AR-HLR-009) is `fit`. -/
def narrow (M : Int) (x : W) : W := fit M x

end Arare
