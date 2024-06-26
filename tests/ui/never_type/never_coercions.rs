//@ run-pass
// Test that having something of type ! doesn't screw up type-checking and that it coerces to the
// LUB type of the other match arms.

fn main() {
    let v: Vec<u32> = Vec::new();
    match 0u32 {
        0 => &v,
        1 => return,
        _ => &v[..],
    };
}

// ferrocene-annotations: fls_dw33yt5g6m0k
// Type Coercion
// ferrocene-annotations: fls_exe4zodlwfez
// Type Unification
