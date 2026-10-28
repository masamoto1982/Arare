#!/usr/bin/env python3
"""Exact-arithmetic comparison suite: the Python side.

    python3 scripts/bench/exact_bench.py [FILTER] [--json]

Computes every case of scripts/bench/exact-bench-cases.json (by its `kind`) with
fractions.Fraction (the standard library), SymPy and python-flint, each in its
own idiom but folding left to right as Ajisai's FOLD does, and prints the time
and the answer each one gave. The answer is rendered the way Ajisai displays
it (`p/q`, `[ a b ]`, `1/1+sqrt(2)`), so scripts/bench/exact-bench.mjs can
compare the two sides by text. A system that cannot express a case is left
out of that case rather than given an approximation: python-flint 0.9 and
Fraction have no exact square roots.

Requires: pip install sympy python-flint (gmpy2 optional; SymPy uses
python-flint for its ground types when it is installed, and the report names
the ground types in use).
"""

import json
import sys
import time
from fractions import Fraction
from functools import reduce
from pathlib import Path

CASES = Path(__file__).with_name("exact-bench-cases.json")

# Newton's iterate has tens of thousands of digits; rendering it is outside
# the timed region, but Python refuses the conversion past 4300 digits.
sys.set_int_max_str_digits(0)


def rational_text(p, q):
    return f"{p}/{q}"


def vector_text(items):
    return "[ " + " ".join(items) + " ]"


# --- fractions.Fraction --------------------------------------------------


def fraction_text(x):
    return rational_text(x.numerator, x.denominator)


FRACTION = {
    "harmonic": lambda n: reduce(lambda a, k: a + Fraction(1, k), range(1, n + 1), Fraction(0)),
    "basel": lambda n: reduce(lambda a, k: a + Fraction(1, k * k), range(1, n + 1), Fraction(0)),
    "newton-sqrt2": lambda n: reduce(lambda x, _: (x + 2 / x) / 2, range(n), Fraction(1)),
    "elementwise": lambda n: [Fraction(k + 1, k) * Fraction(k, k + 2) for k in range(1, n + 1)],
}


def fraction_render(value):
    if isinstance(value, list):
        return vector_text(fraction_text(x) for x in value)
    return fraction_text(value)


# --- python-flint ----------------------------------------------------------


def flint_cases():
    from flint import fmpq

    return {
        "harmonic": lambda n: reduce(lambda a, k: a + fmpq(1, k), range(1, n + 1), fmpq(0)),
        "basel": lambda n: reduce(lambda a, k: a + fmpq(1, k * k), range(1, n + 1), fmpq(0)),
        "newton-sqrt2": lambda n: reduce(lambda x, _: (x + 2 / x) / 2, range(n), fmpq(1)),
        "elementwise": lambda n: [fmpq(k + 1, k) * fmpq(k, k + 2) for k in range(1, n + 1)],
    }


def flint_render(value):
    def one(x):
        return rational_text(int(x.p), int(x.q))

    if isinstance(value, list):
        return vector_text(one(x) for x in value)
    return one(value)


# --- SymPy -------------------------------------------------------------------


def sympy_cases():
    from sympy import Integer, Rational, sqrt

    return {
        "harmonic": lambda n: reduce(lambda a, k: a + Rational(1, k), range(1, n + 1), Integer(0)),
        "basel": lambda n: reduce(lambda a, k: a + Rational(1, k * k), range(1, n + 1), Integer(0)),
        "newton-sqrt2": lambda n: reduce(lambda x, _: (x + 2 / x) / 2, range(n), Integer(1)),
        "elementwise": lambda n: [Rational(k + 1, k) * Rational(k, k + 2) for k in range(1, n + 1)],
        "sqrt-sum": lambda n: reduce(lambda a, k: a + sqrt(k), range(1, n + 1), Integer(0)),
    }


def sympy_render(value):
    from sympy import Rational

    def coefficient(c):
        c = Rational(c)
        return rational_text(abs(c.p), c.q)

    def one(x):
        if x.is_Rational:
            return rational_text(x.p, x.q)
        terms = []
        for term, c in x.as_coefficients_dict().items():
            radicand = 1 if term == 1 else int(term.base)
            terms.append((radicand, Rational(c)))
        out = ""
        for index, (radicand, c) in enumerate(sorted(terms)):
            sign = "-" if c < 0 else ("+" if index else "")
            if radicand == 1:
                body = coefficient(c)
            elif abs(c) == 1:
                body = f"sqrt({radicand})"
            else:
                body = f"{coefficient(c)}*sqrt({radicand})"
            out += sign + body
        return out

    if isinstance(value, list):
        return vector_text(one(x) for x in value)
    return one(value)


# --- driver ------------------------------------------------------------------


def no_reset():
    pass


def systems():
    found = [("Fraction", FRACTION, fraction_render, no_reset, "Python " + sys.version.split()[0])]
    try:
        import flint

        label = "python-flint " + flint.__version__
        found.append(("python-flint", flint_cases(), flint_render, no_reset, label))
    except ImportError:
        print("python-flint not installed; skipping it", file=sys.stderr)
    try:
        import sympy
        from sympy.core.cache import clear_cache
        from sympy.external.gmpy import GROUND_TYPES

        # SymPy caches the results of its constructors, so a second run of
        # `sqrt(k)` or of a sum would answer from the cache; every timed run
        # starts from an empty one.
        label = f"SymPy {sympy.__version__} (ground types: {GROUND_TYPES})"
        found.append(("SymPy", sympy_cases(), sympy_render, clear_cache, label))
    except ImportError:
        print("sympy not installed; skipping it", file=sys.stderr)
    return found


def best_time(fn, n, repeats, reset):
    best, value = float("inf"), None
    for _ in range(repeats):
        reset()
        started = time.perf_counter()
        value = fn(n)
        best = min(best, (time.perf_counter() - started) * 1000.0)
    return best, value


def main():
    args = sys.argv[1:]
    as_json = "--json" in args
    name_filter = next((a for a in args if not a.startswith("--")), None)
    suite = json.loads(CASES.read_text())
    repeats = suite.get("repeats", 5)

    rows, versions = [], {}
    for system, cases, render, reset, version in systems():
        versions[system] = version
        for case in suite["cases"]:
            name, kind, n = case["name"], case["kind"], case["n"]
            if name_filter and name_filter not in name:
                continue
            if kind not in cases:
                continue
            ms, value = best_time(cases[kind], n, repeats, reset)
            rows.append({"system": system, "name": name, "n": n, "ms": ms, "answer": render(value)})
            if not as_json:
                print(f"{system:<13} {name:<14} n={n:<7} {ms:>10.2f} ms")
    if as_json:
        print(json.dumps({"versions": versions, "rows": rows}))


if __name__ == "__main__":
    main()
