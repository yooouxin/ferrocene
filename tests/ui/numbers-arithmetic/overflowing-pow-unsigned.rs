//@ run-fail
//@ error-pattern:thread 'main' panicked
//@ error-pattern:attempt to multiply with overflow
//@ ignore-emscripten no processes
//@ compile-flags: -C debug-assertions

fn main() {
    let _x = 2u32.pow(1024);
}

// ferrocene-annotations: um_rustc_C_debug_assertions
