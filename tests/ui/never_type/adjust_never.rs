// Test that a variable of type ! can coerce to another type.

//@ check-pass

#![feature(never_type)]

fn main() {
    let x: ! = panic!();
    let y: u32 = x;
}

// ferrocene-annotations: fls_dw33yt5g6m0k
// Type Coercion
