//@ edition:2018

#![feature(try_blocks)]

fn main() {}

fn f1() {
    loop
        let x = 0; //~ ERROR expected `{`, found keyword `let`
        drop(0);
    }

fn f2() {
    while true
        let x = 0; //~ ERROR expected `{`, found keyword `let`
    }

fn f3() {
    for x in 0..1
        let x = 0; //~ ERROR expected `{`, found keyword `let`
    }

fn f4() {
    try //~ ERROR expected expression, found reserved keyword `try`
        let x = 0;
    }

fn f5() {
    async
        let x = 0; //~ ERROR expected one of `move`, `|`, or `||`, found keyword `let`
    }

// ferrocene-annotations: fls_aadan19t5006
// async Blocks
//
// ferrocene-annotations: fls_onfyolkcbeh3
// For Loops
//
// ferrocene-annotations: fls_sf4qnd43z2wc
// Infinite Loops
//
// ferrocene-annotations: fls_5jjm1kt43axd
// While Loops
