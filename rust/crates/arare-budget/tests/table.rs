//! AR-VER-215, the part that runs without Lean: the checker renders the whole
//! grid of `formal/BudgetTable.lean`, with the line count and answers worked
//! out by hand at a few points. The Lean job of CI diffs the full table
//! against the Lean checker.

#[test]
fn ar_ver_215_table() {
    let mut out = Vec::new();
    arare_budget::table::write_table(&mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    // 17 literals under 7 unary Words, then every pair.
    assert_eq!(lines.len(), 119 + 119 * 119);
    for expected in [
        "L 0 N 0/1 0/1 0/1 0 0", // the integer 0, exactly
        "L 49 N 1/3 1/3 1/768 -8 -8", // 1/3 at 2^-8: code 85, 85/256 − 1/3 = −1/768
        "L 84 N 1/1000 1/1000 1/1000 0 0", // 1/1000 at scale 0: code 0
        "L 63 N 1/10 1/10 1/10240 -12 -12",   // 0.1 at 2^-12: code 410
        "L 98 T",                             // 200 does not fit int8
        "L 105 I",                            // 1/0
        "L 112 B",                            // 0/0
        // Those two: the errors add (1/768 + 1/1000 = 221/96000); the product's
        // is (1/3 + 1/768)·1/1000 + 1/1000·1/768 = 43/128000; a divisor that
        // may round to zero bounds nothing; the join takes the hull.
        "P 49 84 N 1003/3000 1003/3000 221/96000 -8 -8 | N 997/3000 997/3000 221/96000 -8 -8 | N 1/3000 1/3000 43/128000 -8 -8 | T | N 1/1000 1/3 1/768 -8 0",
    ] {
        assert!(lines.contains(&expected), "missing line: {expected}");
    }
}
