use std::env;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
<<<<<<< HEAD
    let target = env::var("TARGET").expect("TARGET was not set");
    if target.contains("linux")
        || target.contains("netbsd")
        || target.contains("dragonfly")
        || target.contains("openbsd")
        || target.contains("freebsd")
        || target.contains("solaris")
        || target.contains("illumos")
        || target.contains("apple-darwin")
        || target.contains("apple-ios")
        || target.contains("apple-tvos")
        || target.contains("apple-watchos")
        || target.contains("uwp")
        || target.contains("windows")
        || target.contains("fuchsia")
        || (target.contains("sgx") && target.contains("fortanix"))
        || target.contains("hermit")
        || target.contains("l4re")
        || target.contains("redox")
        || target.contains("haiku")
        || target.contains("vxworks")
        || target.contains("wasm32")
        || target.contains("wasm64")
        || target.contains("espidf")
        || target.contains("solid")
        || target.contains("nintendo-3ds")
        || target.contains("vita")
        || target.contains("aix")
        || target.contains("nto")
        || target.contains("xous")
        || target.contains("hurd")
        || target.contains("uefi")
        || target.contains("teeos")
        || target.contains("zkvm")
        || target.contains("ferrocenecoretest")
        // See src/bootstrap/synthetic_targets.rs
=======
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").expect("CARGO_CFG_TARGET_ARCH was not set");
    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("CARGO_CFG_TARGET_OS was not set");
    let target_vendor =
        env::var("CARGO_CFG_TARGET_VENDOR").expect("CARGO_CFG_TARGET_VENDOR was not set");
    let target_env = env::var("CARGO_CFG_TARGET_ENV").expect("CARGO_CFG_TARGET_ENV was not set");

    if target_os == "linux"
        || target_os == "android"
        || target_os == "netbsd"
        || target_os == "dragonfly"
        || target_os == "openbsd"
        || target_os == "freebsd"
        || target_os == "solaris"
        || target_os == "illumos"
        || target_os == "macos"
        || target_os == "ios"
        || target_os == "tvos"
        || target_os == "watchos"
        || target_os == "windows"
        || target_os == "fuchsia"
        || (target_vendor == "fortanix" && target_env == "sgx")
        || target_os == "hermit"
        || target_os == "l4re"
        || target_os == "redox"
        || target_os == "haiku"
        || target_os == "vxworks"
        || target_arch == "wasm32"
        || target_arch == "wasm64"
        || target_os == "espidf"
        || target_os.starts_with("solid")
        || (target_vendor == "nintendo" && target_env == "newlib")
        || target_os == "vita"
        || target_os == "aix"
        || target_os == "nto"
        || target_os == "xous"
        || target_os == "hurd"
        || target_os == "uefi"
        || target_os == "teeos"
        || target_os == "zkvm"

        // See src/bootstrap/src/core/build_steps/synthetic_targets.rs
>>>>>>> pull-upstream-temp--do-not-use-for-real-code
        || env::var("RUSTC_BOOTSTRAP_SYNTHETIC_TARGET").is_ok()
    {
        // These platforms don't have any special requirements.
    } else {
        // This is for Cargo's build-std support, to mark std as unstable for
        // typically no_std platforms.
        // This covers:
        // - os=none ("bare metal" targets)
        // - mipsel-sony-psp
        // - nvptx64-nvidia-cuda
        // - arch=avr
        // - JSON targets
        // - Any new targets that have not been explicitly added above.
        println!("cargo:rustc-cfg=feature=\"restricted-std\"");
    }
    println!("cargo:rustc-env=STD_ENV_ARCH={}", env::var("CARGO_CFG_TARGET_ARCH").unwrap());
    println!("cargo:rustc-cfg=backtrace_in_libstd");
}
