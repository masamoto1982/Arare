//! Rust source from the intermediate representation (AR-LLR-025, refining
//! AR-HLR-030): the fast path as one loop over the lanes, and the reference
//! interpreter behind it.
//!
//! Every node becomes a `let`, in the integer type its bound needs; lane
//! nodes are computed inside the loop, uniform nodes before it and nodes
//! computed from a reduction after it. A check that can fail — a divisor
//! zero, a code that leaves its width — sets `trap`, and a trapped run
//! answers nothing, so the caller runs the interpreter. The output is
//! deterministic: the same program and signature give the same text
//! (WORK_ORDER.md §5.5). `rustc` then compiles it for the host or for
//! `wasm32` with SIMD128 (ADR-0009).

use crate::ir::{Input, Ir, Kind, Op, Output, N};
use crate::lower::{lower, Unsupported};
use crate::node::Emit;
use arare_kernel::Width;
use std::fmt::Write;

/// Lanes per chunk: a reduction's partial sums are kept in a type that
/// holds this many lanes.
pub const CHUNK: u128 = 4096;

const MAX_W64: u128 = (1u128 << 63) - 2;

pub(crate) fn rty(bound: u128) -> &'static str {
    if bound <= i32::MAX as u128 {
        "i32"
    } else if bound <= i64::MAX as u128 {
        "i64"
    } else {
        "i128"
    }
}

pub(crate) fn width_name(w: Width) -> &'static str {
    match w {
        Width::W8 => "Width::W8",
        Width::W16 => "Width::W16",
        Width::W32 => "Width::W32",
        Width::W64 => "Width::W64",
    }
}

pub(crate) fn storage(w: Width) -> (&'static str, &'static str) {
    match w {
        Width::W8 => ("i8", "I8"),
        Width::W16 => ("i16", "I16"),
        Width::W32 => ("i32", "I32"),
        Width::W64 => ("i64", "I64"),
    }
}

/// The nodes the outputs need. A node's operands come before it, so one
/// sweep from the last node marks them all.
fn needed(ir: &Ir) -> Vec<bool> {
    let mut need = vec![false; ir.nodes.len()];
    for o in &ir.outputs {
        let (Output::Scalar(n) | Output::Vector(n)) = *o;
        need[n] = true;
    }
    for n in (0..ir.nodes.len()).rev() {
        if need[n] {
            for d in ir.nodes[n].op.operands() {
                need[d] = true;
            }
        }
    }
    need
}

