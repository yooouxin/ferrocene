//@ run-pass
#![allow(unused_variables)]
// Ensure that we deduce expected argument types when a `fn()` type is expected (#41755)

fn foo(f: fn(Vec<u32>) -> usize) { }

fn main() {
    foo(|x| x.len())
}

// ferrocene-annotations: fls_dw33yt5g6m0k
// Type Coercion
