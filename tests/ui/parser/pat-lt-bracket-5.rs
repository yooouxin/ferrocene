fn main() {
    let v[0] = v[1];
    //~^ error: expected a pattern, found an expression
    //~| error: cannot find value `v` in this scope
}

// ferrocene-annotations: fls_yivm43r5wnp1
// Let Statements
