//@ run-fail
//@ check-stdout
//@ compile-flags: --test
//@ ignore-emscripten

#[test]
fn test_foo() {
    panic!()
}

// ferrocene-annotations: um_rustc_test
