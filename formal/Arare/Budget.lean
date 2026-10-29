import Arare.Codes

/-!
# The error budget

What Phase 3's checker infers, and the theorem that it is sound
(WORK_ORDER.md §5.4: the most important theorem of the model).

A program's numbers have two meanings. `evalX` is the exact one, the value
Ajisai computes on the same source: rationals and the two wheel points, with
QUANTIZE, RESCALE and NARROW keeping the exact value. `evalM` is what an Arare
machine computes: codes at a static scale, rounded where Arare rounds (literal
quantization, `DIV`, `RESCALE`, `QUANTIZE`) and fitted where Arare fits (the
widest width, `NARROW`, `QUANTIZE`). `analyze` reads the expression alone,
child by child, and answers an `Info`: nothing, a wheel point both meanings
reach, or a range for the exact value with a bound on the machine's error.

`budget_sound` says the answer is never wrong (AR-HLR-027). `closed_safe`
says what the `field=closed` contract the checker derives from it guarantees
(AR-HLR-028): no wheel point anywhere, so no overflow, and no division by
zero. `join_sound_left` and `join_sound_right` say a join is right about
either side, which is how the checker carries a number through the lanes of
a vector and the iterations of a loop (AR-HLR-029).

The Rust checker (`rust/crates/arare-budget`, AR-LLR-019) computes each
function here under the same name, and prints the same table as
`formal/BudgetTable.lean`, which CI diffs against it.

The machine fits arithmetic results into the widest width only, as
`Closed.lean` does; that the kernel's narrower result widths never change a
value is tested, not proved here (docs/quality/FORMAL_VERIFICATION.md).

Verification case AR-VER-107.
-/

-- The case-split proofs below hand `simp` one lemma set for every case; not
-- every case uses every lemma.
set_option linter.unusedSimpArgs false

namespace Arare
namespace Budget

/-! ## Rational helpers -/

/-- `2^s` for an integer exponent: the value of one unit at scale `s`. -/
abbrev pow2 (s : Int) : Rat := (2 : Rat) ^ s

def rmax (a b : Rat) : Rat := if a ≤ b then b else a
def rmin (a b : Rat) : Rat := if a ≤ b then a else b

/-- The absolute value. -/
def rabs (a : Rat) : Rat := rmax (-a) a

theorem two_ne_zero : (2 : Rat) ≠ 0 := by decide

theorem pow2_pos (s : Int) : 0 < pow2 s := Rat.zpow_pos (by decide)

theorem pow2_add (a b : Int) : pow2 (a + b) = pow2 a * pow2 b := Rat.zpow_add two_ne_zero a b

theorem pow2_nat (n : Nat) : pow2 (n : Int) = ((2 ^ n : Int) : Rat) := by
  rw [pow2, Rat.zpow_natCast, Rat.intCast_pow]; rfl

theorem pow2_mono {a b : Int} (h : a ≤ b) : pow2 a ≤ pow2 b := by
  have e : b = a + ((b - a).toNat : Int) := by omega
  rw [e, pow2_add, pow2_nat]
  have h0 : 1 ≤ 2 ^ (b - a).toNat := Nat.one_le_two_pow
  have h1 : (1 : Int) ≤ 2 ^ (b - a).toNat := by exact_mod_cast h0
  have h2 : (1 : Rat) ≤ ((2 ^ (b - a).toNat : Int) : Rat) := by exact_mod_cast h1
  have h3 := pow2_pos a
  have h4 := Rat.mul_le_mul_of_nonneg_left h2 (Rat.le_of_lt h3)
  grind

/-- A code shifted left by `d` bits keeps its value at the scale `d` lower. -/
theorem shift_val (k : Int) (d : Nat) (s : Int) :
    ((k * 2 ^ d : Int) : Rat) * pow2 s = (k : Rat) * pow2 (s + d) := by
  rw [pow2_add, pow2_nat, Rat.intCast_mul]; grind

/-- `x·c` lies between `l·c` and `h·c` for `x` between `l` and `h`. -/
theorem mul_between {x l h : Rat} (c : Rat) (hl : l ≤ x) (hh : x ≤ h) :
    rmin (l * c) (h * c) ≤ x * c ∧ x * c ≤ rmax (l * c) (h * c) := by
  unfold rmin rmax
  by_cases hc : 0 ≤ c
  · have h1 := Rat.mul_nonneg (by grind : (0 : Rat) ≤ x - l) hc
    have h2 := Rat.mul_nonneg (by grind : (0 : Rat) ≤ h - x) hc
    grind
  · have h1 := Rat.mul_nonneg (by grind : (0 : Rat) ≤ x - l) (by grind : (0 : Rat) ≤ -c)
    have h2 := Rat.mul_nonneg (by grind : (0 : Rat) ≤ h - x) (by grind : (0 : Rat) ≤ -c)
    grind

/-- The product of two intervals. -/
def imul (l1 h1 l2 h2 : Rat) : Rat × Rat :=
  (rmin (rmin (l1 * l2) (l1 * h2)) (rmin (h1 * l2) (h1 * h2)),
   rmax (rmax (l1 * l2) (l1 * h2)) (rmax (h1 * l2) (h1 * h2)))

theorem imul_mem {x y l1 h1 l2 h2 : Rat} (hx1 : l1 ≤ x) (hx2 : x ≤ h1) (hy1 : l2 ≤ y) (hy2 : y ≤ h2) :
    (imul l1 h1 l2 h2).1 ≤ x * y ∧ x * y ≤ (imul l1 h1 l2 h2).2 := by
  have a := mul_between y hx1 hx2
  have b := mul_between l1 hy1 hy2
  have c := mul_between h1 hy1 hy2
  have e1 : l1 * y = y * l1 := Rat.mul_comm _ _
  have e2 : h1 * y = y * h1 := Rat.mul_comm _ _
  simp only [imul, rmin, rmax] at *
  grind [Rat.mul_comm]

/-- `|u·v| ≤ U·V` from `|u| ≤ U` and `|v| ≤ V`, two-sided. -/
theorem mul_abs_le {u v U V : Rat} (hu1 : -U ≤ u) (hu2 : u ≤ U) (hv1 : -V ≤ v) (hv2 : v ≤ V) :
    -(U * V) ≤ u * v ∧ u * v ≤ U * V := by
  have h1 := Rat.mul_nonneg (by grind : (0 : Rat) ≤ U - u) (by grind : (0 : Rat) ≤ V + v)
  have h2 := Rat.mul_nonneg (by grind : (0 : Rat) ≤ U + u) (by grind : (0 : Rat) ≤ V - v)
  have h3 := Rat.mul_nonneg (by grind : (0 : Rat) ≤ U - u) (by grind : (0 : Rat) ≤ V - v)
  have h4 := Rat.mul_nonneg (by grind : (0 : Rat) ≤ U + u) (by grind : (0 : Rat) ≤ V + v)
  grind


/-! ## The exact meaning: Ajisai's values -/

/-- The values Ajisai computes on: rationals and the two wheel points. -/
inductive X where
  | fin (q : Rat)
  | inf
  | bot

namespace X

def add : X → X → X
  | bot, _ => bot
  | _, bot => bot
  | inf, inf => bot
  | inf, fin _ => inf
  | fin _, inf => inf
  | fin a, fin b => fin (a + b)

def neg : X → X
  | fin a => fin (-a)
  | inf => inf
  | bot => bot

def sub (x y : X) : X := add x (neg y)

def mul : X → X → X
  | bot, _ => bot
  | _, bot => bot
  | inf, inf => inf
  | inf, fin b => if b = 0 then bot else inf
  | fin a, inf => if a = 0 then bot else inf
  | fin a, fin b => fin (a * b)

def div : X → X → X
  | bot, _ => bot
  | _, bot => bot
  | inf, inf => bot
  | inf, fin _ => inf
  | fin _, inf => fin 0
  | fin a, fin b => if b = 0 then (if a = 0 then bot else inf) else fin (a / b)

