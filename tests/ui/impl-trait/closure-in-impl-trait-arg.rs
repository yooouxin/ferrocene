//@ run-pass
#![allow(unused_must_use)]
fn bug(_: impl Iterator<Item = [(); { |x: u32| { x }; 4 }]>) {}

fn main() {
    bug(std::iter::empty());
}

// ferrocene-annotations: fls_tjyexqrx0fx5
// Closure Expressions
