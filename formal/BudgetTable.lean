import Arare

/-!
# The checker as a table

Prints what the Lean checker `Arare.Budget.analyze` infers over a fixed grid
of expressions: a set of literals (integers, quantized rationals that round
and that do not, a literal out of its width, the two wheel points), each under
seven unary Words, and every pair of those under ADD, SUB, MUL, DIV and the
join. The Rust checker prints the same grid
(`rust/crates/arare-budget/examples/budget_table.rs`) and the Lean job of CI
diffs the two outputs: the model-to-code layer for the error budget
(AR-LLR-019, verification case AR-VER-215).
-/

open Arare Arare.Budget

def showQ (q : Rat) : String := s!"{q.num}/{q.den}"

def showI : Info → String
  | .top => "T"
  | .inf => "I"
  | .bot => "B"
  | .num lo hi err a b => s!"N {showQ lo} {showQ hi} {showQ err} {a} {b}"

def M8 : Int := 2 ^ 7
def M16 : Int := 2 ^ 15
def M32 : Int := 2 ^ 31
def Mw : Int := 2 ^ 63

def lits : List E := [
  .lit (.fin 0) M8 0, .lit (.fin 1) M8 0, .lit (.fin (-1)) M8 0, .lit (.fin 3) M8 0,
  .lit (.fin (-7)) M8 0, .lit (.fin 126) M8 0, .lit (.fin 40000) M32 0,
  .lit (.fin (1 / 3)) M16 (-8), .lit (.fin (-5 / 7)) M16 (-4), .lit (.fin (1 / 10)) M16 (-12),
  .lit (.fin (5 / 2)) M8 (-1), .lit (.fin (7 / 4)) M8 0, .lit (.fin (1 / 1000)) M8 0,
  .lit (.fin 3) M8 2, .lit (.fin 200) M8 0, .lit .inf M8 0, .lit .bot M8 0]

def unary (x : E) : List E :=
  [x, .neg x, .rescale x 1, .rescale x 3, .narrow x M8, .quantize x M16 (-4), .quantize x M8 0]

def level1 : List E := lits.flatMap unary

def main : IO Unit := do
  let out ← IO.getStdout
  let xs := level1.zipIdx
  for (x, i) in xs do
    out.putStrLn s!"L {i} {showI (analyze Mw x)}"
  for (a, i) in xs do
    for (b, j) in xs do
      let r := [analyze Mw (.add a b), analyze Mw (.sub a b), analyze Mw (.mul a b),
        analyze Mw (.div a b), join (analyze Mw a) (analyze Mw b)]
      out.putStrLn s!"P {i} {j} {String.intercalate " | " (r.map showI)}"