@[simp] theorem add_bot (x : X) : add x bot = bot := by cases x <;> rfl
@[simp] theorem mul_bot (x : X) : mul x bot = bot := by cases x <;> rfl
@[simp] theorem div_bot (x : X) : div x bot = bot := by cases x <;> rfl

end X

@[simp] theorem W_add_bot (x : W) : W.add x .bot = .bot := by cases x <;> rfl
@[simp] theorem W_mul_bot (x : W) : W.mul x .bot = .bot := by cases x <;> rfl
@[simp] theorem W_div_bot (x : W) : W.div x .bot = .bot := by cases x <;> rfl

/-! ## Programs and their two meanings -/

/-- The numeric Words of a program, as an expression. A literal carries its
exact value and the width (half-range `M`) and exponent it is quantized at; an
integer literal is `lit (.fin k) M 0`. -/
inductive E where
  | lit (x : X) (M e : Int)
  | add (a b : E)
  | sub (a b : E)
  | mul (a b : E)
  | neg (a : E)
  | div (a b : E)
  | rescale (a : E) (s : Nat)
  | narrow (a : E) (M : Int)
  | quantize (a : E) (M e : Int)

namespace E

/-- The static scale exponent of a result (AR-HLR-003, 007, 008, 026). -/
def scale : E → Int
  | lit _ _ e => e
  | add a b => min (scale a) (scale b)
  | sub a b => min (scale a) (scale b)
  | mul a b => scale a + scale b
  | neg a => scale a
  | div a _ => scale a
  | rescale a s => scale a + s
  | narrow a _ => scale a
  | quantize _ _ e => e

/-- The exact value, as Ajisai computes it: QUANTIZE, RESCALE and NARROW
keep the exact value (ADR-0007). -/
def evalX : E → X
  | lit x _ _ => x
  | add a b => X.add (evalX a) (evalX b)
  | sub a b => X.sub (evalX a) (evalX b)
  | mul a b => X.mul (evalX a) (evalX b)
  | neg a => X.neg (evalX a)
  | div a b => X.div (evalX a) (evalX b)
  | rescale a _ => evalX a
  | narrow a _ => evalX a
  | quantize a _ _ => evalX a

end E

/-- A rational quantized at exponent `e`: `q·2^(−e)` rounded half to even,
as `Num::from_ratio` computes it (AR-HLR-026). -/
def roundQ (q : Rat) (e : Int) : Int :=
  if e ≤ 0 then roundDiv (q.num * 2 ^ (-e).toNat) q.den
  else roundDiv q.num (q.den * 2 ^ e.toNat)

/-- A code shifted left by `d` bits; the points are unchanged. -/
def shiftW : W → Nat → W
  | .fin k, d => .fin (k * 2 ^ d)
  | p, _ => p

/-- A code rescaled by `s` bits; the points are unchanged. -/
def rescaleW : W → Nat → W
  | .fin k, s => .fin (rescale k s)
  | p, _ => p

namespace E

/-- The machine's code, at the static scale `scale`: the kernel's arithmetic
(AR-LLR-004 to 006, 010), with every arithmetic result fitted into the widest
width, of half-range `Mw`. -/
def evalM (Mw : Int) : E → W
  | lit x M e =>
    match x with
    | .fin q => fit M (.fin (roundQ q e))
    | .inf => .inf
    | .bot => .bot
  | add a b =>
    fit Mw (W.add (shiftW (evalM Mw a) (scale a - min (scale a) (scale b)).toNat)
      (shiftW (evalM Mw b) (scale b - min (scale a) (scale b)).toNat))
  | sub a b =>
    fit Mw (W.add (shiftW (evalM Mw a) (scale a - min (scale a) (scale b)).toNat)
      (shiftW (W.neg (evalM Mw b)) (scale b - min (scale a) (scale b)).toNat))
  | mul a b => fit Mw (W.mul (evalM Mw a) (evalM Mw b))
  | neg a => W.neg (evalM Mw a)
  | div a b =>
    fit Mw (W.div (shiftW (evalM Mw a) (-(scale b)).toNat) (shiftW (evalM Mw b) (scale b).toNat))
  | rescale a s => rescaleW (evalM Mw a) s
  | narrow a M => fit M (evalM Mw a)
  | quantize a M e =>
    if scale a ≤ e then fit M (rescaleW (evalM Mw a) (e - scale a).toNat)
    else fit M (shiftW (evalM Mw a) (scale a - e).toNat)

end E

/-! ## What the checker knows -/

/-- What the checker knows of a number. `num lo hi err smin smax`: both
meanings are finite, the exact value lies in `[lo, hi]`, the machine's value
is within `err` of it, and the scale lies in `[smin, smax]`. -/
inductive Info where
  | top
  | inf
  | bot
  | num (lo hi err : Rat) (smin smax : Int)

/-- What it means for an `Info` to be right about an exact value `x` and a
machine code `m` at scale `s`. -/
def Sound : Info → X → W → Int → Prop
  | .top, _, _, _ => True
  | .inf, x, m, _ => x = .inf ∧ m = .inf
  | .bot, x, m, _ => x = .bot ∧ m = .bot
  | .num lo hi err smin smax, x, m, s =>
    smin ≤ s ∧ s ≤ smax ∧ ∃ q k, x = .fin q ∧ m = .fin k ∧ lo ≤ q ∧ q ≤ hi ∧
      -err ≤ (k : Rat) * pow2 s - q ∧ (k : Rat) * pow2 s - q ≤ err

/-- A bound on the magnitude of the exact value. -/
def mag (lo hi : Rat) : Rat := rmax (-lo) hi

/-- A bound on the magnitude of the machine's value. -/
def mmag (lo hi err : Rat) : Rat := rmax (-(lo - err)) (hi + err)

/-- Surely nonzero, in both meanings. -/
def nonzero (lo hi err : Rat) : Bool := decide (0 < lo - err) || decide (hi + err < 0)

/-- Surely zero, in both meanings. -/
def isZero (lo hi err : Rat) : Bool := decide (lo = 0) && decide (hi = 0) && decide (err = 0)

/-- A lower bound on the magnitude of a value surely nonzero, in both meanings. -/
def low (lo hi err : Rat) : Rat := if 0 < lo - err then lo - err else -(hi + err)

/-- Fitting into a width of half-range `M`: what is known survives when the
machine's magnitude provably fits at the smallest scale; a point passes. -/
def guard (M : Int) : Info → Info
  | .num lo hi err smin smax =>
    if mmag lo hi err ≤ ((M - 2 : Int) : Rat) * pow2 smin then .num lo hi err smin smax else .top
  | i => i

def alit : X → Int → Int → Info
  | .fin q, M, e => guard M (.num q q (rabs ((roundQ q e : Rat) * pow2 e - q)) e e)
  | .inf, _, _ => .inf
  | .bot, _, _ => .bot

def aneg : Info → Info
  | .num lo hi err smin smax => .num (-hi) (-lo) err smin smax
  | i => i

/-- ADD before the fit: the ranges and the errors add; alignment is exact. -/
def padd : Info → Info → Info
  | .bot, _ => .bot
  | _, .bot => .bot
  | .inf, .inf => .bot
  | .inf, .num .. => .inf
  | .num .., .inf => .inf
  | .num l1 h1 e1 a1 b1, .num l2 h2 e2 a2 b2 =>
    .num (l1 + l2) (h1 + h2) (e1 + e2) (min a1 a2) (min b1 b2)
  | _, _ => .top

/-- MUL before the fit: `|ab − xy| ≤ |a|·|b − y| + |y|·|a − x|`. -/
def pmul : Info → Info → Info
  | .bot, _ => .bot
  | _, .bot => .bot
  | .inf, .inf => .inf
  | .inf, .num l h e _ _ => if nonzero l h e then .inf else if isZero l h e then .bot else .top
  | .num l h e _ _, .inf => if nonzero l h e then .inf else if isZero l h e then .bot else .top
  | .num l1 h1 e1 a1 b1, .num l2 h2 e2 a2 b2 =>
    .num (imul l1 h1 l2 h2).1 (imul l1 h1 l2 h2).2 ((mag l1 h1 + e1) * e2 + mag l2 h2 * e1)
      (a1 + a2) (b1 + b2)
  | _, _ => .top

