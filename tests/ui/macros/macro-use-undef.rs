//@ aux-build:two_macros.rs

#[macro_use(macro_two, no_way)] //~ ERROR imported macro not found
extern crate two_macros;

pub fn main() {
    macro_two!();
}

// ferrocene-annotations: fls_qxjy0f758x5s
// Attribute macro_use
