//@ check-pass
//@ edition:2018
//@ aux-build:edition-imports-2015.rs

#[macro_use]
extern crate edition_imports_2015;

mod import {
    pub struct Path;
}
mod absolute {
    pub struct Path;
}

mod check {
    #[derive(Derive2015)] // OK
    struct S;

    fn check() {
        Path;
    }
}

fn main() {}

// ferrocene-annotations: fls_r6gj1p4gajnq
// Attribute derive
