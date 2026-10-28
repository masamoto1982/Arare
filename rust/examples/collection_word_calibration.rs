//! What a collection Word actually costs, and what would have to be counted to
//! price it.
//!
//! Run with:  `cargo run --release --example collection_word_calibration`
//!
//! The work meter prices arithmetic. A collection Word performs no arithmetic,
//! loops internally in Rust, and therefore costs exactly one execution step no
//! matter how much it does — so `UNIQUE` over the materialization ceiling's
//! 100,000 elements spends tens of seconds while every declared ceiling stays
//! silent. Before deciding what to charge, this measures what there is to
//! charge for.
//!
//! Three questions, three sections:
//!
//!  1. **How does cost grow with element count?** Separates the linear Words
//!     (copy/permute) from the comparison Words (`SORT` / `ORDER`, n log n) and
//!     the equality-scan Words (`UNIQUE` / `TALLY` / `GROUP`, n × distinct).
//!  2. **Does element width matter?** A vector of 4096-digit rationals costs
//!     more per element than a vector of small integers — the question is how
//!     much more, and whether ignoring width leaves an unpriced path.
//!  3. **Does the *data* matter, not just its size?** The scan Words are
//!     quadratic only when the elements are distinct. If cost tracks the
//!     distinct count rather than the element count, an up-front charge on n²
//!     would refuse programs that are actually cheap.
//!
//! Like `work_meter_calibration`, this is deliberately not a test: the absolute
//! numbers belong to one machine and one build. What it produces is the cost
//! structure a pricing rule has to match.

use ajisai_core::agent::time_after_setup;

/// Run `setup` to leave the operands on the stack, then time `word` alone
/// (`agent::time_after_setup`).
///
/// The operand has to be built outside the timed region: `0 99999 RANGE
/// UNIQUE` measures `RANGE` and `UNIQUE` together, and `RANGE` is the cheaper
/// of the two by three orders of magnitude at that size — which is exactly the
/// asymmetry being measured.
fn measure(setup: &str, word: &str) -> f64 {
    time_after_setup(setup, word).1
}

/// A vector of `n` distinct small integers.
fn distinct(n: usize) -> String {
    format!("0 {} RANGE", n - 1)
}

/// A vector of `n` small integers drawn from `d` distinct values, so the
/// equality scan finds a match after `d/2` probes on average instead of `n/2`.
fn few_distinct(n: usize, d: usize) -> String {
    format!("0 {} RANGE {{ {d} MOD }} MAP", n - 1)
}

/// A vector of `n` distinct integers each `digits` decimal digits wide.
///
/// Built by multiplying a wide literal, so every element is a genuine BigInt
/// rather than a machine word — and distinct in its *high* limbs, which is the
/// cheap direction for an equality test. The expensive direction is measured by
/// `wide_few_distinct`.
fn wide_distinct(n: usize, digits: usize) -> String {
    format!("1 {n} RANGE {{ {} MUL }} MAP", "9".repeat(digits))
}

/// A vector of `n` wide integers with only `d` distinct values, so most
/// equality tests compare two *equal* wide numbers and cannot exit early.
fn wide_few_distinct(n: usize, d: usize, digits: usize) -> String {
    format!(
        "0 {} RANGE {{ {d} MOD 1 ADD {} MUL }} MAP",
        n - 1,
        "9".repeat(digits)
    )
}

/// A vector of `n` strings, so equality and ordering run over text rather than
/// numbers.
fn text_distinct(n: usize) -> String {
    format!("0 {} RANGE {{ STR }} MAP", n - 1)
}

/// A vector of `n` nested vectors of `k` elements each: equality on an element
/// is itself a loop.
///
/// The elements differ in their *first* position, which is where an elementwise
/// equality test starts — so this is the cheap arrangement. `shared_prefix` is
/// the dear one.
fn nested(n: usize, k: usize) -> String {
    format!("0 {} RANGE {{ 1 {k} RANGE ADD }} MAP", n - 1)
}

/// A vector of `n` nested vectors of `k` elements that agree on the first
/// `k SUB 1` positions and differ only in the last.
///
/// This is the adversarial shape for any price that ignores what is *inside* an
/// element: every equality test walks the whole element before it can answer,
/// so one probe costs `k` leaf comparisons rather than the one an early exit
/// would have needed. Built by masking rather than by writing the literal out,
/// so the source stays small: `i · [0…0 1] + [1…1]`.
fn shared_prefix(n: usize, k: usize) -> String {
    let mut mask = vec!["0"; k];
    mask[k - 1] = "1";
    let ones = vec!["1"; k];
    format!(
        "0 {} RANGE {{ [ {} ] MUL [ {} ] ADD }} MAP",
        n - 1,
        mask.join(" "),
        ones.join(" ")
    )
}