/-- DIV before the fit: half a unit of the dividend's scale for the rounding,
plus `|a/b − x/y| ≤ |a − x|/|b| + |x|·|y − b|/(|b|·|y|)`; a divisor that may
be zero gives nothing. -/
def pdiv : Info → Info → Info
  | .bot, _ => .bot
  | _, .bot => .bot
  | .inf, .inf => .bot
  | .inf, .num .. => .inf
  | .num _ _ _ a1 b1, .inf => .num 0 0 0 a1 b1
  | .num l1 h1 e1 a1 b1, .num l2 h2 e2 _ _ =>
    if isZero l2 h2 e2 then
      (if nonzero l1 h1 e1 then .inf else if isZero l1 h1 e1 then .bot else .top)
    else if nonzero l2 h2 e2 then
      .num (imul l1 h1 h2⁻¹ l2⁻¹).1 (imul l1 h1 h2⁻¹ l2⁻¹).2
        (pow2 b1 / 2 + e1 * (low l2 h2 e2)⁻¹ + mag l1 h1 * e2 * ((low l2 h2 e2)⁻¹ * (low l2 h2 e2)⁻¹))
        a1 b1
    else .top
  | _, _ => .top

def arescale : Info → Nat → Info
  | .num l h e a b, s => .num l h (e + pow2 (b + s) / 2) (a + s) (b + s)
  | i, _ => i

def aquantize (M e : Int) : Info → Info
  | .num l h r a _ => guard M (.num l h (r + if a < e then pow2 e / 2 else 0) e e)
  | i => i

/-- The checker on an expression: child by child, never looking at a value
the expression computes. -/
def analyze (Mw : Int) : E → Info
  | .lit x M e => alit x M e
  | .add a b => guard Mw (padd (analyze Mw a) (analyze Mw b))
  | .sub a b => guard Mw (padd (analyze Mw a) (aneg (analyze Mw b)))
  | .mul a b => guard Mw (pmul (analyze Mw a) (analyze Mw b))
  | .neg a => aneg (analyze Mw a)
  | .div a b => guard Mw (pdiv (analyze Mw a) (analyze Mw b))
  | .rescale a s => arescale (analyze Mw a) s
  | .narrow a M => guard M (analyze Mw a)
  | .quantize a M e => aquantize M e (analyze Mw a)


/-! ## Soundness, one Word at a time -/

/-- An `Info` that is right bounds the machine's magnitude. -/
theorem mmag_bound {lo hi err q : Rat} {v : Rat} (h1 : lo ≤ q) (h2 : q ≤ hi)
    (h3 : -err ≤ v - q) (h4 : v - q ≤ err) : -(mmag lo hi err) ≤ v ∧ v ≤ mmag lo hi err := by
  unfold mmag rmax; grind

theorem guard_sound (M : Int) {i : Info} {x : X} {m : W} {s : Int} (h : Sound i x m s) :
    Sound (guard M i) x (fit M m) s := by
  cases i with
  | top => trivial
  | inf => obtain ⟨rfl, rfl⟩ := h; exact ⟨rfl, rfl⟩
  | bot => obtain ⟨rfl, rfl⟩ := h; exact ⟨rfl, rfl⟩
  | num lo hi err smin smax =>
    obtain ⟨hs1, hs2, q, k, rfl, rfl, hq1, hq2, he1, he2⟩ := h
    simp only [guard]
    split
    · rename_i hfit
      have hb := mmag_bound hq1 hq2 he1 he2
      have hp := pow2_pos s
      have hmono := pow2_mono hs1
      -- |k|·2^s ≤ (M − 2)·2^smin ≤ (M − 2)·2^s, so |k| ≤ M − 2.
      have hM : (0 : Rat) ≤ ((M - 2 : Int) : Rat) := by
        have := pow2_pos smin
        by_cases h0 : (0 : Rat) ≤ ((M - 2 : Int) : Rat)
        · exact h0
        · exfalso
          have := Rat.mul_lt_mul_of_pos_right (by grind : ((M - 2 : Int) : Rat) < 0) this
          grind
      have hle := Rat.mul_le_mul_of_nonneg_left hmono hM
      have hk1 : (k : Rat) * pow2 s ≤ ((M - 2 : Int) : Rat) * pow2 s := by grind
      have hk2 : -(k : Rat) * pow2 s ≤ ((M - 2 : Int) : Rat) * pow2 s := by grind
      have r1 := Rat.le_of_mul_le_mul_right hk1 hp
      have r2 := Rat.le_of_mul_le_mul_right hk2 hp
      have i1 : k ≤ M - 2 := by exact_mod_cast r1
      have i2 : -k ≤ M - 2 := by
        have : ((-k : Int) : Rat) ≤ ((M - 2 : Int) : Rat) := by rw [Rat.intCast_neg]; exact r2
        exact_mod_cast this
      have hr : inRange M k := ⟨by omega, i1⟩
      simp only [fit, hr, ite_true]
      exact ⟨hs1, hs2, q, k, rfl, rfl, hq1, hq2, he1, he2⟩
    · trivial

theorem aneg_sound {i : Info} {x : X} {m : W} {s : Int} (h : Sound i x m s) :
    Sound (aneg i) (X.neg x) (W.neg m) s := by
  cases i with
  | top => trivial
  | inf => obtain ⟨rfl, rfl⟩ := h; exact ⟨rfl, rfl⟩
  | bot => obtain ⟨rfl, rfl⟩ := h; exact ⟨rfl, rfl⟩
  | num lo hi err smin smax =>
    obtain ⟨hs1, hs2, q, k, rfl, rfl, hq1, hq2, he1, he2⟩ := h
    refine ⟨hs1, hs2, -q, -k, rfl, rfl, ?_, ?_, ?_, ?_⟩ <;> push_cast <;> grind


theorem padd_sound {ia ib : Info} {xa xb : X} {ma mb : W} {sa sb : Int}
    (ha : Sound ia xa ma sa) (hb : Sound ib xb mb sb) :
    Sound (padd ia ib) (X.add xa xb)
      (W.add (shiftW ma (sa - min sa sb).toNat) (shiftW mb (sb - min sa sb).toNat)) (min sa sb) := by
  cases ia with
  | num l1 h1 e1 a1 b1 =>
    cases ib with
    | num l2 h2 e2 a2 b2 =>
      obtain ⟨hs1, hs2, q1, k1, rfl, rfl, hq1, hq2, he1, he2⟩ := ha
      obtain ⟨hs3, hs4, q2, k2, rfl, rfl, hq3, hq4, he3, he4⟩ := hb
      refine ⟨by omega, by omega, q1 + q2, _, rfl, rfl, by grind, by grind, ?_, ?_⟩ <;>
      · have v1 := shift_val k1 (sa - min sa sb).toNat (min sa sb)
        have v2 := shift_val k2 (sb - min sa sb).toNat (min sa sb)
        have c1 : min sa sb + ((sa - min sa sb).toNat : Int) = sa := by omega
        have c2 : min sa sb + ((sb - min sa sb).toNat : Int) = sb := by omega
        rw [c1] at v1; rw [c2] at v2
        rw [Rat.intCast_add]; grind
    | top => trivial
    | inf =>
      obtain ⟨_, _, _, _, rfl, rfl, _⟩ := ha; obtain ⟨rfl, rfl⟩ := hb
      simp [padd, Sound, X.add, W.add, shiftW]
    | bot => obtain ⟨rfl, rfl⟩ := hb; simp [padd, Sound, shiftW]
  | top => cases ib <;> first | trivial | (obtain ⟨rfl, rfl⟩ := hb; simp [padd, Sound, shiftW])
  | bot => obtain ⟨rfl, rfl⟩ := ha; simp [padd, Sound, X.add, W.add, shiftW]
  | inf =>
    obtain ⟨rfl, rfl⟩ := ha
    cases ib with
    | top => trivial
    | inf => obtain ⟨rfl, rfl⟩ := hb; simp [padd, Sound, X.add, W.add, shiftW]
    | bot => obtain ⟨rfl, rfl⟩ := hb; simp [padd, Sound, shiftW]
    | num => obtain ⟨_, _, _, _, rfl, rfl, _⟩ := hb; simp [padd, Sound, X.add, W.add, shiftW]


