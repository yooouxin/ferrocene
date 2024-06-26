//@ edition:2021
//@ run-pass

// Test that edition 2021 enables disjoint capture by default.

struct Point {
    x: i32,
    y: i32,
}

fn main() {
    let mut p = Point { x: 10, y: 10 };

    let c = || {
        println!("{}", p.x);
    };

    // `c` should only capture `p.x`, therefore mutating `p.y` is allowed.
    let py = &mut p.y;

    c();
    *py = 20;
}

// ferrocene-annotations: fls_9ucqbbd0s2yo
// Struct Type
