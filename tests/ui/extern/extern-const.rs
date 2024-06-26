// Check extern items cannot be const + `rustfix` suggests using
// extern static.
//
// #54388: an unused reference to an undefined static may or may not
// compile. To sidestep this by using one that *is* defined.

//@ run-rustfix
//@ compile-flags: -g

use std::ffi::c_int;

#[link(name = "rust_test_helpers", kind = "static")]
extern "C" {
    const rust_dbg_static_mut: c_int; //~ ERROR extern items cannot be `const`
}

fn main() {
    // We suggest turning the (illegal) extern `const` into an extern `static`,
    // but this also requires `unsafe` (a deny-by-default lint at comment time,
    // future error; Issue #36247)
    unsafe {
        let _x = rust_dbg_static_mut;
    }
}

// ferrocene-annotations: fls_tmoh3y9oyqsy
// External Blocks