/// A vector of `n` distinct algebraic (Tier 1) values.
///
/// Comparison and equality on these do not reduce to a limb compare: an
/// algebraic value carries a term list, and `ExactReal::cmp_exact` decides an
/// ordering by refining enclosures. If the price is written in limbs alone,
/// this is the shape it cannot see.
fn algebraic(n: usize) -> String {
    format!("2 {} RANGE {{ SQRT }} MAP", n + 1)
}

struct Case {
    /// Word under measurement, written as the source that applies it.
    word: &'static str,
    /// Extra operands the Word needs, pushed after the vector.
    ///
    /// Held separate from the word so the vector is always the same setup and
    /// the operand push is inside the timed region only when it is free.
    extra: &'static str,
}

impl Case {
    const fn unary(word: &'static str) -> Self {
        Self { word, extra: "" }
    }
    const fn with(word: &'static str, extra: &'static str) -> Self {
        Self { word, extra }
    }

    fn run(&self, vector: &str) -> f64 {
        let setup = if self.extra.is_empty() {
            vector.to_string()
        } else {
            format!("{vector} {}", self.extra)
        };
        measure(&setup, self.word)
    }
}

fn print_row(label: &str, cells: &[String]) {
    print!("{label:<26}");
    for cell in cells {
        print!("{cell:>11}");
    }
    println!();
}

fn header(label: &str, columns: &[String]) {
    print_row(label, columns);
    println!("{}", "─".repeat(26 + 11 * columns.len()));
}

fn millis(value: f64) -> String {
    if value >= 100.0 {
        format!("{value:.0}")
    } else if value >= 1.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.2}")
    }
}

/// Cost growth with element count, at a fixed narrow element width.
fn section_element_count() {
    const SIZES: [usize; 5] = [2_000, 4_000, 8_000, 16_000, 32_000];
    println!("── 1. cost vs element count (distinct small integers, ms) ──────\n");
    println!(
        "Every element is one machine-word integer, so width is held constant\n\
         and only the count moves. A row that doubles with n is linear; a row\n\
         that quadruples is quadratic.\n"
    );

    let columns: Vec<String> = SIZES.iter().map(|n| format!("n={n}")).collect();
    header("word", &columns);

    let cases = [
        Case::unary("LENGTH"),
        Case::with("GET", "[ 0 ]"),
        Case::unary("REVERSE"),
        Case::with("TAKE", "[ 100 ]"),
        Case::with("PUT", "0 7"),
        Case::with("CONCAT", "[ 1 2 3 ]"),
        Case::unary("SORT"),
        Case::unary("ORDER"),
        Case::unary("UNIQUE"),
        Case::unary("TALLY"),
    ];
    for case in &cases {
        let cells: Vec<String> = SIZES
            .iter()
            .map(|&n| millis(case.run(&distinct(n))))
            .collect();
        print_row(case.word, &cells);
    }

    // GROUP takes two vectors of equal length; ZIP takes a vector of vectors.
    let group: Vec<String> = SIZES
        .iter()
        .map(|&n| {
            let vector = distinct(n);
            millis(measure(&format!("{vector} {vector}"), "GROUP"))
        })
        .collect();
    print_row("GROUP", &group);

    // `ZIP` takes one vector *of* vectors, so the pair is bundled with
    // `COLLECT` in the setup rather than written as a literal.
    let zip: Vec<String> = SIZES
        .iter()
        .map(|&n| {
            let vector = distinct(n);
            millis(measure(&format!("{vector} {vector} 2 COLLECT"), "ZIP"))
        })
        .collect();
    print_row("ZIP", &zip);
    println!();
}

/// Cost growth with element width, at a fixed element count.
fn section_element_width() {
    const WIDTHS: [usize; 4] = [1, 64, 512, 4_096];
    const N: usize = 4_000;
    println!("── 2. cost vs element width ({N} distinct elements, ms) ───────\n");
    println!(
        "Same element count, wider elements. A row that is flat across the\n\
         widths is priced by count alone; a row that grows needs width in the\n\
         price. `distinct` values differ in their high limbs, which is the\n\
         *cheap* direction for an equality test — §3 measures the other one.\n"
    );

    let columns: Vec<String> = WIDTHS.iter().map(|d| format!("{d}dig")).collect();
    header("word", &columns);

    let cases = [
        Case::unary("REVERSE"),
        Case::with("CONCAT", "[ 1 2 3 ]"),
        Case::unary("SORT"),
        Case::unary("ORDER"),
        Case::unary("UNIQUE"),
    ];
    for case in &cases {
        let cells: Vec<String> = WIDTHS
            .iter()
            .map(|&digits| millis(case.run(&wide_distinct(N, digits))))
            .collect();
        print_row(case.word, &cells);
    }
    println!();
}

