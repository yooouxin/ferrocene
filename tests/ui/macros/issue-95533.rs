//@ check-pass

#![no_implicit_prelude]
// the macro should not rely on the prelude being imported
::std::thread_local! { static P: () = (); }
::std::thread_local! { static Q: () = const { () }; }

fn main () {}

// ferrocene-annotations: fls_iikmhqsp1r5a
// Attribute no_implicit_prelude