/-- What `nonzero` promises: neither meaning is zero. -/
theorem nonzero_ne {l h e q : Rat} {k s : Int} (hq1 : l ≤ q) (hq2 : q ≤ h)
    (he1 : -e ≤ (k : Rat) * pow2 s - q) (he2 : (k : Rat) * pow2 s - q ≤ e)
    (hnz : nonzero l h e = true) : q ≠ 0 ∧ k ≠ 0 := by
  have hp := pow2_pos s
  simp only [nonzero, Bool.or_eq_true, decide_eq_true_eq] at hnz
  refine ⟨by grind, fun hk => ?_⟩
  subst hk
  simp at he1 he2
  grind

/-- What `isZero` promises: both meanings are zero. -/
theorem isZero_eq {l h e q : Rat} {k s : Int} (hq1 : l ≤ q) (hq2 : q ≤ h)
    (he1 : -e ≤ (k : Rat) * pow2 s - q) (he2 : (k : Rat) * pow2 s - q ≤ e)
    (hz : isZero l h e = true) : q = 0 ∧ k = 0 := by
  have hp := pow2_pos s
  simp only [isZero, Bool.and_eq_true, decide_eq_true_eq] at hz
  obtain ⟨⟨rfl, rfl⟩, rfl⟩ := hz
  have hq : q = 0 := by grind
  subst hq
  refine ⟨rfl, ?_⟩
  have h0 : (k : Rat) * pow2 s = 0 := by grind
  rcases Rat.mul_eq_zero.mp h0 with h | h
  · exact_mod_cast h
  · grind

theorem pmul_sound {ia ib : Info} {xa xb : X} {ma mb : W} {sa sb : Int}
    (ha : Sound ia xa ma sa) (hb : Sound ib xb mb sb) :
    Sound (pmul ia ib) (X.mul xa xb) (W.mul ma mb) (sa + sb) := by
  cases ia with
  | num l1 h1 e1 a1 b1 =>
    cases ib with
    | num l2 h2 e2 a2 b2 =>
      obtain ⟨hs1, hs2, q1, k1, rfl, rfl, hq1, hq2, he1, he2⟩ := ha
      obtain ⟨hs3, hs4, q2, k2, rfl, rfl, hq3, hq4, he3, he4⟩ := hb
      have hr := imul_mem hq1 hq2 hq3 hq4
      -- ab − xy = a·(b − y) + y·(a − x)
      have v : ((k1 * k2 : Int) : Rat) * pow2 (sa + sb) = ((k1 : Rat) * pow2 sa) * ((k2 : Rat) * pow2 sb) := by
        rw [pow2_add, Rat.intCast_mul]; grind
      have m1 := mul_abs_le (u := (k1 : Rat) * pow2 sa) (v := (k2 : Rat) * pow2 sb - q2)
        (U := mag l1 h1 + e1) (V := e2) (by unfold mag rmax; grind) (by unfold mag rmax; grind) he3 he4
      have m2 := mul_abs_le (u := q2) (v := (k1 : Rat) * pow2 sa - q1)
        (U := mag l2 h2) (V := e1) (by unfold mag rmax; grind) (by unfold mag rmax; grind) he1 he2
      refine ⟨by omega, by omega, q1 * q2, k1 * k2, rfl, rfl, hr.1, hr.2, ?_, ?_⟩ <;> rw [v] <;> grind
    | top => trivial
    | bot => obtain ⟨rfl, rfl⟩ := hb; simp [pmul, Sound]
    | inf =>
      obtain ⟨_, _, q, k, rfl, rfl, hq1, hq2, he1, he2⟩ := ha; obtain ⟨rfl, rfl⟩ := hb
      simp only [pmul]
      split
      · rename_i hnz
        obtain ⟨hq, hk⟩ := nonzero_ne hq1 hq2 he1 he2 hnz
        simp [Sound, X.mul, W.mul, hq, hk]
      · split
        · rename_i _ hz
          obtain ⟨rfl, rfl⟩ := isZero_eq hq1 hq2 he1 he2 hz
          simp [Sound, X.mul, W.mul]
        · trivial
  | top => cases ib <;> first | trivial | (obtain ⟨rfl, rfl⟩ := hb; simp [pmul, Sound])
  | bot => obtain ⟨rfl, rfl⟩ := ha; simp [pmul, Sound, X.mul, W.mul]
  | inf =>
    obtain ⟨rfl, rfl⟩ := ha
    cases ib with
    | top => trivial
    | inf => obtain ⟨rfl, rfl⟩ := hb; simp [pmul, Sound, X.mul, W.mul]
    | bot => obtain ⟨rfl, rfl⟩ := hb; simp [pmul, Sound]
    | num =>
      obtain ⟨_, _, q, k, rfl, rfl, hq1, hq2, he1, he2⟩ := hb
      simp only [pmul]
      split
      · rename_i hnz
        obtain ⟨hq, hk⟩ := nonzero_ne hq1 hq2 he1 he2 hnz
        simp [Sound, X.mul, W.mul, hq, hk]
      · split
        · rename_i _ hz
          obtain ⟨rfl, rfl⟩ := isZero_eq hq1 hq2 he1 he2 hz
          simp [Sound, X.mul, W.mul]
        · trivial


/-- `|1/b| ≤ 1/L` when `|b| ≥ L > 0`. -/
theorem inv_abs_le {b L : Rat} (hL : 0 < L) (hb : L ≤ b ∨ b ≤ -L) :
    -(L⁻¹) ≤ b⁻¹ ∧ b⁻¹ ≤ L⁻¹ := by
  have hb0 : b ≠ 0 := by grind
  have i1 := Rat.mul_inv_cancel b hb0
  have i2 := Rat.mul_inv_cancel L (by grind)
  have hLi : 0 < L⁻¹ := Rat.inv_pos.mpr hL
  -- (b − L)·b⁻¹·L⁻¹ = L⁻¹ − b⁻¹
  have e : (b - L) * b⁻¹ * L⁻¹ = L⁻¹ - b⁻¹ := by
    have : (b - L) * b⁻¹ * L⁻¹ = (b * b⁻¹) * L⁻¹ - (L * L⁻¹) * b⁻¹ := by grind
    rw [this, i1, i2]; grind
  -- (b + L)·b⁻¹·L⁻¹ = L⁻¹ + b⁻¹
  have e' : (b + L) * b⁻¹ * L⁻¹ = L⁻¹ + b⁻¹ := by
    have : (b + L) * b⁻¹ * L⁻¹ = (b * b⁻¹) * L⁻¹ + (L * L⁻¹) * b⁻¹ := by grind
    rw [this, i1, i2]; grind
  rcases hb with hb | hb
  · have hbi : 0 < b⁻¹ := Rat.inv_pos.mpr (by grind)
    have h1 := Rat.mul_nonneg (Rat.mul_nonneg (by grind : (0 : Rat) ≤ b - L) (Rat.le_of_lt hbi)) (Rat.le_of_lt hLi)
    grind
  · have hbi : b⁻¹ < 0 := by
      have : 0 < (-b)⁻¹ := Rat.inv_pos.mpr (by grind)
      have i3 := Rat.mul_inv_cancel (-b) (by grind)
      have : (-b)⁻¹ = -b⁻¹ := by
        have : (-b)⁻¹ = (-b)⁻¹ * (b * b⁻¹) := by rw [i1]; grind
        rw [this]
        have : (-b)⁻¹ * (b * b⁻¹) = -(((-b) * (-b)⁻¹) * b⁻¹) := by grind
        rw [this, i3]; grind
      grind
    have h1 := Rat.mul_nonneg (Rat.mul_nonneg (by grind : (0 : Rat) ≤ -(b + L)) (by grind : (0 : Rat) ≤ -b⁻¹)) (Rat.le_of_lt hLi)
    have : -(b + L) * -b⁻¹ * L⁻¹ = (b + L) * b⁻¹ * L⁻¹ := by grind
    grind

