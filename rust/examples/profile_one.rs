//! Run one program on an unbounded interpreter, for a profiler.
//! `cargo run --release --example profile_one -- '<setup>' '<source>' [repeat]`
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let reps: usize = a.get(2).map(|s| s.parse().unwrap()).unwrap_or(1);
    let (interp, ms) = ajisai_core::agent::time_after_setup(&a[0], &a[1].repeat(reps));
    eprintln!("{ms:.2} ms, stack {}", interp.get_stack().len());
}
