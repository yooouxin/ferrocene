// Regression test for issue #91227.

//@ run-rustfix

#![allow(unused_macros)]

marco_rules! thing {
//~^ ERROR: expected one of
//~| HELP: perhaps you meant to define a macro
    () => {}
}

fn main() {}

// ferrocene-annotations: fls_xa7lp0zg1ol2
// Declarative Macros
//
// ferrocene-annotations: fls_9kjpxri0axvg
// Weak Keywords
