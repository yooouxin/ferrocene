//@ run-pass
#![allow(dead_code)]
//@ pretty-expanded FIXME #23616

enum Foo {
    Bar { x: isize },
    Baz { y: isize }
}

pub fn main() { }

// ferrocene-annotations: fls_szibmtfv117b
// Enum Types
//
// ferrocene-annotations: fls_9ucqbbd0s2yo
// Struct Types
