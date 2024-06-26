//@ aux-build:variants.rs

extern crate variants;

use variants::NonExhaustiveVariants;

fn main() {
    let variant_struct = NonExhaustiveVariants::Struct { field: 640 };
    //~^ ERROR cannot create non-exhaustive variant

    let variant_tuple = NonExhaustiveVariants::Tuple(640);
    //~^ ERROR tuple variant `Tuple` is private [E0603]

    let variant_unit = NonExhaustiveVariants::Unit;
    //~^ ERROR unit variant `Unit` is private [E0603]

    match variant_struct {
        NonExhaustiveVariants::Unit => "",
        //~^ ERROR unit variant `Unit` is private [E0603]
        NonExhaustiveVariants::Tuple(fe_tpl) => "",
        //~^ ERROR tuple variant `Tuple` is private [E0603]
        NonExhaustiveVariants::Struct { field } => ""
        //~^ ERROR `..` required with variant marked as non-exhaustive
    };

    if let NonExhaustiveVariants::Tuple(fe_tpl) = variant_struct {
        //~^ ERROR tuple variant `Tuple` is private [E0603]
    }

    if let NonExhaustiveVariants::Struct { field } = variant_struct {
        //~^ ERROR `..` required with variant marked as non-exhaustive
    }
}

// ferrocene-annotations: fls_e5td0fa92fay
// Match Expressions
// Enum Types
// ferrocene-annotations: fls_yivm43r5wnp1
// Let Statements
// ferrocene-annotations: fls_eexupzdsu7f
// Tuple Struct Pattern Matching
// ferrocene-annotations: fls_vlrto778v49m
// Tuple Struct Patterns
// ferrocene-annotations: fls_asj8rgccvkoe
// Struct Pattern Matching
// ferrocene-annotations: fls_nruvg0es3kx7
// Record Struct Patterns
