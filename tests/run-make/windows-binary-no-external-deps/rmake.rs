//! Ensure that we aren't relying on any non-system DLLs when running
//! a "hello world" application by setting `PATH` to `C:\Windows\System32`.
//@ only-windows

use run_make_support::{env_var, rustc, tmp_dir};
use std::path::PathBuf;
use std::process::Command;

fn main() {
    rustc().input("hello.rs").run();

    let windows_dir = env_var("SystemRoot");
    let system32: PathBuf = [&windows_dir, "System32"].iter().collect();
    // Note: This does not use the support wrappers so that we can precisely control the PATH
    let exe = tmp_dir().join("hello.exe");
    let status = Command::new(exe).env("PATH", &system32).spawn().unwrap().wait().unwrap();
    if !status.success() {
        panic!("Command failed!\noutput status: `{status}`");
    }
}
