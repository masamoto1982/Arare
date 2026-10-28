//! AR-VER-201, the part that runs without Lean: the specification renders the
//! whole table, with the grid's line count and answers checked by hand at the
//! points the Lean theorems name. The Lean job of CI diffs the full table
//! against `formal/Table.lean`.

#[test]
fn ar_ver_201_table() {
    let mut out = Vec::new();
    arare_spec::write_table(&mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    // 255² pairs, 255 unary, 253·9 RESCALE, 81·(12+24) quotients, 601 fits,
    // 256 codes, 3 + 9 truth, 2·259 SUM and FILTER, 4 SELECT.
    assert_eq!(
        lines.len(),
        65_025 + 255 + 2_277 + 2_916 + 601 + 256 + 12 + 518 + 4
    );
    for expected in [
        "A F3 F-2 F1 F5 F-6 F-2 F F", // 3 / −2 = −1.5, a tie, to the even −2
        "A I I B B I B T U",          // 1/0 + 1/0 = 0/0, 1/0 · 1/0 = 1/0, 1/0 / 1/0 = 0/0
        "A F0 I I I B F0 F U",        // 0 · 1/0 = 0/0; 0 / 1/0 = 0
        "R 5 1 2",                    // RESCALE 2.5 → 2
        "R 7 1 4",                    // RESCALE 3.5 → 4
        "D 3 -2 -2",
        "T 127 I",           // just past int8's finite range
        "C -127 F-127 -127", // the unused code decodes, but no width produces it
        "K U F F U",         // UNKNOWN AND FALSE = FALSE; OR = UNKNOWN
        "S N,F1,N F1",       // SUM skips NIL
        "S N,N N",           // and is NIL when nothing is left
        "L F-1,N,F1 F-1",    // FILTER keeps only TRUE
        "Q U 2 1",           // a branch on UNKNOWN takes ELSE
    ] {
        assert!(lines.contains(&expected), "missing line: {expected}");
    }
}
