//@ run-pass
#![allow(unused_imports)]
#![allow(non_shorthand_field_patterns)]

//@ aux-build:xcrate_struct_aliases.rs

extern crate xcrate_struct_aliases;

use xcrate_struct_aliases::{S, S2};

fn main() {
    let s = S2 {
        x: 1,
        y: 2,
    };
    match s {
        S2 {
            x: x,
            y: y
        } => {
            assert_eq!(x, 1);
            assert_eq!(y, 2);
        }
    }
}

// ferrocene-annotations: fls_kgvleup5mdhq
// Type Aliases
//
// ferrocene-annotations: fls_maw4u1o8q37u
// Crates
//
// ferrocene-annotations: fls_9gprp17h6t1q
// Use Imports
