//@ run-rustfix

extern crate std;
fn main() {}
//~^^ ERROR the name `std` is defined multiple times [E0259]

// ferrocene-annotations: fls_gklst7joeo33
// External Crates
