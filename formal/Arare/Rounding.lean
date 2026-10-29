/-!
# Rounding

The one rounding rule Arare uses, round half to even, as an integer quotient.
`RESCALE` (AR-HLR-008) and `DIV` (AR-HLR-007) both round through it, so its
error bound and its tie rule are proved once here.

Verification case AR-VER-103.
-/

namespace Arare

/-- The quotient of `n` by a positive `d`, rounded half to even. `n / d` and
`n % d` are Lean's Euclidean division, which for a positive divisor is the
floor quotient and a remainder in `[0, d)`. -/
def roundDivPos (n d : Int) : Int :=
  if 2 * (n % d) < d then n / d
  else if 2 * (n % d) > d then n / d + 1
  else if (n / d) % 2 = 0 then n / d else n / d + 1

/-- AR-HLR-007, AR-HLR-008: the rounded quotient is within half a unit of the
exact one, `|n/d − q| ≤ 1/2`, stated without division as `2·|n − d·q| ≤ d`. -/
theorem roundDivPos_error (n d : Int) (hd : 0 < d) :
    -d ≤ 2 * (n - d * roundDivPos n d) ∧ 2 * (n - d * roundDivPos n d) ≤ d := by
  have h1 : d * (n / d) + n % d = n := Int.mul_ediv_add_emod n d
  have h2 : 0 ≤ n % d := Int.emod_nonneg n (by omega)
  have h3 : n % d < d := Int.emod_lt_of_pos n hd
  have hs : d * (n / d + 1) = d * (n / d) + d := by rw [Int.mul_add, Int.mul_one]
  unfold roundDivPos
  split <;> (try split) <;> (try split) <;> (try rw [hs]) <;> omega

/-- AR-HLR-008: a tie goes to the even neighbour. -/
theorem roundDivPos_tie_even (n d : Int) (_hd : 0 < d) (htie : 2 * (n % d) = d) :
    roundDivPos n d % 2 = 0 := by
  have h1 : ¬ 2 * (n % d) < d := by omega
  have h2 : ¬ 2 * (n % d) > d := by omega
  unfold roundDivPos
  rw [ite_eq_right h1, ite_eq_right h2]
  split <;> omega

/-- An exact quotient is returned unchanged: rounding only moves a value that
is not already on the grid. -/
theorem roundDivPos_exact (q d : Int) (hd : 0 < d) : roundDivPos (d * q) d = q := by
  have hq : d * q / d = q := Int.mul_ediv_cancel_left q (by omega)
  have hr : d * q % d = 0 := by rw [Int.mul_comm]; exact Int.mul_emod_left q d
  have h0 : 2 * (d * q % d) < d := by rw [hr]; omega
  unfold roundDivPos
  rw [ite_eq_left h0, hq]

/-- The quotient of `n` by a nonzero `d`, rounded half to even; a negative
divisor is reduced to a positive one. The value at `d = 0` is never used:
division by zero is decided by the wheel rules (AR-HLR-007). -/
def roundDiv (n d : Int) : Int :=
  if 0 < d then roundDivPos n d
  else if d < 0 then roundDivPos (-n) (-d)
  else 0

/-- AR-HLR-007, AR-HLR-026: `DIV` of two finite numbers, and a quantized literal, the divisor nonzero, are within
half a unit of the exact quotient: `2·|n − d·q| ≤ |d|`. -/
theorem roundDiv_error (n d : Int) (hd : d ≠ 0) :
    -(if 0 < d then d else -d) ≤ 2 * (n - d * roundDiv n d) ∧
      2 * (n - d * roundDiv n d) ≤ (if 0 < d then d else -d) := by
  unfold roundDiv
  by_cases hp : 0 < d
  · rw [ite_eq_left hp, ite_eq_left hp]
    exact roundDivPos_error n d hp
  · have hn : d < 0 := by omega
    rw [ite_eq_right hp, ite_eq_right hp, ite_eq_left hn]
    have h := roundDivPos_error (-n) (-d) (by omega)
    have e : -n - -d * roundDivPos (-n) (-d) = -(n - d * roundDivPos (-n) (-d)) := by
      rw [Int.neg_mul]; omega
    rw [e] at h
    omega

/-- `RESCALE` by `s` bits: the code `k` at scale `e` becomes the code at scale
`e + s`, rounded half to even (AR-HLR-008). -/
def rescale (k : Int) (s : Nat) : Int := roundDivPos k ((2 ^ s : Nat) : Int)

theorem two_pow_pos_int (s : Nat) : (0 : Int) < ((2 ^ s : Nat) : Int) := by
  have := Nat.two_pow_pos s
  omega

/-- AR-HLR-008, AR-HLR-026: `RESCALE`, and `QUANTIZE` raising an exponent, err by at most half an ulp of the new scale. With
`u = 2^s` the new ulp in units of the old one, `2·|k − u·r| ≤ u`. -/
theorem rescale_error (k : Int) (s : Nat) :
    -((2 ^ s : Nat) : Int) ≤ 2 * (k - ((2 ^ s : Nat) : Int) * rescale k s) ∧
      2 * (k - ((2 ^ s : Nat) : Int) * rescale k s) ≤ ((2 ^ s : Nat) : Int) :=
  roundDivPos_error k _ (two_pow_pos_int s)

/-- AR-HLR-008: `RESCALE` by zero bits changes nothing. -/
theorem rescale_zero (k : Int) : rescale k 0 = k := by
  have := roundDivPos_exact k 1 (by decide)
  simpa [rescale] using this

end Arare
