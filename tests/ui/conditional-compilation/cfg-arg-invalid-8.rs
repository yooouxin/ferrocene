//@ compile-flags: --error-format=human --cfg )
//@ error-pattern: invalid `--cfg` argument: `)` (expected `key` or `key="value"`)
fn main() {}

// ferrocene-annotations: um_rustc_cfg
