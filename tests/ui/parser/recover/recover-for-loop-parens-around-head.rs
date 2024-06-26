//@ run-rustfix
// Here we test that the parser is able to recover in a situation like
// `for ( $pat in $expr )` since that is familiar syntax in other languages.
// Instead we suggest that the user writes `for $pat in $expr`.

#![deny(unused)] // Make sure we don't trigger `unused_parens`.

fn main() {
    let vec = vec![1, 2, 3];

    for ( _elem in vec ) {
        //~^ ERROR unexpected parentheses surrounding `for` loop head
        const _RECOVERY_WITNESS: u32 = 0u8; //~ ERROR mismatched types
    }
}

// ferrocene-annotations: fls_onfyolkcbeh3
// For Loops
