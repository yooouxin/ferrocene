//@ check-pass
//@ edition:2021

fn main() {
    let _: u16 = 123i32.try_into().unwrap();
}

// ferrocene-annotations: fls_ld0ize96cm6m
// Preludes
