//@ run-pass

macro_rules! higher_order {
    (subst $lhs:tt => $rhs:tt) => ({
            macro_rules! anon { $lhs => $rhs }
            anon!(1_usize, 2_usize, "foo")
    });
}

macro_rules! outer {
    ($x:expr; $fragment:ident) => {
        macro_rules! inner { ($y:$fragment) => { $x + $y } }
    }
}

fn main() {
    let val = higher_order!(subst ($x:expr, $y:expr, $foo:expr) => (($x + $y, $foo)));
    assert_eq!(val, (3, "foo"));

    outer!(2; expr);
    assert_eq!(inner!(3), 5);
}

// ferrocene-annotations: fls_xa7lp0zg1ol2
// Declarative Macros
//
// ferrocene-annotations: fls_wjldgtio5o75
// Macro Expansion
//
// ferrocene-annotations: fls_vnvt40pa48n8
// Macro Invocation
//
// ferrocene-annotations: fls_4apk1exafxii
// Macro Matching
//
// ferrocene-annotations: fls_ym00b6ewf4n3
// Macro Transcription
//
// ferrocene-annotations: fls_8nzypdu9j3ge
// Metavariables
//
// ferrocene-annotations: fls_n3ktmjqf87qb
// Rule Matching
//
// ferrocene-annotations: fls_qpx6lgapce57
// Token Matching
