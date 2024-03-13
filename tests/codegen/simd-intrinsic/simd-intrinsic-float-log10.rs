//@ compile-flags: -C no-prepopulate-passes

#![crate_type = "lib"]

#![feature(repr_simd, platform_intrinsics)]
#![allow(non_camel_case_types)]

#[repr(simd)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct f32x2(pub f32, pub f32);

#[repr(simd)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct f32x4(pub f32, pub f32, pub f32, pub f32);

#[repr(simd)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct f32x8(pub f32, pub f32, pub f32, pub f32,
                 pub f32, pub f32, pub f32, pub f32);

#[repr(simd)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct f32x16(pub f32, pub f32, pub f32, pub f32,
                  pub f32, pub f32, pub f32, pub f32,
                  pub f32, pub f32, pub f32, pub f32,
                  pub f32, pub f32, pub f32, pub f32);

extern "platform-intrinsic" {
    fn simd_flog10<T>(x: T) -> T;
}

// CHECK-LABEL: @log10_32x2
#[no_mangle]
pub unsafe fn log10_32x2(a: f32x2) -> f32x2 {
    // CHECK: call <2 x float> @llvm.log10.v2f32
    simd_flog10(a)
}

// CHECK-LABEL: @log10_32x4
#[no_mangle]
pub unsafe fn log10_32x4(a: f32x4) -> f32x4 {
    // CHECK: call <4 x float> @llvm.log10.v4f32
    simd_flog10(a)
}

// CHECK-LABEL: @log10_32x8
#[no_mangle]
pub unsafe fn log10_32x8(a: f32x8) -> f32x8 {
    // CHECK: call <8 x float> @llvm.log10.v8f32
    simd_flog10(a)
}

// CHECK-LABEL: @log10_32x16
#[no_mangle]
pub unsafe fn log10_32x16(a: f32x16) -> f32x16 {
    // CHECK: call <16 x float> @llvm.log10.v16f32
    simd_flog10(a)
}

#[repr(simd)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct f64x2(pub f64, pub f64);

#[repr(simd)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct f64x4(pub f64, pub f64, pub f64, pub f64);

#[repr(simd)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct f64x8(pub f64, pub f64, pub f64, pub f64,
                 pub f64, pub f64, pub f64, pub f64);

// CHECK-LABEL: @log10_64x4
#[no_mangle]
pub unsafe fn log10_64x4(a: f64x4) -> f64x4 {
    // CHECK: call <4 x double> @llvm.log10.v4f64
    simd_flog10(a)
}

// CHECK-LABEL: @log10_64x2
#[no_mangle]
pub unsafe fn log10_64x2(a: f64x2) -> f64x2 {
    // CHECK: call <2 x double> @llvm.log10.v2f64
    simd_flog10(a)
}

// CHECK-LABEL: @log10_64x8
#[no_mangle]
pub unsafe fn log10_64x8(a: f64x8) -> f64x8 {
    // CHECK: call <8 x double> @llvm.log10.v8f64
    simd_flog10(a)
}
