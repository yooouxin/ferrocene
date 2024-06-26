//@ run-pass
#![allow(dead_code)]
//@ pretty-expanded FIXME #23616

enum Foo {
    A,
    B(isize),
    C { a: isize },
}

fn _foo (f: Foo) {
    match f {
        Foo::A | Foo::B(_) | Foo::C { .. } => {}
    }
}

pub fn main() {}

// ferrocene-annotations: fls_szibmtfv117b
// Enum Types
//
// ferrocene-annotations: fls_dq403wq5yrs
// Namespaces
