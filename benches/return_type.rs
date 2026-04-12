use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use sapphire::compiler::compile;
use sapphire::lexer::Lexer;
use sapphire::parser::Parser;
use sapphire::vm::Vm;

/// Compile a Sapphire source string to a ready-to-run VM.
/// Compilation is done once per benchmark group; only `vm.run()` is measured.
fn make_vm(src: &str) -> Vm {
    let tokens = Lexer::new(src).scan_tokens();
    let stmts = Parser::new(tokens).parse().expect("parse error");
    let func = compile(&stmts).expect("compile error");
    Vm::new(func)
}

// ── 1. Annotated vs unannotated (shallow, 1M iterations) ──────────────────────
//
// Isolates the cost of the Option check + string comparison on every Return.

const ITERS: usize = 1_000_000;

fn src_unannotated(n: usize) -> String {
    format!(
        "def add(a, b) {{ a + b }}\n\
         i = 0\n\
         while i < {n} {{ add(i, 1)\ni = i + 1 }}\n\
         i"
    )
}

fn src_annotated(n: usize) -> String {
    format!(
        "def add(a: Int, b: Int) -> Int {{ a + b }}\n\
         i = 0\n\
         while i < {n} {{ add(i, 1)\ni = i + 1 }}\n\
         i"
    )
}

fn bench_shallow(c: &mut Criterion) {
    let mut g = c.benchmark_group("shallow_call");
    g.throughput(Throughput::Elements(ITERS as u64));

    g.bench_function("unannotated", |b| {
        let src = src_unannotated(ITERS);
        b.iter(|| make_vm(&src).run().unwrap());
    });

    g.bench_function("annotated", |b| {
        let src = src_annotated(ITERS);
        b.iter(|| make_vm(&src).run().unwrap());
    });

    g.finish();
}

// ── 2. Call depth (annotated 1-level vs 10-level chain) ───────────────────────
//
// Checks whether the cost compounds linearly with depth.

fn src_deep(depth: usize, n: usize) -> String {
    // Build a chain: f1 calls f2 calls … fN
    let mut src = String::new();
    for i in (1..=depth).rev() {
        if i == depth {
            src.push_str(&format!(
                "def f{i}(x: Int) -> Int {{ x + 1 }}\n"
            ));
        } else {
            src.push_str(&format!(
                "def f{i}(x: Int) -> Int {{ f{}(x) }}\n",
                i + 1
            ));
        }
    }
    src.push_str(&format!(
        "i = 0\nwhile i < {n} {{ f1(i)\ni = i + 1 }}\ni"
    ));
    src
}

fn bench_depth(c: &mut Criterion) {
    let mut g = c.benchmark_group("call_depth");
    let n = 100_000;
    g.throughput(Throughput::Elements(n as u64));

    g.bench_function("depth_1", |b| {
        let src = src_deep(1, n);
        b.iter(|| make_vm(&src).run().unwrap());
    });

    g.bench_function("depth_5", |b| {
        let src = src_deep(5, n);
        b.iter(|| make_vm(&src).run().unwrap());
    });

    g.bench_function("depth_10", |b| {
        let src = src_deep(10, n);
        b.iter(|| make_vm(&src).run().unwrap());
    });

    g.finish();
}

// ── 3. Explicit return vs implicit return ──────────────────────────────────────
//
// Both go through the same Return opcode — should be identical.
// Any difference is noise; confirms the check path is the same.

fn bench_explicit_vs_implicit(c: &mut Criterion) {
    let mut g = c.benchmark_group("return_style");
    let n = 1_000_000;
    g.throughput(Throughput::Elements(n as u64));

    g.bench_function("implicit", |b| {
        let src = format!(
            "def f(x: Int) -> Int {{ x }}\n\
             i = 0\nwhile i < {n} {{ f(i)\ni = i + 1 }}\ni"
        );
        b.iter(|| make_vm(&src).run().unwrap());
    });

    g.bench_function("explicit", |b| {
        let src = format!(
            "def f(x: Int) -> Int {{ return x }}\n\
             i = 0\nwhile i < {n} {{ f(i)\ni = i + 1 }}\ni"
        );
        b.iter(|| make_vm(&src).run().unwrap());
    });

    g.finish();
}

// ── 4. Option::None branch cost ───────────────────────────────────────────────
//
// A loop with no function calls at all — measures the loop/VM overhead
// as a floor, so we can compute the per-call overhead from bench 1.

fn bench_loop_floor(c: &mut Criterion) {
    let mut g = c.benchmark_group("loop_floor");
    let n = 1_000_000;
    g.throughput(Throughput::Elements(n as u64));

    g.bench_function("bare_loop", |b| {
        let src = format!("i = 0\nwhile i < {n} {{ i = i + 1 }}\ni");
        b.iter(|| make_vm(&src).run().unwrap());
    });

    g.finish();
}

criterion_group!(
    benches,
    bench_shallow,
    bench_depth,
    bench_explicit_vs_implicit,
    bench_loop_floor,
);
criterion_main!(benches);
