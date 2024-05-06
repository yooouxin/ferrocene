// Check to see if we can get parameters from an @argsfile file
//
//@ build-pass
//@ compile-flags: --cfg cmdline_set --check-cfg=cfg(cmdline_set,unbroken)
//@ compile-flags: @{{src-base}}/argfile/commandline-argfile.args

#[cfg(not(cmdline_set))]
compile_error!("cmdline_set not set");

#[cfg(not(unbroken))]
compile_error!("unbroken not set");

<<<<<<< HEAD
fn main() {
}

// ferrocene-annotations: um_rustc_cfg
=======
fn main() {}
>>>>>>> pull-upstream-temp--do-not-use-for-real-code
