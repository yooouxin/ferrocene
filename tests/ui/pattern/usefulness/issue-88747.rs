//@ check-pass: this used to be a stack overflow because of recursion in `usefulness.rs`

macro_rules! long_tuple_arg {
    ([$($t:tt)*]#$($h:tt)*) => {
        long_tuple_arg!{[$($t)*$($t)*]$($h)*}
    };
    ([$([$t:tt $y:tt])*]) => {
        pub fn _f(($($t,)*): ($($y,)*)) {}
    }
}

long_tuple_arg!{[[_ u8]]########## ###}

fn main() {}

// ferrocene-annotations: fls_xa7lp0zg1ol2
// Declarative Macros
//
// ferrocene-annotations: fls_8nzypdu9j3ge
// Metavariables
//
// ferrocene-annotations: fls_k01lsksqtq1r
// Repetition
//
// ferrocene-annotations: fls_vnvt40pa48n8
// Macro Invocation
//
// ferrocene-annotations: fls_wjldgtio5o75
// Macro Expansion
//
// ferrocene-annotations: fls_4apk1exafxii
// Macro Matching
//
// ferrocene-annotations: fls_n3ktmjqf87qb
// Rule Matching
//
// ferrocene-annotations: fls_qpx6lgapce57
// Token Matching
//
// ferrocene-annotations: fls_ym00b6ewf4n3
// Macro Transcription
//
// ferrocene-annotations: fls_xlfo7di0gsqz
// Hygiene