/-- `1/y` lies in `[1/h, 1/l]` for `y` in `[l, h]` away from zero. -/
theorem inv_between {y l h : Rat} (hy1 : l ≤ y) (hy2 : y ≤ h) (hs : 0 < l ∨ h < 0) :
    h⁻¹ ≤ y⁻¹ ∧ y⁻¹ ≤ l⁻¹ := by
  have i1 := Rat.mul_inv_cancel y (by grind)
  have i2 := Rat.mul_inv_cancel l (by grind)
  have i3 := Rat.mul_inv_cancel h (by grind)
  have e1 : (y - l) * (y⁻¹ * l⁻¹) = l⁻¹ - y⁻¹ := by
    have : (y - l) * (y⁻¹ * l⁻¹) = (y * y⁻¹) * l⁻¹ - (l * l⁻¹) * y⁻¹ := by grind
    rw [this, i1, i2]; grind
  have e2 : (h - y) * (y⁻¹ * h⁻¹) = y⁻¹ - h⁻¹ := by
    have : (h - y) * (y⁻¹ * h⁻¹) = (h * h⁻¹) * y⁻¹ - (y * y⁻¹) * h⁻¹ := by grind
    rw [this, i1, i3]; grind
  -- y⁻¹·l⁻¹ and y⁻¹·h⁻¹ are positive: y, l and h share a sign.
  have pos : ∀ u v : Rat, u ≠ 0 → (0 < u ∧ 0 < v ∨ u < 0 ∧ v < 0) → 0 ≤ u⁻¹ * v⁻¹ := by
    intro u v _ huv
    rcases huv with ⟨hu, hv⟩ | ⟨hu, hv⟩
    · exact Rat.mul_nonneg (Rat.le_of_lt (Rat.inv_pos.mpr hu)) (Rat.le_of_lt (Rat.inv_pos.mpr hv))
    · have a : 0 < (-u)⁻¹ := Rat.inv_pos.mpr (by grind)
      have b : 0 < (-v)⁻¹ := Rat.inv_pos.mpr (by grind)
      have c := Rat.mul_nonneg (Rat.le_of_lt a) (Rat.le_of_lt b)
      have ea : (-u)⁻¹ = -u⁻¹ := by
        rw [Rat.inv_def, Rat.inv_def]; simp [Rat.neg_divInt, Rat.divInt_neg']
      have eb : (-v)⁻¹ = -v⁻¹ := by
        rw [Rat.inv_def, Rat.inv_def]; simp [Rat.neg_divInt, Rat.divInt_neg']
      rw [ea, eb] at c; grind
  have p1 := pos y l (by grind) (by grind)
  have p2 := pos y h (by grind) (by grind)
  have a := Rat.mul_nonneg (by grind : (0 : Rat) ≤ y - l) p1
  have b := Rat.mul_nonneg (by grind : (0 : Rat) ≤ h - y) p2
  grind


/-- What `low` promises of a value `nonzero` accepts. -/
theorem low_bound {l h e q : Rat} {k s : Int} (hq1 : l ≤ q) (hq2 : q ≤ h)
    (he1 : -e ≤ (k : Rat) * pow2 s - q) (he2 : (k : Rat) * pow2 s - q ≤ e)
    (hnz : nonzero l h e = true) :
    0 < low l h e ∧ (low l h e ≤ q ∨ q ≤ -low l h e) ∧
      (low l h e ≤ (k : Rat) * pow2 s ∨ (k : Rat) * pow2 s ≤ -low l h e) ∧ (0 < l ∨ h < 0) := by
  simp only [nonzero, Bool.or_eq_true, decide_eq_true_eq] at hnz
  unfold low
  split <;> grind

/-- `|n/d − r| ≤ 1/2` from the rounding bound `2·|n − d·r| ≤ |d|`. -/
theorem half_unit {A B r : Int} (hB : B ≠ 0)
    (h : -(if 0 < B then B else -B) ≤ 2 * (A - B * r) ∧ 2 * (A - B * r) ≤ (if 0 < B then B else -B)) :
    -(1 / 2 : Rat) ≤ ((A : Rat) - B * r) * (B : Rat)⁻¹ ∧ ((A : Rat) - B * r) * (B : Rat)⁻¹ ≤ 1 / 2 := by
  have cB := Rat.mul_inv_cancel (B : Rat) (by exact_mod_cast hB)
  have e1 : (2 * ((A : Rat) - B * r)) * (B : Rat)⁻¹ = 2 * (((A : Rat) - B * r) * (B : Rat)⁻¹) := by grind
  by_cases hp : 0 < B
  · simp only [hp, ite_true] at h
    have h1 : (-(B : Rat)) ≤ 2 * ((A : Rat) - B * r) := by exact_mod_cast h.1
    have h2 : 2 * ((A : Rat) - B * r) ≤ B := by exact_mod_cast h.2
    have hi : 0 ≤ (B : Rat)⁻¹ := Rat.le_of_lt (Rat.inv_pos.mpr (by exact_mod_cast hp))
    have a := Rat.mul_le_mul_of_nonneg_right h2 hi
    have b := Rat.mul_le_mul_of_nonneg_right h1 hi
    have e2 : (-(B : Rat)) * (B : Rat)⁻¹ = -((B : Rat) * (B : Rat)⁻¹) := by grind
    rw [e1, cB] at a; rw [e1, e2, cB] at b
    grind
  · simp only [hp, ite_false] at h
    have hn : (B : Rat) < 0 := by exact_mod_cast (by omega : B < 0)
    have h1 : (B : Rat) ≤ 2 * ((A : Rat) - B * r) := by
      have := h.1; rw [Int.neg_neg] at this; exact_mod_cast this
    have h2 : 2 * ((A : Rat) - B * r) ≤ -(B : Rat) := by exact_mod_cast h.2
    have hi : 0 ≤ -(B : Rat)⁻¹ := by
      have : 0 < (-(B : Rat))⁻¹ := Rat.inv_pos.mpr (by grind)
      have e : (-(B : Rat))⁻¹ = -(B : Rat)⁻¹ := by
        rw [Rat.inv_def, Rat.inv_def]; simp [Rat.neg_divInt, Rat.divInt_neg']
      grind
    have a := Rat.mul_le_mul_of_nonneg_right h2 hi
    have b := Rat.mul_le_mul_of_nonneg_right h1 hi
    have e3 : (2 * ((A : Rat) - B * r)) * -(B : Rat)⁻¹ = -(2 * (((A : Rat) - B * r) * (B : Rat)⁻¹)) := by grind
    have e4 : -(B : Rat) * -(B : Rat)⁻¹ = (B : Rat) * (B : Rat)⁻¹ := by grind
    have e5 : (B : Rat) * -(B : Rat)⁻¹ = -((B : Rat) * (B : Rat)⁻¹) := by grind
    rw [e3, e4, cB] at a; rw [e3, e5, cB] at b
    grind

/-- `DIV`'s rounding: the quotient the kernel computes on shifted codes is
within half a unit of the dividend's scale of the quotient of the two values. -/
theorem div_round (k1 k2 sa sb : Int) (hk2 : k2 ≠ 0) :
    -(pow2 sa / 2) ≤ (roundDiv (k1 * 2 ^ (-sb).toNat) (k2 * 2 ^ sb.toNat) : Rat) * pow2 sa -
        ((k1 : Rat) * pow2 sa) / ((k2 : Rat) * pow2 sb) ∧
      (roundDiv (k1 * 2 ^ (-sb).toNat) (k2 * 2 ^ sb.toNat) : Rat) * pow2 sa -
        ((k1 : Rat) * pow2 sa) / ((k2 : Rat) * pow2 sb) ≤ pow2 sa / 2 := by
  generalize hp : (-sb).toNat = p
  generalize hq : sb.toNat = q
  have hpq : (q : Int) = p + sb := by omega
  have h2q : (2 : Int) ^ q ≠ 0 := by have : (0 : Int) < 2 ^ q := Int.pow_pos (by decide); omega
  have hd : k2 * 2 ^ q ≠ 0 := Int.mul_ne_zero hk2 h2q
  have herr := roundDiv_error (k1 * 2 ^ p) (k2 * 2 ^ q) hd
  generalize roundDiv (k1 * 2 ^ p) (k2 * 2 ^ q) = r at herr ⊢
  have eN : ((k1 * 2 ^ p : Int) : Rat) = (k1 : Rat) * pow2 p := by rw [pow2_nat, Rat.intCast_mul]
  have eD : ((k2 * 2 ^ q : Int) : Rat) = (k2 : Rat) * pow2 p * pow2 sb := by
    rw [Rat.intCast_mul, ← pow2_nat, hpq, pow2_add]; grind
  generalize k1 * 2 ^ p = A at herr eN
  generalize k2 * 2 ^ q = B at herr eD hd
  have hb := half_unit hd herr
  have hP := pow2_pos sa
  have hPp := pow2_pos p
  have hk2r : (k2 : Rat) ≠ 0 := by exact_mod_cast hk2
  have cB := Rat.mul_inv_cancel (B : Rat) (by exact_mod_cast hd)
  -- The quotient of the values is 2^sa·A/B.
  have eQ : ((k1 : Rat) * pow2 sa) / ((k2 : Rat) * pow2 sb) = pow2 sa * (A : Rat) * (B : Rat)⁻¹ := by
    rw [eN, eD, Rat.div_def]
    have c := Rat.mul_inv_cancel (pow2 p) (by grind)
    rw [Rat.inv_mul_rev, Rat.inv_mul_rev, Rat.inv_mul_rev]
    have : pow2 sa * (↑k1 * pow2 p) * ((pow2 sb)⁻¹ * ((pow2 p)⁻¹ * (↑k2)⁻¹)) =
        (↑k1 * pow2 sa) * ((pow2 sb)⁻¹ * (↑k2)⁻¹) * (pow2 p * (pow2 p)⁻¹) := by grind
    rw [this, c]; grind
  have eU : ((A : Rat) - B * r) * (B : Rat)⁻¹ = (A : Rat) * (B : Rat)⁻¹ - r := by
    have : ((A : Rat) - B * r) * (B : Rat)⁻¹ = (A : Rat) * (B : Rat)⁻¹ - ((B : Rat) * (B : Rat)⁻¹) * r := by grind
    rw [this, cB]; grind
  rw [eQ]
  rw [eU] at hb
  have m1 := Rat.mul_le_mul_of_nonneg_left hb.1 (Rat.le_of_lt hP)
  have m2 := Rat.mul_le_mul_of_nonneg_left hb.2 (Rat.le_of_lt hP)
  constructor <;> grind


/-- How an error in the operands carries through a quotient:
`|a/b − x/y| ≤ ea/L + A·eb/L²` when `|b|, |y| ≥ L > 0`. -/
theorem div_prop {a b x y ea eb A L : Rat} (hL : 0 < L) (hbL : L ≤ b ∨ b ≤ -L) (hyL : L ≤ y ∨ y ≤ -L)
    (ha1 : -ea ≤ a - x) (ha2 : a - x ≤ ea) (hb1 : -eb ≤ b - y) (hb2 : b - y ≤ eb)
    (hx1 : -A ≤ x) (hx2 : x ≤ A) :
    -(ea * L⁻¹ + A * eb * (L⁻¹ * L⁻¹)) ≤ a / b - x / y ∧
      a / b - x / y ≤ ea * L⁻¹ + A * eb * (L⁻¹ * L⁻¹) := by
  have ib := inv_abs_le hL hbL
  have iy := inv_abs_le hL hyL
  have i1 := Rat.mul_inv_cancel b (by grind)
  have i2 := Rat.mul_inv_cancel y (by grind)
  -- a/b − x/y = (a − x)·b⁻¹ + x·((y − b)·(b⁻¹·y⁻¹))
  have e : a / b - x / y = (a - x) * b⁻¹ + x * ((y - b) * (b⁻¹ * y⁻¹)) := by
    have : x * ((y - b) * (b⁻¹ * y⁻¹)) = x * ((y * y⁻¹) * b⁻¹) - x * ((b * b⁻¹) * y⁻¹) := by grind
    rw [this, i1, i2, Rat.div_def, Rat.div_def]; grind
  rw [e]
  have m1 := mul_abs_le ha1 ha2 ib.1 ib.2
  have m2 := mul_abs_le ib.1 ib.2 iy.1 iy.2
  have m3 := mul_abs_le (by grind : -eb ≤ y - b) (by grind : y - b ≤ eb) m2.1 m2.2
  have m4 := mul_abs_le hx1 hx2 m3.1 m3.2
  constructor <;> grind

theorem pdiv_sound {ia ib : Info} {xa xb : X} {ma mb : W} {sa sb : Int}
    (ha : Sound ia xa ma sa) (hb : Sound ib xb mb sb) :
    Sound (pdiv ia ib) (X.div xa xb) (W.div (shiftW ma (-sb).toNat) (shiftW mb sb.toNat)) sa := by
  cases ia with
  | num l1 h1 e1 a1 b1 =>
    cases ib with
    | num l2 h2 e2 a2 b2 =>
      obtain ⟨hs1, hs2, q1, k1, rfl, rfl, hq1, hq2, he1, he2⟩ := ha
      obtain ⟨hs3, hs4, q2, k2, rfl, rfl, hq3, hq4, he3, he4⟩ := hb
      simp only [pdiv]
      split
      · rename_i hz2
        obtain ⟨rfl, rfl⟩ := isZero_eq hq3 hq4 he3 he4 hz2
        split
        · rename_i hnz
          obtain ⟨hq, hk⟩ := nonzero_ne hq1 hq2 he1 he2 hnz
          have hk' : k1 * 2 ^ (-sb).toNat ≠ 0 :=
            Int.mul_ne_zero hk (by have : (0 : Int) < 2 ^ (-sb).toNat := Int.pow_pos (by decide); omega)
          simp [Sound, X.div, W.div, shiftW, hq, hk']
        · split
          · rename_i _ hz1
            obtain ⟨rfl, rfl⟩ := isZero_eq hq1 hq2 he1 he2 hz1
            simp [Sound, X.div, W.div, shiftW]
          · trivial
      · split
        · rename_i _ hnz
          obtain ⟨hq, hk⟩ := nonzero_ne hq3 hq4 he3 he4 hnz
          obtain ⟨hL, hLy, hLb, hs⟩ := low_bound hq3 hq4 he3 he4 hnz
          have hd : k2 * 2 ^ sb.toNat ≠ 0 :=
            Int.mul_ne_zero hk (by have : (0 : Int) < 2 ^ sb.toNat := Int.pow_pos (by decide); omega)
          have hx : X.div (.fin q1) (.fin q2) = .fin (q1 * q2⁻¹) := by
            simp [X.div, hq, Rat.div_def]
          have hm : W.div (shiftW (.fin k1) (-sb).toNat) (shiftW (.fin k2) sb.toNat) =
              .fin (roundDiv (k1 * 2 ^ (-sb).toNat) (k2 * 2 ^ sb.toNat)) := by
            simp [W.div, shiftW, hd]
          rw [hx, hm]
          have hi := inv_between hq3 hq4 hs
          have hr := imul_mem hq1 hq2 hi.1 hi.2
          have hround := div_round k1 k2 sa sb hk
          have hprop := div_prop (a := (k1 : Rat) * pow2 sa) (x := q1) hL hLb hLy he1 he2
            (by grind : -e2 ≤ (k2 : Rat) * pow2 sb - q2) (by grind : (k2 : Rat) * pow2 sb - q2 ≤ e2)
            (by unfold mag rmax; grind : -(mag l1 h1) ≤ q1) (by unfold mag rmax; grind : q1 ≤ mag l1 h1)
          have hpb : pow2 sa ≤ pow2 b1 := pow2_mono hs2
          have eq : q1 * q2⁻¹ = q1 / q2 := (Rat.div_def q1 q2).symm
          refine ⟨hs1, hs2, q1 * q2⁻¹, _, rfl, rfl, hr.1, hr.2, ?_, ?_⟩ <;> rw [eq] <;> grind
        · trivial
    | top => trivial
    | bot => obtain ⟨rfl, rfl⟩ := hb; simp [pdiv, Sound, shiftW]
    | inf =>
      obtain ⟨hs1, hs2, q, k, rfl, rfl, _⟩ := ha; obtain ⟨rfl, rfl⟩ := hb
      refine ⟨hs1, hs2, 0, 0, ?_, ?_, ?_⟩ <;> simp [X.div, W.div, shiftW] <;> grind
  | top => cases ib <;> first | trivial | (obtain ⟨rfl, rfl⟩ := hb; simp [pdiv, Sound, shiftW])
  | bot => obtain ⟨rfl, rfl⟩ := ha; simp [pdiv, Sound, X.div, W.div, shiftW]
  | inf =>
    obtain ⟨rfl, rfl⟩ := ha
    cases ib with
    | top => trivial
    | inf => obtain ⟨rfl, rfl⟩ := hb; simp [pdiv, Sound, X.div, W.div, shiftW]
    | bot => obtain ⟨rfl, rfl⟩ := hb; simp [pdiv, Sound, shiftW]
    | num => obtain ⟨_, _, q, k, rfl, rfl, _⟩ := hb; simp [pdiv, Sound, X.div, W.div, shiftW]


/-- `RESCALE`'s rounding, in values: half a unit of the new scale. -/
theorem rescale_val (k : Int) (n : Nat) (s : Int) :
    -(pow2 (s + n) / 2) ≤ (rescale k n : Rat) * pow2 (s + n) - (k : Rat) * pow2 s ∧
      (rescale k n : Rat) * pow2 (s + n) - (k : Rat) * pow2 s ≤ pow2 (s + n) / 2 := by
  have h := rescale_error k n
  generalize rescale k n = r at h ⊢
  have eU : (((2 ^ n : Nat) : Int) : Rat) = pow2 n := by rw [pow2_nat]; norm_cast
  generalize ((2 ^ n : Nat) : Int) = U at h eU
  have h1 : -(U : Rat) ≤ 2 * ((k : Rat) - U * r) := by exact_mod_cast h.1
  have h2 : 2 * ((k : Rat) - U * r) ≤ U := by exact_mod_cast h.2
  have hp := pow2_pos s
  rw [pow2_add, ← eU]
  have a := Rat.mul_le_mul_of_nonneg_left h1 (Rat.le_of_lt hp)
  have b := Rat.mul_le_mul_of_nonneg_left h2 (Rat.le_of_lt hp)
  constructor <;> grind

theorem arescale_sound {i : Info} {x : X} {m : W} {s : Int} (n : Nat) (h : Sound i x m s) :
    Sound (arescale i n) x (rescaleW m n) (s + n) := by
  cases i with
  | top => trivial
  | inf => obtain ⟨rfl, rfl⟩ := h; exact ⟨rfl, rfl⟩
  | bot => obtain ⟨rfl, rfl⟩ := h; exact ⟨rfl, rfl⟩
  | num lo hi err smin smax =>
    obtain ⟨hs1, hs2, q, k, rfl, rfl, hq1, hq2, he1, he2⟩ := h
    have hr := rescale_val k n s
    have hm := pow2_mono (by omega : s + (n : Int) ≤ smax + n)
    refine ⟨by omega, by omega, q, rescale k n, rfl, rfl, hq1, hq2, ?_, ?_⟩ <;> grind

theorem aquantize_sound (M e : Int) {i : Info} {x : X} {m : W} {s : Int} (h : Sound i x m s) :
    Sound (aquantize M e i) x
      (if s ≤ e then fit M (rescaleW m (e - s).toNat) else fit M (shiftW m (s - e).toNat)) e := by
  cases i with
  | top => trivial
  | inf => obtain ⟨rfl, rfl⟩ := h; split <;> exact ⟨rfl, by simp [fit, rescaleW, shiftW]⟩
  | bot => obtain ⟨rfl, rfl⟩ := h; split <;> exact ⟨rfl, by simp [fit, rescaleW, shiftW]⟩
  | num lo hi err smin smax =>
    obtain ⟨hs1, hs2, q, k, rfl, rfl, hq1, hq2, he1, he2⟩ := h
    have hP := pow2_pos e
    have hx : (0 : Rat) ≤ (if smin < e then pow2 e / 2 else 0) := by split <;> grind
    have hif : (if s ≤ e then fit M (rescaleW (.fin k) (e - s).toNat) else fit M (shiftW (.fin k) (s - e).toNat)) =
        fit M (if s ≤ e then rescaleW (.fin k) (e - s).toNat else shiftW (.fin k) (s - e).toNat) := by
      split <;> rfl
    rw [hif]
    simp only [aquantize]
    apply guard_sound
    by_cases hse : s ≤ e
    · rw [ite_eq_left hse]
      have hn : s + ((e - s).toNat : Int) = e := by omega
      have hr := rescale_val k (e - s).toNat s
      rw [hn] at hr
      by_cases hlt : s < e
      · have hx' : (if smin < e then pow2 e / 2 else 0) = pow2 e / 2 := ite_eq_left (by omega)
        refine ⟨Int.le_refl e, Int.le_refl e, q, _, rfl, rfl, hq1, hq2, ?_, ?_⟩ <;> grind
      · have hse' : e = s := by omega
        subst hse'
        have hz : (e - e).toNat = 0 := by omega
        simp only [hz, rescaleW, rescale_zero]
        refine ⟨Int.le_refl e, Int.le_refl e, q, k, rfl, rfl, hq1, hq2, ?_, ?_⟩ <;> grind
    · rw [ite_eq_right hse]
      have v := shift_val k (s - e).toNat e
      have hn : e + ((s - e).toNat : Int) = s := by omega
      rw [hn] at v
      refine ⟨Int.le_refl e, Int.le_refl e, q, _, rfl, rfl, hq1, hq2, ?_, ?_⟩ <;> grind

theorem alit_sound (x : X) (M e : Int) :
    Sound (alit x M e) (E.evalX (.lit x M e)) (E.evalM Mw (.lit x M e)) (E.scale (.lit x M e)) := by
  cases x with
  | fin q =>
    simp only [alit, E.evalX, E.evalM, E.scale]
    apply guard_sound
    refine ⟨Int.le_refl e, Int.le_refl e, q, roundQ q e, rfl, rfl, Rat.le_refl, Rat.le_refl, ?_, ?_⟩ <;>
      unfold rabs rmax <;> grind
  | inf => exact ⟨rfl, rfl⟩
  | bot => exact ⟨rfl, rfl⟩

/-! ## The theorem -/

/-- **AR-HLR-027: the error budget is sound.** For every expression, what the
checker infers is right: a point it names is the point both meanings reach,
and a range and error it gives contain the exact value and bound the
machine's distance from it. -/
theorem budget_sound (Mw : Int) (e : E) :
    Sound (analyze Mw e) (E.evalX e) (E.evalM Mw e) (E.scale e) := by
  induction e with
  | lit x M k => exact alit_sound x M k
  | add a b iha ihb => exact guard_sound Mw (padd_sound iha ihb)
  | sub a b iha ihb =>
    have h := guard_sound Mw (padd_sound iha (aneg_sound ihb))
    exact h
  | mul a b iha ihb => exact guard_sound Mw (pmul_sound iha ihb)
  | neg a iha => exact aneg_sound iha
  | div a b iha ihb => exact guard_sound Mw (pdiv_sound iha ihb)
  | rescale a n iha => exact arescale_sound n iha
  | narrow a M iha => exact guard_sound M iha
  | quantize a M k iha => exact aquantize_sound M k iha


/-! ## Joining what is known of several values -/

/-- What is known of a value that is one of two: the lanes of a vector, or
the iterations of a loop (AR-LLR-020). -/
def join : Info → Info → Info
  | .inf, .inf => .inf
  | .bot, .bot => .bot
  | .num l1 h1 e1 a1 b1, .num l2 h2 e2 a2 b2 =>
    .num (rmin l1 l2) (rmax h1 h2) (rmax e1 e2) (min a1 a2) (max b1 b2)
  | _, _ => .top

/-- AR-HLR-027: a join is right about whatever either side is right about. -/
theorem join_sound_left {i1 i2 : Info} {x : X} {m : W} {s : Int} (h : Sound i1 x m s) :
    Sound (join i1 i2) x m s := by
  cases i1 <;> cases i2 <;> simp only [join, Sound] at h ⊢ <;> try trivial
  all_goals first
    | exact h
    | (obtain ⟨hs1, hs2, q, k, rfl, rfl, hq1, hq2, he1, he2⟩ := h
       refine ⟨by omega, by omega, q, k, rfl, rfl, ?_, ?_, ?_, ?_⟩ <;> grind [rmin, rmax])

theorem join_sound_right {i1 i2 : Info} {x : X} {m : W} {s : Int} (h : Sound i2 x m s) :
    Sound (join i1 i2) x m s := by
  cases i1 <;> cases i2 <;> simp only [join, Sound] at h ⊢ <;> try trivial
  all_goals first
    | exact h
    | (obtain ⟨hs1, hs2, q, k, rfl, rfl, hq1, hq2, he1, he2⟩ := h
       refine ⟨by omega, by omega, q, k, rfl, rfl, ?_, ?_, ?_, ?_⟩ <;> grind [rmin, rmax])

/-! ## The field contract -/

def isNum : Info → Bool
  | .num .. => true
  | _ => false

/-- `field=closed`: the checker bounds every Word's result as a number
(AR-HLR-028). -/
def closed (Mw : Int) : E → Bool
  | e@(.lit ..) => isNum (analyze Mw e)
  | e@(.add a b) => isNum (analyze Mw e) && closed Mw a && closed Mw b
  | e@(.sub a b) => isNum (analyze Mw e) && closed Mw a && closed Mw b
  | e@(.mul a b) => isNum (analyze Mw e) && closed Mw a && closed Mw b
  | e@(.neg a) => isNum (analyze Mw e) && closed Mw a
  | e@(.div a b) => isNum (analyze Mw e) && closed Mw a && closed Mw b
  | e@(.rescale a _) => isNum (analyze Mw e) && closed Mw a
  | e@(.narrow a _) => isNum (analyze Mw e) && closed Mw a
  | e@(.quantize a _ _) => isNum (analyze Mw e) && closed Mw a

/-- Both meanings of `e` are finite numbers. -/
def Finite (Mw : Int) (e : E) : Prop := ∃ q k, E.evalX e = .fin q ∧ E.evalM Mw e = .fin k

/-- No Word anywhere in `e` answers a wheel point, so nothing overflows, and
no `DIV` divides by zero, in either meaning. -/
def Safe (Mw : Int) : E → Prop
  | e@(.lit ..) => Finite Mw e
  | e@(.add a b) => Finite Mw e ∧ Safe Mw a ∧ Safe Mw b
  | e@(.sub a b) => Finite Mw e ∧ Safe Mw a ∧ Safe Mw b
  | e@(.mul a b) => Finite Mw e ∧ Safe Mw a ∧ Safe Mw b
  | e@(.neg a) => Finite Mw e ∧ Safe Mw a
  | e@(.div a b) => Finite Mw e ∧ Safe Mw a ∧ Safe Mw b ∧
      ∃ q k, E.evalX b = .fin q ∧ E.evalM Mw b = .fin k ∧ q ≠ 0 ∧ k ≠ 0
  | e@(.rescale a _) => Finite Mw e ∧ Safe Mw a
  | e@(.narrow a _) => Finite Mw e ∧ Safe Mw a
  | e@(.quantize a _ _) => Finite Mw e ∧ Safe Mw a

theorem finite_of_isNum {Mw : Int} {e : E} (h : isNum (analyze Mw e) = true) : Finite Mw e := by
  have hs := budget_sound Mw e
  generalize analyze Mw e = i at h hs
  cases i with
  | num => obtain ⟨_, _, q, k, hx, hm, _⟩ := hs; exact ⟨q, k, hx, hm⟩
  | _ => simp [isNum] at h

/-- A divisor whose quotient the checker bounds as a number is nonzero in
both meanings. -/
theorem div_divisor_nonzero {Mw : Int} {a b : E} (h : isNum (analyze Mw (.div a b)) = true)
    (hb' : isNum (analyze Mw b) = true) :
    ∃ q k, E.evalX b = .fin q ∧ E.evalM Mw b = .fin k ∧ q ≠ 0 ∧ k ≠ 0 := by
  have hb := budget_sound Mw b
  simp only [analyze] at h
  generalize analyze Mw a = ia at h
  generalize analyze Mw b = ib at h hb hb'
  cases ib with
  | num l h2 e2 a2 b2 =>
    obtain ⟨_, _, q, k, hx, hm, hq1, hq2, he1, he2⟩ := hb
    cases ia with
    | num =>
      simp only [pdiv] at h
      split at h
      · split at h <;> (try split at h) <;> simp [guard, isNum] at h
      · split at h
        · rename_i _ hnz
          obtain ⟨hq, hk⟩ := nonzero_ne hq1 hq2 he1 he2 hnz
          exact ⟨q, k, hx, hm, hq, hk⟩
        · simp [guard, isNum] at h
    | _ => simp [pdiv, guard, isNum] at h
  | _ => simp [isNum] at hb'

/-- **AR-HLR-028: what `field=closed` guarantees.** Closed code has no wheel
point anywhere — no overflow, no `NARROW` or `QUANTIZE` out of range, no
written point — and no division by zero, in either meaning; by
`budget_sound`, its result is moreover within its budget. -/
theorem closed_safe (Mw : Int) (e : E) (h : closed Mw e = true) : Safe Mw e := by
  induction e with
  | lit => exact finite_of_isNum h
  | add a b iha ihb =>
    simp only [closed, Bool.and_eq_true] at h
    exact ⟨finite_of_isNum h.1.1, iha h.1.2, ihb h.2⟩
  | sub a b iha ihb =>
    simp only [closed, Bool.and_eq_true] at h
    exact ⟨finite_of_isNum h.1.1, iha h.1.2, ihb h.2⟩
  | mul a b iha ihb =>
    simp only [closed, Bool.and_eq_true] at h
    exact ⟨finite_of_isNum h.1.1, iha h.1.2, ihb h.2⟩
  | neg a iha =>
    simp only [closed, Bool.and_eq_true] at h
    exact ⟨finite_of_isNum h.1, iha h.2⟩
  | div a b iha ihb =>
    simp only [closed, Bool.and_eq_true] at h
    have hb : isNum (analyze Mw b) = true := by
      cases b <;> simp only [closed, Bool.and_eq_true] at h <;> grind
    exact ⟨finite_of_isNum h.1.1, iha h.1.2, ihb h.2, div_divisor_nonzero h.1.1 hb⟩
  | rescale a _ iha =>
    simp only [closed, Bool.and_eq_true] at h
    exact ⟨finite_of_isNum h.1, iha h.2⟩
  | narrow a _ iha =>
    simp only [closed, Bool.and_eq_true] at h
    exact ⟨finite_of_isNum h.1, iha h.2⟩
  | quantize a _ _ iha =>
    simp only [closed, Bool.and_eq_true] at h
    exact ⟨finite_of_isNum h.1, iha h.2⟩

end Budget
end Arare
