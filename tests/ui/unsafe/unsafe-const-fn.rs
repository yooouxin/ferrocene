// A quick test of 'unsafe const fn' functionality

const unsafe fn dummy(v: u32) -> u32 {
    !v
}

const VAL: u32 = dummy(0xFFFF);
//~^ ERROR E0133

fn main() {
    assert_eq!(VAL, 0xFFFF0000);
}

// ferrocene-annotations: fls_qcb1n9c0e5hz
// Functions
//
// ferrocene-annotations: fls_ixjc5jaamx84
// Constants
