//@ run-pass

mod foo {
    pub fn test() {
        let x = std::cmp::min(2, 3);
        assert_eq!(x, 2);
    }
}

fn main() {
    foo::test();
}

// ferrocene-annotations: fls_ld0ize96cm6m
// Preludes
