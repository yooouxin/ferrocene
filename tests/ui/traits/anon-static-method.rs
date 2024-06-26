//@ run-pass
struct Foo {
    x: isize
}

impl Foo {
    pub fn new() -> Foo {
        Foo { x: 3 }
    }
}

pub fn main() {
    let x = Foo::new();
    println!("{}", x.x);
}

// ferrocene-annotations: fls_fk2m2irwpeof
// Implementations
