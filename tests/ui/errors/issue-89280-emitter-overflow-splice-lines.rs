//@ check-pass

trait X {
    fn test(x: u32, (
//~^ WARN anonymous parameters are deprecated and will be removed in the next edition
//~^^ WARN this is accepted in the current edition (Rust 2015) but is a hard error in Rust 2018!
    )) {}
}

fn main() {}

// ferrocene-annotations: fls_qcb1n9c0e5hz
// Functions
// ferrocene-annotations: fls_l21tjqjkkaa0
// Associated Items
