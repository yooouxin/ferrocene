//@ run-pass
#![allow(unused_mut)]
// Check that a trait is still object-safe (and usable) if it has
// methods with by-value self so long as they require `Self : Sized`.


trait Counter {
    fn tick(&mut self) -> u32;
    fn get(self) -> u32 where Self : Sized;
}

struct CCounter {
    c: u32
}

impl Counter for CCounter {
    fn tick(&mut self) -> u32 { self.c += 1; self.c }
    fn get(self) -> u32 where Self : Sized { self.c }
}

fn tick1<C:Counter>(mut c: C) -> u32 {
    tick2(&mut c);
    c.get()
}

fn tick2(c: &mut dyn Counter) {
    tick3(c);
}

fn tick3<C:?Sized+Counter>(c: &mut C) {
    c.tick();
    c.tick();
}

fn main() {
    let mut c = CCounter { c: 0 };
    let value = tick1(c);
    assert_eq!(value, 2);
}

// ferrocene-annotations: fls_7nv8ualeaqe3
// Where Clauses
//
// ferrocene-annotations: fls_142vncdktbin
// Reference Types
//
// ferrocene-annotations: fls_qztk0bkju9u
// Borrow Expression
//
// ferrocene-annotations: fls_a14slch83hzn
// Borrowing
//
// ferrocene-annotations: fls_77scxuomlbgs
// Passing Conventions
//
// ferrocene-annotations: fls_jeoas4n6su4
// Trait and Lifetime Bounds
