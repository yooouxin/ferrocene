//@ run-pass
// Testcase for issue #130, operator associativity.

pub fn main() { assert_eq!(3 * 5 / 2, 7); }

// ferrocene-annotations: fls_kw25194gpael
// Expression Precedence