/// Cost as a function of the data, not its size.
fn section_distinctness() {
    const N: usize = 16_000;
    const DISTINCT: [usize; 5] = [1, 10, 100, 1_000, N];
    println!("── 3. scan cost vs distinct count (n={N}, ms) ──────────────────\n");
    println!(
        "`UNIQUE` / `TALLY` / `GROUP` scan the distinct values found so far for\n\
         each element, so the work is n × d, not n². The element count alone\n\
         cannot tell these two apart — and d is not known before the scan runs.\n"
    );

    let columns: Vec<String> = DISTINCT.iter().map(|d| format!("d={d}")).collect();
    header("word", &columns);

    for word in ["UNIQUE", "TALLY"] {
        let cells: Vec<String> = DISTINCT
            .iter()
            .map(|&d| millis(measure(&few_distinct(N, d), word)))
            .collect();
        print_row(word, &cells);
    }
    let group: Vec<String> = DISTINCT
        .iter()
        .map(|&d| {
            let keys = few_distinct(N, d);
            millis(measure(&format!("{} {keys}", distinct(N)), "GROUP"))
        })
        .collect();
    print_row("GROUP", &group);

    println!();
    println!(
        "And the same scan over *wide* elements, where a matching probe has to\n\
         compare every limb before it can answer equal (n={}, 4096-digit):\n",
        4_000
    );
    let wide_columns: Vec<String> = [1usize, 10, 100, 1_000]
        .iter()
        .map(|d| format!("d={d}"))
        .collect();
    header("word", &wide_columns);
    for word in ["UNIQUE", "TALLY"] {
        let cells: Vec<String> = [1usize, 10, 100, 1_000]
            .iter()
            .map(|&d| millis(measure(&wide_few_distinct(4_000, d, 4_096), word)))
            .collect();
        print_row(word, &cells);
    }
    println!();
}

/// Elements that are not scalars at all.
fn section_element_shape() {
    const N: usize = 4_000;
    println!("── 4. cost vs element shape (n={N}, ms) ────────────────────────\n");
    println!(
        "An element does not have to be a number. Text compares by codepoint\n\
         and a nested vector compares elementwise, so `one element` is not a\n\
         unit of work unless the elements are scalars.\n"
    );
    let columns = [
        "small".to_string(),
        "text".to_string(),
        "nest8".to_string(),
        "nest64".to_string(),
    ];
    header("word", &columns);

    let shapes = [distinct(N), text_distinct(N), nested(N, 8), nested(N, 64)];
    for word in ["REVERSE", "UNIQUE"] {
        let cells: Vec<String> = shapes
            .iter()
            .map(|shape| millis(measure(shape, word)))
            .collect();
        print_row(word, &cells);
    }
    // SORT cannot order text or nested vectors — it is scalar-only — so it is
    // measured on the shapes it accepts and reported as such.
    print_row(
        "SORT",
        &[
            millis(measure(&distinct(N), "SORT")),
            "n/a".into(),
            "n/a".into(),
            "n/a".into(),
        ],
    );
    println!();
}

