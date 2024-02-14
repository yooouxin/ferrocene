//! # Feature gates
//!
//! This crate declares the set of past and present unstable features in the compiler.
//! Feature gate checking itself is done in `rustc_ast_passes/src/feature_gate.rs`
//! at the moment.
//!
//! Features are enabled in programs via the crate-level attributes of
//! `#![feature(...)]` with a comma-separated list of features.
//!
//! For the purpose of future feature-tracking, once a feature gate is added,
//! even if it is stabilized or removed, *do not remove it*. Instead, move the
//! symbol to the `accepted` or `removed` modules respectively.

#![allow(internal_features)]
#![feature(rustdoc_internals)]
#![doc(rust_logo)]
#![feature(lazy_cell)]

mod accepted;
mod builtin_attrs;
mod removed;
mod unstable;

#[cfg(test)]
mod tests;

use rustc_span::symbol::Symbol;
use std::num::NonZeroU32;

#[derive(Debug, Clone)]
pub struct Feature {
    pub name: Symbol,
    pub since: &'static str,
    issue: Option<NonZeroU32>,
}

#[derive(Copy, Clone, Debug)]
pub enum Stability {
    Unstable,
    // First argument is tracking issue link; second argument is an optional
    // help message, which defaults to "remove this attribute".
    Deprecated(&'static str, Option<&'static str>),
}

#[derive(Clone, Copy, Debug, Hash)]
pub enum UnstableFeatures {
    /// Disallow use of unstable features, as on beta/stable channels.
    Disallow,
    /// Allow use of unstable features, as on nightly.
    Allow,
    /// Errors are bypassed for bootstrapping. This is required any time
    /// during the build that feature-related lints are set to warn or above
    /// because the build turns on warnings-as-errors and uses lots of unstable
    /// features. As a result, this is always required for building Rust itself.
    Cheat,
}

impl UnstableFeatures {
    /// This takes into account `RUSTC_BOOTSTRAP`.
    ///
    /// If `krate` is [`Some`], then setting `RUSTC_BOOTSTRAP=krate` will enable the nightly
    /// features. Otherwise, only `RUSTC_BOOTSTRAP=1` will work.
    pub fn from_environment(krate: Option<&str>) -> Self {
        // `true` if this is a feature-staged build, i.e., on the beta or stable channel.
        let disable_unstable_features =
            option_env!("CFG_DISABLE_UNSTABLE_FEATURES").is_some_and(|s| s != "0");
        // Returns whether `krate` should be counted as unstable
        let is_unstable_crate =
            |var: &str| krate.is_some_and(|name| var.split(',').any(|new_krate| new_krate == name));
        // `true` if we should enable unstable features for bootstrapping.
        let bootstrap =
            std::env::var("RUSTC_BOOTSTRAP").is_ok_and(|var| var == "1" || is_unstable_crate(&var));
        match (disable_unstable_features, bootstrap) {
            (_, true) => UnstableFeatures::Cheat,
            (true, _) => UnstableFeatures::Disallow,
            (false, _) => UnstableFeatures::Allow,
        }
    }

    pub fn is_nightly_build(&self) -> bool {
        match *self {
            UnstableFeatures::Allow | UnstableFeatures::Cheat => true,
            UnstableFeatures::Disallow => false,
        }
    }
}

fn find_lang_feature_issue(feature: Symbol) -> Option<NonZeroU32> {
    // Search in all the feature lists.
    if let Some(f) = UNSTABLE_FEATURES.iter().find(|f| f.feature.name == feature) {
        return f.feature.issue;
    }
    if let Some(f) = ACCEPTED_FEATURES.iter().find(|f| f.name == feature) {
        return f.issue;
    }
    if let Some(f) = REMOVED_FEATURES.iter().find(|f| f.feature.name == feature) {
        return f.feature.issue;
    }
    panic!("feature `{feature}` is not declared anywhere");
}

const fn to_nonzero(n: Option<u32>) -> Option<NonZeroU32> {
    // Can be replaced with `n.and_then(NonZeroU32::new)` if that is ever usable
    // in const context. Requires https://github.com/rust-lang/rfcs/pull/2632.
    match n {
        None => None,
        Some(n) => NonZeroU32::new(n),
    }
}

pub enum GateIssue {
    Language,
    Library(Option<NonZeroU32>),
}

pub fn find_feature_issue(feature: Symbol, issue: GateIssue) -> Option<NonZeroU32> {
    match issue {
        GateIssue::Language => find_lang_feature_issue(feature),
        GateIssue::Library(lib) => lib,
    }
}

pub use accepted::ACCEPTED_FEATURES;
pub use builtin_attrs::AttributeDuplicates;
pub use builtin_attrs::{
    deprecated_attributes, find_gated_cfg, is_builtin_attr_name, is_builtin_only_local,
    is_valid_for_get_attr, AttributeGate, AttributeTemplate, AttributeType, BuiltinAttribute,
    GatedCfg, BUILTIN_ATTRIBUTES, BUILTIN_ATTRIBUTE_MAP,
};
pub use removed::REMOVED_FEATURES;
pub use unstable::{Features, INCOMPATIBLE_FEATURES, UNSTABLE_FEATURES};
