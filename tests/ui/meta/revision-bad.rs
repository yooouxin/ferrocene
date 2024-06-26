// Meta test for compiletest: check that when we give the wrong error
// patterns, the test fails.

//@ run-fail
//@ revisions: foo bar
//@ should-fail
//@ needs-run-enabled
//@[foo] error-pattern:bar
//@[bar] error-pattern:foo

#[cfg(foo)]
fn die() {
    panic!("foo");
}
#[cfg(bar)]
fn die() {
    panic!("bar");
}

fn main() {
    die();
}

// ferrocene-annotations: fls_fymvsy6ig99a
// Attribute cfg
