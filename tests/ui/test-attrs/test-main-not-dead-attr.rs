//@ run-pass
//@ compile-flags: --test

#![feature(rustc_attrs)]

#![deny(dead_code)]

#[rustc_main]
fn foo() { panic!(); }

// ferrocene-annotations: um_rustc_test
