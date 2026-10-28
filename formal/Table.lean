import Arare

/-!
# The model as a table

Prints the Lean model's answers over a fixed grid of inputs: every pair of
int8 values under each arithmetic and comparison Word, RESCALE, the two
rounding quotients, `fit`, the codes, the truth connectives, SUM, FILTER and
SELECT. The Rust
executable specification prints the same grid
(`rust/crates/arare-spec/src/table.rs`), and the Lean job of CI diffs
the two outputs line by line: the model-to-code layer of
docs/quality/FORMAL_VERIFICATION.md (AR-LLR-001, verification case AR-VER-201).
-/

open Arare

def showW : W → String
  | .fin k => s!"F{k}"
  | .inf => "I"
  | .bot => "B"

def showT : Truth → String
  | .false => "F"
  | .unknown => "U"
  | .true => "T"

def showO : Option W → String
  | none => "N"
  | some w => showW w

/-- The integers `lo, lo+1, …, lo+n-1`. -/
def ints (lo : Int) (n : Nat) : List Int := (List.range n).map (fun i => lo + Int.ofNat i)

/-- Every int8 value: the 256 codes less the unused one, decoded. -/
def v8 : List W := ((ints (-128) 256).filter (· ≠ -127)).map (decode 128)

def sumInputs : List (List (Option W)) :=
  let atoms : List (Option W) := [none, some (.fin 0), some (.fin 1), some (.fin (-1)), some .inf, some .bot]
  [[]] ++ atoms.map (fun a => [a]) ++
    atoms.flatMap (fun a => atoms.map (fun b => [a, b])) ++
    atoms.flatMap (fun a => atoms.flatMap (fun b => atoms.map (fun c => [a, b, c])))

def main : IO Unit := do
  let out ← IO.getStdout
  for x in v8 do
    for y in v8 do
      out.putStrLn s!"A {showW x} {showW y} {showW (W.add x y)} {showW (W.sub x y)} {showW (W.mul x y)} {showW (W.div x y)} {showT (eqT x y)} {showT (ltT x y)}"
  for x in v8 do
    out.putStrLn s!"G {showW x} {showW (W.neg x)} {showW (fit 128 x)}"
  for k in ints (-126) 253 do
    for s in List.range 9 do
      out.putStrLn s!"R {k} {s} {rescale k s}"
  for n in ints (-40) 81 do
    for d in ints 1 12 do
      out.putStrLn s!"P {n} {d} {roundDivPos n d}"
    for d in (ints (-12) 25).filter (· ≠ 0) do
      out.putStrLn s!"D {n} {d} {roundDiv n d}"
  for k in ints (-300) 601 do
    out.putStrLn s!"T {k} {showW (fit 128 (.fin k))}"
  for c in ints (-128) 256 do
    out.putStrLn s!"C {c} {showW (decode 128 c)} {encode 128 (decode 128 c)}"
  for a in [Truth.false, .unknown, .true] do
    out.putStrLn s!"N {showT a} {showT (Truth.not a)}"
    for b in [Truth.false, .unknown, .true] do
      out.putStrLn s!"K {showT a} {showT b} {showT (Truth.and a b)} {showT (Truth.or a b)}"
  for xs in sumInputs do
    out.putStrLn s!"S {String.intercalate "," (xs.map showO)} {showO (sumT xs)}"
    let kept := filterT (fun (x : Option W) => x.map (fun w => ltT w (.fin 0))) xs
    out.putStrLn s!"L {String.intercalate "," (xs.map showO)} {String.intercalate "," (kept.map showO)}"
  for c in [none, some Truth.false, some .unknown, some .true] do
    let shown := match c with | none => "N" | some t => showT t
    out.putStrLn s!"Q {shown} {selectT c (1 : Nat) 2} {(truthOf c).rank}"
