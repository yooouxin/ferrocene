//@ compile-flags: --cfg a::b
//@ error-pattern: invalid `--cfg` argument: `a::b` (argument key must be an identifier)
fn main() {}

// ferrocene-annotations: um_rustc_cfg
