//@ run-pass
//@ compile-flags: --cfg bar --check-cfg=cfg(bar) -D warnings

#![cfg(bar)]

fn main() {}

// ferrocene-annotations: um_rustc_cfg
// ferrocene-annotations: um_rustc_D