/// The body of the fast path for a lowered program.
fn fast(ir: &Ir, name: &str) -> String {
    let mut e = Emit {
        ir,
        out: String::new(),
    };
    let need = needed(ir);
    e.line(
        0,
        &format!("#[inline(never)]\npub fn {name}_fast(args: &[Arg]) -> Option<Vec<Out>> {{"),
    );
    e.line(
        1,
        &format!("if args.len() != {} {{ return None; }}", ir.inputs.len()),
    );
    let mut columns = Vec::new();
    for (i, input) in ir.inputs.iter().enumerate() {
        match *input {
            Input::Scalar { width, scale } => {
                e.line(
                    1,
                    &format!(
                        "let s{i} = args[{i}].plain_num({}, {scale})?;",
                        width_name(width)
                    ),
                );
            }
            Input::Vector { width, scale } => {
                let (_, codes) = storage(width);
                e.line(
                    1,
                    &format!(
                        "let Codes::{codes}(a{i}) = args[{i}].column()?.plain_codes({}, {scale})? else {{ return None; }};",
                        width_name(width)
                    ),
                );
                columns.push(i);
            }
        }
    }
    e.line(1, "let mut trap = 0u8;");
    match columns.split_first() {
        Some((first, rest)) => {
            e.line(1, &format!("let n = a{first}.len();"));
            for c in rest {
                e.line(1, &format!("if a{c}.len() != n {{ return None; }}"));
            }
        }
        None => e.line(1, "let n = 0usize;"),
    }
    // The reductions: enough lanes for the settled width, and a total that
    // cannot leave int64 however the lanes fall.
    for sum in &ir.sums {
        let (s, widths) = (sum.node, &sum.widths);
        let b = e.bound(sum.elem);
        e.line(
            1,
            &format!(
                "if n < {} || !rt::sum_fits(n, {b}, {MAX_W64}) {{ return None; }}",
                widths.len()
            ),
        );
        e.line(1, &format!("let mut t{s}: i64 = 0;"));
    }
    // Uniform nodes, before the loop.
    for (n, node) in ir.nodes.iter().enumerate() {
        if need[n] && !node.lane && !node.late {
            e.node(1, n);
        }
    }
    // Vector outputs.
    for (k, o) in ir.outputs.iter().enumerate() {
        if let Output::Vector(n) = o {
            match ir.nodes[*n].kind {
                // Written a chunk at a time through a buffer in cache, so the
                // output is written once, never zeroed first.
                Kind::Num { width, .. } => {
                    let (st, _) = storage(width);
                    e.line(
                        1,
                        &format!("let mut o{k}: Vec<{st}> = Vec::with_capacity(n);"),
                    );
                    e.line(1, &format!("let mut b{k} = [0 as {st}; {CHUNK}];"));
                }
                Kind::Bool => {
                    e.line(
                        1,
                        &format!("let mut o{k}: Vec<bool> = Vec::with_capacity(n);"),
                    );
                    e.line(1, &format!("let mut b{k} = [false; {CHUNK}];"));
                }
            }
        }
    }
    let lane_nodes: Vec<N> = (0..ir.nodes.len())
        .filter(|&n| need[n] && ir.nodes[n].lane)
        .collect();
    if !lane_nodes.is_empty() {
        e.line(
            1,
            &format!(
                "let mut c = 0usize;\n    while c < n {{\n        let end = (c + {CHUNK}).min(n);"
            ),
        );
        let sums: Vec<(N, N)> = ir.sums.iter().map(|s| (s.node, s.elem)).collect();
        for &(s, x) in &sums {
            e.line(
                2,
                &format!("let mut p{s}: {} = 0;", rty(CHUNK * e.bound(x))),
            );
        }
        // Each chunk as slices of one length, so that the loop needs no
        // bounds checks and vectorizes.
        e.line(2, "let len = end - c;");
        for &i in &columns {
            e.line(2, &format!("let x{i} = &a{i}[c..end];"));
        }
        let single = sums.is_empty()
            && ir
                .outputs
                .iter()
                .filter(|o| matches!(o, Output::Vector(_)))
                .count()
                == 1;
        if !single {
            for (k, o) in ir.outputs.iter().enumerate() {
                if let Output::Vector(_) = o {
                    e.line(2, &format!("let q{k} = &mut b{k}[..len];"));
                }
            }
        }
        let vectors: Vec<(usize, N)> = ir
            .outputs
            .iter()
            .enumerate()
            .filter_map(|(k, o)| match o {
                Output::Vector(n) => Some((k, *n)),
                Output::Scalar(_) => None,
            })
            .collect();
        if let ([(k, v)], true) = (&vectors[..], sums.is_empty()) {
            // One vector and nothing else: the lanes are written straight
            // into the output, which is never zeroed first.
            let st = match ir.nodes[*v].kind {
                Kind::Num { width, .. } => storage(width).0,
                Kind::Bool => "bool",
            };
            e.line(2, &format!("o{k}.extend((0..len).map(|i| {{"));
            for &n in &lane_nodes {
                e.node(3, n);
            }
            e.line(3, &format!("v{v} as {st}"));
            e.line(2, "}));");
        } else {
            e.line(2, "for i in 0..len {");
            for &n in &lane_nodes {
                e.node(3, n);
            }
            for &(s, x) in &sums {
                e.line(3, &format!("p{s} += v{x} as {};", rty(CHUNK * e.bound(x))));
            }
            for &(k, n) in &vectors {
                match ir.nodes[n].kind {
                    Kind::Num { width, .. } => {
                        let (st, _) = storage(width);
                        e.line(3, &format!("q{k}[i] = v{n} as {st};"));
                    }
                    Kind::Bool => e.line(3, &format!("q{k}[i] = v{n};")),
                }
            }
            e.line(2, "}");
            for &(k, _) in &vectors {
                e.line(2, &format!("o{k}.extend_from_slice(&b{k}[..len]);"));
            }
        }
        for &(s, _) in &sums {
            e.line(2, &format!("t{s} += p{s} as i64;"));
        }
        e.line(2, "c = end;");
        e.line(1, "}");
    }
    e.line(1, "if trap != 0 { return None; }");
    // Nodes after the loop: the reductions' results and what is computed from them.
    for (n, node) in ir.nodes.iter().enumerate() {
        if !(need[n] && node.late) {
            continue;
        }
        e.node(1, n);
    }
    e.line(1, "if trap != 0 { return None; }");
    let mut outs = Vec::new();
    for (k, o) in ir.outputs.iter().enumerate() {
        let text = match *o {
            Output::Scalar(n) => match ir.nodes[n].kind {
                Kind::Bool => format!("rt::truth(v{n})"),
                Kind::Num { width, scale } => match ir.nodes[n].op {
                    Op::Sum(_) => {
                        let widths = &ir.sums.iter().find(|s| s.node == n).expect("a SUM").widths;
                        let names: Vec<&str> = widths.iter().map(|w| width_name(*w)).collect();
                        format!(
                            "rt::sum(n, v{n} as i128, &[{}], {scale})?",
                            names.join(", ")
                        )
                    }
                    _ => format!("rt::num(v{n} as i64, {}, {scale})", width_name(width)),
                },
            },
            Output::Vector(n) => match ir.nodes[n].kind {
                Kind::Bool => format!("Out::Bools(o{k})"),
                Kind::Num { width, scale } => {
                    let (_, codes) = storage(width);
                    format!(
                        "Out::Column(Column::computed({}, {scale}, Codes::{codes}(o{k})))",
                        width_name(width)
                    )
                }
            },
        };
        outs.push(text);
    }
    e.line(1, &format!("Some(vec![{}])", outs.join(", ")));
    e.line(0, "}");
    e.out
}

