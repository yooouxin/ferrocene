//@ check-pass

enum E {
    Foo(String, String, String),
}

struct Bar {
    a: String,
    b: String,
}

fn main() {
    let bar = Bar { a: "1".to_string(), b: "2".to_string() };
    match E::Foo("".into(), "".into(), "".into()) {
        E::Foo(a, b, ref c) => {}
    }
    match bar {
        Bar { a, ref b } => {}
    }
}

// ferrocene-annotations: fls_d44aflefat88
// Path Pattern Matching
//
// ferrocene-annotations: fls_uloyjbaso8pz
// Path Patterns
