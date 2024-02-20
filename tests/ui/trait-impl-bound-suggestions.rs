// run-rustfix

#[allow(unused)]
use std::fmt::Debug;
// Rustfix should add this, or use `std::fmt::Debug` instead.

#[allow(dead_code)]
struct ConstrainedStruct<X: Copy> {
    x: X
}

#[allow(dead_code)]
trait InsufficientlyConstrainedGeneric<X=()> where Self: Sized {
    fn return_the_constrained_type(&self, x: X) -> ConstrainedStruct<X> {
        //~^ ERROR the trait bound `X: Copy` is not satisfied
        ConstrainedStruct { x }
        //~^ ERROR the trait bound `X: Copy` is not satisfied
    }
}

// Regression test for #120838
#[allow(dead_code)]
trait InsufficientlyConstrainedGenericWithEmptyWhere<X=()> where Self: Sized {
    fn return_the_constrained_type(&self, x: X) -> ConstrainedStruct<X> {
        //~^ ERROR the trait bound `X: Copy` is not satisfied
        ConstrainedStruct { x }
        //~^ ERROR the trait bound `X: Copy` is not satisfied
    }
}

pub fn main() { }