/// Rust source for `source` compiled for `inputs`: a function `name` taking
/// the inputs bottom first and answering the final stack, as the reference
/// interpreter would (AR-HLR-030). When the program has no fast path the
/// function is the interpreter, and a comment says why.
pub fn emit(name: &str, source: &str, inputs: &[Input]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "/// `{source}`, compiled by arare-codegen.");
    let lowered: Result<Ir, Unsupported> = lower(source, inputs);
    let body = match &lowered {
        Ok(ir) => {
            let _ = writeln!(out, "#[allow(unused, clippy::all)]");
            let _ = write!(out, "{}", fast(ir, name));
            format!(
                "if let Some(out) = {name}_fast(args) {{\n        return Ok(out);\n    }}\n    "
            )
        }
        Err(why) => {
            let _ = writeln!(out, "// No fast path: {why}.");
            let _ = writeln!(
                out,
                "pub fn {name}_fast(_: &[Arg]) -> Option<Vec<Out>> {{\n    None\n}}"
            );
            String::new()
        }
    };
    let _ = writeln!(out, "#[allow(unused, clippy::all)]");
    let _ = writeln!(
        out,
        "pub fn {name}(args: &[Arg]) -> Result<Vec<Out>, RunError> {{\n    {body}rt::fallback({source:?}, args)\n}}"
    );
    out
}

/// The header a file of compiled programs starts with.
pub const PRELUDE: &str =
    "use arare_codegen::rt::{self, Arg, Codes, Column, Out, RunError, Width};\n";
