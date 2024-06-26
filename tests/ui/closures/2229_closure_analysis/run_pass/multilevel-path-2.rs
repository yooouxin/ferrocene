//@ edition:2021
//@ run-pass

#![allow(unused)]

// If the closures can handle such precision we should be able to read one path in the closure
// while being able mutate another path outside the closure, where the two paths are disjoint
// after applying two projections on the root variable.


struct Point {
    x: i32,
    y: i32,
}
struct Wrapper {
    p: Point,
}

fn main() {
    let mut w = Wrapper { p: Point { x: 10, y: 10 } };

    let c = || {
        println!("{}", w.p.x);
    };

    // `c` only captures `w.p.x`, therefore it's safe to mutate `w.p.y`.
    let py = &mut w.p.y;
    c();

    *py = 20
}

// ferrocene-annotations: fls_9ucqbbd0s2yo
// Struct Type
//
// ferrocene-annotations: fls_18k3uajrgq5f
// Field Access Expressions
//
// ferrocene-annotations: fls_6ydylimiv553
// Place Expressions
//
// ferrocene-annotations: fls_xcwfotmq2e5d
// Field Resolution
