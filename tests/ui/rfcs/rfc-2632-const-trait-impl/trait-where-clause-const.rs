// Like trait-where-clause.rs, but we are calling from a const context.
// Checking the validity of traits' where clauses happen at a later stage.
// (`rustc_const_eval` instead of `rustc_hir_analysis`) Therefore one file as a
// test is not enough.
//@ known-bug: #110395
// FIXME check-pass
#![feature(const_trait_impl, effects)]

#[const_trait]
trait Bar {}

#[const_trait]
trait Foo {
    fn a();
    fn b() where Self: ~const Bar;
    fn c<T: ~const Bar>();
}

const fn test1<T: ~const Foo + Bar>() {
    T::a();
    T::b();
    //FIXME ~^ ERROR the trait bound
    T::c::<T>();
    //FIXME ~^ ERROR the trait bound
}

const fn test2<T: ~const Foo + ~const Bar>() {
    T::a();
    T::b();
    T::c::<T>();
}

fn main() {}
