// Test that `-C instrument-coverage` injects Coverage statements.
// The Coverage::CounterIncrement statements are later converted into LLVM
// instrprof.increment intrinsics, during codegen.

<<<<<<< HEAD
//@ ignore-aarch64-unknown-ferrocenecoretest - instrument-coverage is not supported
//@ unit-test: InstrumentCoverage
=======
//@ test-mir-pass: InstrumentCoverage
>>>>>>> pull-upstream-temp--do-not-use-for-real-code
//@ compile-flags: -Cinstrument-coverage -Zno-profiler-runtime

// EMIT_MIR instrument_coverage.main.InstrumentCoverage.diff
// EMIT_MIR instrument_coverage.bar.InstrumentCoverage.diff
fn main() {
    loop {
        if bar() {
            break;
        }
    }
}

#[inline(never)]
fn bar() -> bool {
    true
}

// CHECK:     coverage ExpressionId({{[0-9]+}}) =>
// CHECK-DAG: coverage Code(Counter({{[0-9]+}})) =>
// CHECK-DAG: coverage Code(Expression({{[0-9]+}})) =>
// CHECK:     bb0:
// CHECK-DAG: Coverage::ExpressionUsed({{[0-9]+}})
// CHECK-DAG: Coverage::CounterIncrement({{[0-9]+}})
