#![allow(unused)]

use std::ops::{Deref, DerefMut};
use std::mem::ManuallyDrop;

#[derive(Default)]
struct MockBox<T> {
    value: [T; 1],
}

impl<T> MockBox<T> {
    fn new(value: T) -> Self { MockBox { value: [value] } }
}

impl<T> Deref for MockBox<T> {
    type Target = T;
    fn deref(&self) -> &T { &self.value[0] }
}

impl<T> DerefMut for MockBox<T> {
    fn deref_mut(&mut self) -> &mut T { &mut self.value[0] }
}

#[derive(Default)]
struct MockVec<T> {
    value: [T; 0],
}

impl<T> MockVec<T> {
    fn new() -> Self { MockVec { value: [] } }
}

impl<T> Deref for MockVec<T> {
    type Target = [T];
    fn deref(&self) -> &[T] { &self.value }
}

impl<T> DerefMut for MockVec<T> {
    fn deref_mut(&mut self) -> &mut [T] { &mut self.value }
}


union U {
    x: ManuallyDrop<((MockVec<u8>, MockVec<u8>), MockVec<u8>)>,
    y: ManuallyDrop<MockBox<MockVec<u8>>>,
}

fn use_borrow<T>(_: &T) {}

unsafe fn parent_sibling_borrow() {
    let mut u = U { x: ManuallyDrop::new(((MockVec::new(), MockVec::new()), MockVec::new())) };
    let a = &mut (*u.x).0;
    let b = &u.y; //~ ERROR cannot borrow `u` (via `u.y`)
    use_borrow(a);
}

unsafe fn parent_sibling_move() {
    let u = U { x: ManuallyDrop::new(((MockVec::new(), MockVec::new()), MockVec::new())) };
    let a = u.x.0; //~ERROR cannot move out of dereference
    let a = u.x;
    let b = u.y; //~ ERROR use of moved value: `u`
}

unsafe fn grandparent_sibling_borrow() {
    let mut u = U { x: ManuallyDrop::new(((MockVec::new(), MockVec::new()), MockVec::new())) };
    let a = &mut ((*u.x).0).0;
    let b = &u.y; //~ ERROR cannot borrow `u` (via `u.y`)
    use_borrow(a);
}

unsafe fn grandparent_sibling_move() {
    let u = U { x: ManuallyDrop::new(((MockVec::new(), MockVec::new()), MockVec::new())) };
    let a = (u.x.0).0; //~ERROR cannot move out of dereference
    let a = u.x;
    let b = u.y; //~ ERROR use of moved value: `u`
}

unsafe fn deref_sibling_borrow() {
    let mut u = U { y: ManuallyDrop::new(MockBox::default()) };
    let a = &mut *u.y;
    let b = &u.x; //~ ERROR cannot borrow `u` (via `u.x`)
    use_borrow(a);
}

unsafe fn deref_sibling_move() {
    let u = U { x: ManuallyDrop::new(((MockVec::new(), MockVec::new()), MockVec::new())) };
    // No way to test deref-move without Box in union
    // let a = *u.y;
    // let b = u.x; ERROR use of moved value: `u`
}


fn main() {}

// ferrocene-annotations: fls_9ucqbbd0s2yo
// Struct Types
//
// ferrocene-annotations: fls_uj0kpjwyld60
// Array Types
//
// ferrocene-annotations: fls_fk2m2irwpeof
// Implementations
//
// ferrocene-annotations: fls_vhpwge5123cm
// Generic Parameters
//
// ferrocene-annotations: fls_jep7p27kaqlp
// Unsafety
//
// ferrocene-annotations: fls_18k3uajrgq5f
// Field Access Expressions
//
// ferrocene-annotations: fls_qztk0bkju9u
// Borrow Expression
//
// ferrocene-annotations: fls_a14slch83hzn
// Borrowing
//
// ferrocene-annotations: fls_5cm4gkt55hjh
// Dereference Expression
//
// ferrocene-annotations: fls_g0uyl7qw4c7w
// Parenthesized Expressions
//
// ferrocene-annotations: fls_xcwfotmq2e5d
// Field Resolution
