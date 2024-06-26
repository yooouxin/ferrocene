// Checks that `const fn` with const params can be used.
//@ run-pass

const fn const_u32_identity<const X: u32>() -> u32 {
    X
}

fn main() {
    assert_eq!(const_u32_identity::<18>(), 18);
}

// ferrocene-annotations: fls_utuu8mdbuyxm
// Generic Arguments
// ferrocene-annotations: fls_qcb1n9c0e5hz
// Functions
