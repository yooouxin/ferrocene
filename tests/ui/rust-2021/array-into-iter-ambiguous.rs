// See https://github.com/rust-lang/rust/issues/88475
//@ run-rustfix
//@ edition:2018
//@ check-pass
#![warn(array_into_iter)]
#![allow(unused)]

struct FooIter;

trait MyIntoIter {
    fn into_iter(self) -> FooIter;
}

impl<T, const N: usize> MyIntoIter for [T; N] {
    fn into_iter(self) -> FooIter {
        FooIter
    }
}

struct Point;

pub fn main() {
    let points: [Point; 1] = [Point];
    let y = points.into_iter();
    //~^ WARNING trait method `into_iter` will become ambiguous in Rust 2021
    //~| WARNING this changes meaning in Rust 2021
}

// ferrocene-annotations: fls_85vx1qfa061i
// Traits
//
// ferrocene-annotations: fls_fk2m2irwpeof
// Implementations
//
// ferrocene-annotations: fls_vhpwge5123cm
// Generic Parameters
//
// ferrocene-annotations: fls_uj0kpjwyld60
// Array Types
//
// ferrocene-annotations: fls_z7q8kbjwdc7g
// Method Call Expressions
//
// ferrocene-annotations: fls_wqazkzle0ix9
// Method Resolution
