/-!
# Arare's semantic model

Phase 0 of WORK_ORDER.md §7 sets up the project and the CI job that builds it;
it states no semantics. Phase 1 writes the value model of WORK_ORDER.md §3
here as the high-level requirements in `spec/` fix it, and proves:

* the wheel axioms over the value codes,
* that strong Kleene AND and NOT form the truth lattice,
* that inside `field=closed` code the operations agree with ℤ,
* that RESCALE errs by at most half an ulp,

and, in Phase 3, that the error-budget inference is sound.

The one theorem below exists so the CI job has something to check: a build
that silently compiled nothing would otherwise pass.
-/

namespace Arare

/-- Placeholder: the toolchain and the CI job are live. -/
theorem toolchain_live : (1 : Nat) + 1 = 2 := rfl

end Arare