/// The two shapes a price written in `element count × element width` cannot
/// see.
fn section_blind_spots() {
    println!("── 5. what element count × width does not capture ──────────────\n");
    println!(
        "Two shapes defeat a price that reads only the top-level element count\n\
         and the widest leaf: an element whose equality test cannot exit early,\n\
         and an element whose comparison is not a limb compare at all.\n"
    );

    let columns: Vec<String> = ["n=1000", "n=2000", "n=4000"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    header("case", &columns);

    for (label, build) in [
        (
            "UNIQUE nested-64 first-differs",
            nested as fn(usize, usize) -> String,
        ),
        ("UNIQUE nested-64 last-differs", shared_prefix),
    ] {
        let cells: Vec<String> = [1000usize, 2000, 4000]
            .iter()
            .map(|&n| millis(measure(&build(n, 64), "UNIQUE")))
            .collect();
        print_row(label, &cells);
    }

    for (label, word) in [
        ("UNIQUE algebraic", "UNIQUE"),
        ("SORT algebraic", "SORT"),
        ("ORDER algebraic", "ORDER"),
    ] {
        let cells: Vec<String> = [1000usize, 2000, 4000]
            .iter()
            .map(|&n| millis(measure(&algebraic(n), word)))
            .collect();
        print_row(label, &cells);
    }
    println!();
}

/// What the ceiling actually permits today.
fn section_ceiling() {
    println!("── 5. what the materialization ceiling permits ─────────────────\n");
    println!(
        "`LOCAL_AGENT_RUNTIME_LIMITS.max_materialized_elements` is 100,000, and\n\
         nothing else bounds a collection Word. Each of these is *one*\n\
         execution step out of a budget of 100,000.\n"
    );
    let columns = ["ms".to_string()];
    header("case", &columns);
    for (label, setup, word) in [
        ("100k UNIQUE", distinct(100_000), "UNIQUE"),
        ("100k SORT", distinct(100_000), "SORT"),
        ("100k REVERSE", distinct(100_000), "REVERSE"),
        ("100k LENGTH", distinct(100_000), "LENGTH"),
    ] {
        print_row(label, &[millis(measure(&setup, word))]);
    }
    println!(
        "\n`LENGTH` was the surprise this harness found and is the one row that\n\
         got fixed rather than priced: it reads a count and is documented O(1),\n\
         but `extract_vector_elements` deep-copied the whole element vector to\n\
         call `.len()` on the copy — 18.1 ms at 100,000 elements, the third\n\
         dearest linear Word in the family. It reads the header now, and what\n\
         is left above is the cost of *dropping* the operand it consumed, which\n\
         every consuming Word pays and no Word can avoid.\n"
    );
}

/// units/ms floor under the current (post-dequadraticization) pricing model,
/// on paths `collectionWork` alone has to bound.
///
/// Mirrors `work_meter_calibration`'s selection of `dense tensor lanes` as
/// `numericWork`'s floor: several representative unbounded paths — a scan
/// word at low distinctness, a copy word, a comparison word — each read back
/// through `collection_work_used()` rather than assumed from the billing
/// design doc's pre-dequadraticization formula, since both the algorithm and
/// the per-element charge changed underneath it.
///
/// **Native, not what `DEFAULT_MAX_COLLECTION_WORK` is sized against** — see
/// `work_meter_calibration`'s equivalent note on its steps/ms section. The
/// playground default is calibrated on WASM, by
/// `scripts/wasm-profile-calibration.mjs`
/// (`docs/dev/host-profile-derivation-2026-08-14.md` §10).
fn section_floor_rate() {
    println!("── 6. collectionWork floor: units/ms under this build ───────────\n");
    println!(
        "Charged units read back from `collection_work_used()`, not derived\n\
         from the pricing formula by hand — the scan family's algorithm and\n\
         charge both changed in the de-quadraticization follow-up, so the old\n\
         hand-derived 30,800 units/ms no longer applies as-is.\n"
    );
    println!(
        "{:<32} {:>12} {:>9} {:>12}",
        "case", "units", "run ms", "units/ms"
    );
    let cases: Vec<(&str, String, &str)> = vec![
        ("UNIQUE 200k d=1", few_distinct(200_000, 1), "UNIQUE"),
        ("TALLY 200k d=1", few_distinct(200_000, 1), "TALLY"),
        ("UNIQUE 100k all-distinct", distinct(100_000), "UNIQUE"),
        ("REVERSE 100k", distinct(100_000), "REVERSE"),
        ("SORT 100k", distinct(100_000), "SORT"),
    ];
    let mut floor = f64::INFINITY;
    for (label, setup, word) in &cases {
        // What the collection meter actually charged for `word`, so a
        // units/ms rate is computed against the pricing model instead of
        // hand-derived from the wall clock and an assumed formula.
        let (interp, millis) = time_after_setup(setup, word);
        let units = interp.collection_work_used();
        let rate = if millis > 0.0 {
            units as f64 / millis
        } else {
            f64::INFINITY
        };
        floor = floor.min(rate);
        println!("{label:<32} {units:>12} {millis:>9.1} {rate:>12.0}");
    }
    println!("\nfloor: {floor:.0} units/ms on this container, this build.");
}

fn main() {
    section_element_count();
    section_element_width();
    section_distinctness();
    section_element_shape();
    section_blind_spots();
    section_ceiling();
    section_floor_rate();
}
