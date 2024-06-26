//@ run-rustfix
struct X {
    x: String,
}

impl Drop for X {
    fn drop(&mut self) {
        println!("value: {}", self.x);
    }
}

fn main() {
    let x = X { x: "hello".to_string() };

    match x {
    //~^ ERROR cannot move out of type `X`, which implements the `Drop` trait
        X { x: y } => println!("contents: {}", y)
    }
}

// ferrocene-annotations: fls_u2mzjgiwbkz0
// Destructors
//
// ferrocene-annotations: fls_asj8rgccvkoe
// Struct Pattern Matching
//
// ferrocene-annotations: fls_7dbd5t2750ce
// Struct Patterns
