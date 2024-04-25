//! Built-in attributes and `cfg` flag gating.

use AttributeDuplicates::*;
use AttributeGate::*;
use AttributeType::*;

use crate::{Features, Stability};

use rustc_data_structures::fx::FxHashMap;
use rustc_span::symbol::{sym, Symbol};

use std::sync::LazyLock;

type GateFn = fn(&Features) -> bool;

macro_rules! cfg_fn {
    ($field: ident) => {
        (|features| features.$field) as GateFn
    };
}

pub type GatedCfg = (Symbol, Symbol, GateFn);

/// `cfg(...)`'s that are feature gated.
const GATED_CFGS: &[GatedCfg] = &[
    // (name in cfg, feature, function to check if the feature is enabled)
    (sym::overflow_checks, sym::cfg_overflow_checks, cfg_fn!(cfg_overflow_checks)),
    (sym::ub_checks, sym::cfg_ub_checks, cfg_fn!(cfg_ub_checks)),
    (sym::target_thread_local, sym::cfg_target_thread_local, cfg_fn!(cfg_target_thread_local)),
    (
        sym::target_has_atomic_equal_alignment,
        sym::cfg_target_has_atomic_equal_alignment,
        cfg_fn!(cfg_target_has_atomic_equal_alignment),
    ),
    (sym::target_has_atomic_load_store, sym::cfg_target_has_atomic, cfg_fn!(cfg_target_has_atomic)),
    (sym::sanitize, sym::cfg_sanitize, cfg_fn!(cfg_sanitize)),
    (sym::version, sym::cfg_version, cfg_fn!(cfg_version)),
    (sym::relocation_model, sym::cfg_relocation_model, cfg_fn!(cfg_relocation_model)),
    (sym::sanitizer_cfi_generalize_pointers, sym::cfg_sanitizer_cfi, cfg_fn!(cfg_sanitizer_cfi)),
    (sym::sanitizer_cfi_normalize_integers, sym::cfg_sanitizer_cfi, cfg_fn!(cfg_sanitizer_cfi)),
];

/// Find a gated cfg determined by the `pred`icate which is given the cfg's name.
pub fn find_gated_cfg(pred: impl Fn(Symbol) -> bool) -> Option<&'static GatedCfg> {
    GATED_CFGS.iter().find(|(cfg_sym, ..)| pred(*cfg_sym))
}

// If you change this, please modify `src/doc/unstable-book` as well. You must
// move that documentation into the relevant place in the other docs, and
// remove the chapter on the flag.

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum AttributeType {
    /// Normal, builtin attribute that is consumed
    /// by the compiler before the unused_attribute check
    Normal,

    /// Builtin attribute that is only allowed at the crate level
    CrateLevel,
}

#[derive(Clone, Copy)]
pub enum AttributeGate {
    /// Is gated by a given feature gate, reason
    /// and function to check if enabled
    Gated(Stability, Symbol, &'static str, fn(&Features) -> bool),

    /// Ungated attribute, can be used on all release channels
    Ungated,
}

// fn() is not Debug
impl std::fmt::Debug for AttributeGate {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::Gated(ref stab, name, expl, _) => {
                write!(fmt, "Gated({stab:?}, {name}, {expl})")
            }
            Self::Ungated => write!(fmt, "Ungated"),
        }
    }
}

impl AttributeGate {
    fn is_deprecated(&self) -> bool {
        matches!(*self, Self::Gated(Stability::Deprecated(_, _), ..))
    }
}

/// A template that the attribute input must match.
/// Only top-level shape (`#[attr]` vs `#[attr(...)]` vs `#[attr = ...]`) is considered now.
#[derive(Clone, Copy, Default)]
pub struct AttributeTemplate {
    /// If `true`, the attribute is allowed to be a bare word like `#[test]`.
    pub word: bool,
    /// If `Some`, the attribute is allowed to take a list of items like `#[allow(..)]`.
    pub list: Option<&'static str>,
    /// If `Some`, the attribute is allowed to be a name/value pair where the
    /// value is a string, like `#[must_use = "reason"]`.
    pub name_value_str: Option<&'static str>,
}

/// How to handle multiple duplicate attributes on the same item.
#[derive(Clone, Copy, Default)]
pub enum AttributeDuplicates {
    /// Duplicates of this attribute are allowed.
    ///
    /// This should only be used with attributes where duplicates have semantic
    /// meaning, or some kind of "additive" behavior. For example, `#[warn(..)]`
    /// can be specified multiple times, and it combines all the entries. Or use
    /// this if there is validation done elsewhere.
    #[default]
    DuplicatesOk,
    /// Duplicates after the first attribute will be an unused_attribute warning.
    ///
    /// This is usually used for "word" attributes, where they are used as a
    /// boolean marker, like `#[used]`. It is not necessarily wrong that there
    /// are duplicates, but the others should probably be removed.
    WarnFollowing,
    /// Same as `WarnFollowing`, but only issues warnings for word-style attributes.
    ///
    /// This is only for special cases, for example multiple `#[macro_use]` can
    /// be warned, but multiple `#[macro_use(...)]` should not because the list
    /// form has different meaning from the word form.
    WarnFollowingWordOnly,
    /// Duplicates after the first attribute will be an error.
    ///
    /// This should be used where duplicates would be ignored, but carry extra
    /// meaning that could cause confusion. For example, `#[stable(since="1.0")]
    /// #[stable(since="2.0")]`, which version should be used for `stable`?
    ErrorFollowing,
    /// Duplicates preceding the last instance of the attribute will be an error.
    ///
    /// This is the same as `ErrorFollowing`, except the last attribute is the
    /// one that is "used". This is typically used in cases like codegen
    /// attributes which usually only honor the last attribute.
    ErrorPreceding,
    /// Duplicates after the first attribute will be an unused_attribute warning
    /// with a note that this will be an error in the future.
    ///
    /// This should be used for attributes that should be `ErrorFollowing`, but
    /// because older versions of rustc silently accepted (and ignored) the
    /// attributes, this is used to transition.
    FutureWarnFollowing,
    /// Duplicates preceding the last instance of the attribute will be a
    /// warning, with a note that this will be an error in the future.
    ///
    /// This is the same as `FutureWarnFollowing`, except the last attribute is
    /// the one that is "used". Ideally these can eventually migrate to
    /// `ErrorPreceding`.
    FutureWarnPreceding,
}

/// A convenience macro for constructing attribute templates.
/// E.g., `template!(Word, List: "description")` means that the attribute
/// supports forms `#[attr]` and `#[attr(description)]`.
macro_rules! template {
    (Word) => { template!(@ true, None, None) };
    (List: $descr: expr) => { template!(@ false, Some($descr), None) };
    (NameValueStr: $descr: expr) => { template!(@ false, None, Some($descr)) };
    (Word, List: $descr: expr) => { template!(@ true, Some($descr), None) };
    (Word, NameValueStr: $descr: expr) => { template!(@ true, None, Some($descr)) };
    (List: $descr1: expr, NameValueStr: $descr2: expr) => {
        template!(@ false, Some($descr1), Some($descr2))
    };
    (Word, List: $descr1: expr, NameValueStr: $descr2: expr) => {
        template!(@ true, Some($descr1), Some($descr2))
    };
    (@ $word: expr, $list: expr, $name_value_str: expr) => { AttributeTemplate {
        word: $word, list: $list, name_value_str: $name_value_str
    } };
}

macro_rules! ungated {
    ($attr:ident, $typ:expr, $tpl:expr, $duplicates:expr, $encode_cross_crate:expr $(,)?) => {
        BuiltinAttribute {
            name: sym::$attr,
            encode_cross_crate: $encode_cross_crate,
            type_: $typ,
            template: $tpl,
            gate: Ungated,
            duplicates: $duplicates,
        }
    };
}

macro_rules! gated {
    ($attr:ident, $typ:expr, $tpl:expr, $duplicates:expr, $encode_cross_crate:expr, $gate:ident, $msg:expr $(,)?) => {
        BuiltinAttribute {
            name: sym::$attr,
            encode_cross_crate: $encode_cross_crate,
            type_: $typ,
            template: $tpl,
            duplicates: $duplicates,
            gate: Gated(Stability::Unstable, sym::$gate, $msg, cfg_fn!($gate)),
        }
    };
    ($attr:ident, $typ:expr, $tpl:expr, $duplicates:expr, $encode_cross_crate:expr, $msg:expr $(,)?) => {
        BuiltinAttribute {
            name: sym::$attr,
            encode_cross_crate: $encode_cross_crate,
            type_: $typ,
            template: $tpl,
            duplicates: $duplicates,
            gate: Gated(Stability::Unstable, sym::$attr, $msg, cfg_fn!($attr)),
        }
    };
}

macro_rules! rustc_attr {
    (TEST, $attr:ident, $typ:expr, $tpl:expr, $duplicate:expr, $encode_cross_crate:expr $(,)?) => {
        rustc_attr!(
            $attr,
            $typ,
            $tpl,
            $duplicate,
            $encode_cross_crate,
            concat!(
                "the `#[",
                stringify!($attr),
                "]` attribute is just used for rustc unit tests \
                and will never be stable",
            ),
        )
    };
    ($attr:ident, $typ:expr, $tpl:expr, $duplicates:expr, $encode_cross_crate:expr, $msg:expr $(,)?) => {
        BuiltinAttribute {
            name: sym::$attr,
            encode_cross_crate: $encode_cross_crate,
            type_: $typ,
            template: $tpl,
            duplicates: $duplicates,
            gate: Gated(Stability::Unstable, sym::rustc_attrs, $msg, cfg_fn!(rustc_attrs)),
        }
    };
}

macro_rules! experimental {
    ($attr:ident) => {
        concat!("the `#[", stringify!($attr), "]` attribute is an experimental feature")
    };
}

const IMPL_DETAIL: &str = "internal implementation detail";
const INTERNAL_UNSTABLE: &str = "this is an internal attribute that will never be stable";

#[derive(PartialEq)]
pub enum EncodeCrossCrate {
    Yes,
    No,
}

pub struct BuiltinAttribute {
    pub name: Symbol,
    /// Whether this attribute is encode cross crate.
    ///
    /// If so, it is encoded in the crate metadata.
    /// Otherwise, it can only be used in the local crate.
    pub encode_cross_crate: EncodeCrossCrate,
    pub type_: AttributeType,
    pub template: AttributeTemplate,
    pub duplicates: AttributeDuplicates,
    pub gate: AttributeGate,
}

/// Attributes that have a special meaning to rustc or rustdoc.
#[rustfmt::skip]
pub const BUILTIN_ATTRIBUTES: &[BuiltinAttribute] = &[
    // ==========================================================================
    // Stable attributes:
    // ==========================================================================

    // Conditional compilation:
    ungated!(cfg, Normal, template!(List: "predicate"), DuplicatesOk, EncodeCrossCrate::Yes),
    ungated!(cfg_attr, Normal, template!(List: "predicate, attr1, attr2, ..."), DuplicatesOk, EncodeCrossCrate::Yes),

    // Testing:
    ungated!(
        ignore, Normal, template!(Word, NameValueStr: "reason"), WarnFollowing,
        EncodeCrossCrate::No,
    ),
    ungated!(
        should_panic, Normal,
        template!(Word, List: r#"expected = "reason""#, NameValueStr: "reason"), FutureWarnFollowing,
        EncodeCrossCrate::No,
    ),
    // FIXME(Centril): This can be used on stable but shouldn't.
    ungated!(
        reexport_test_harness_main, CrateLevel, template!(NameValueStr: "name"), ErrorFollowing,
        EncodeCrossCrate::No,
    ),

    // Macros:
    ungated!(automatically_derived, Normal, template!(Word), WarnFollowing, EncodeCrossCrate::Yes),
    ungated!(
        macro_use, Normal, template!(Word, List: "name1, name2, ..."), WarnFollowingWordOnly,
        EncodeCrossCrate::No,
    ),
    ungated!(macro_escape, Normal, template!(Word), WarnFollowing, EncodeCrossCrate::No), // Deprecated synonym for `macro_use`.
    ungated!(
        macro_export, Normal, template!(Word, List: "local_inner_macros"),
        WarnFollowing, EncodeCrossCrate::Yes
    ),
    ungated!(proc_macro, Normal, template!(Word), ErrorFollowing, EncodeCrossCrate::No),
    ungated!(
        proc_macro_derive, Normal, template!(List: "TraitName, /*opt*/ attributes(name1, name2, ...)"),
        ErrorFollowing, EncodeCrossCrate::No,
    ),
    ungated!(proc_macro_attribute, Normal, template!(Word), ErrorFollowing, EncodeCrossCrate::No),

    // Lints:
    ungated!(
        warn, Normal, template!(List: r#"lint1, lint2, ..., /*opt*/ reason = "...""#),
        DuplicatesOk, EncodeCrossCrate::No,
    ),
    ungated!(
        allow, Normal, template!(List: r#"lint1, lint2, ..., /*opt*/ reason = "...""#),
        DuplicatesOk, EncodeCrossCrate::No,
    ),
    gated!(
        expect, Normal, template!(List: r#"lint1, lint2, ..., /*opt*/ reason = "...""#), DuplicatesOk,
        EncodeCrossCrate::No, lint_reasons, experimental!(expect)
    ),
    ungated!(
        forbid, Normal, template!(List: r#"lint1, lint2, ..., /*opt*/ reason = "...""#),
        DuplicatesOk, EncodeCrossCrate::No
    ),
    ungated!(
        deny, Normal, template!(List: r#"lint1, lint2, ..., /*opt*/ reason = "...""#),
        DuplicatesOk, EncodeCrossCrate::No
    ),
    ungated!(
        must_use, Normal, template!(Word, NameValueStr: "reason"),
        FutureWarnFollowing, EncodeCrossCrate::Yes
    ),
    gated!(
        must_not_suspend, Normal, template!(Word, NameValueStr: "reason"), WarnFollowing,
        EncodeCrossCrate::Yes, experimental!(must_not_suspend)
    ),
    ungated!(
        deprecated, Normal,
        template!(
            Word,
            List: r#"/*opt*/ since = "version", /*opt*/ note = "reason""#,
            NameValueStr: "reason"
        ),
        ErrorFollowing, EncodeCrossCrate::Yes
    ),

    // Crate properties:
    ungated!(
        crate_name, CrateLevel, template!(NameValueStr: "name"), FutureWarnFollowing,
        EncodeCrossCrate::No,
    ),
    ungated!(
        crate_type, CrateLevel, template!(NameValueStr: "bin|lib|..."), DuplicatesOk,
        EncodeCrossCrate::No,
    ),
    // crate_id is deprecated
    ungated!(
        crate_id, CrateLevel, template!(NameValueStr: "ignored"), FutureWarnFollowing,
        EncodeCrossCrate::No,
    ),

    // ABI, linking, symbols, and FFI
    ungated!(
        link, Normal,
        template!(List: r#"name = "...", /*opt*/ kind = "dylib|static|...", /*opt*/ wasm_import_module = "...", /*opt*/ import_name_type = "decorated|noprefix|undecorated""#),
        DuplicatesOk,
        EncodeCrossCrate::No,
    ),
    ungated!(
        link_name, Normal, template!(NameValueStr: "name"),
        FutureWarnPreceding, EncodeCrossCrate::Yes
    ),
    ungated!(no_link, Normal, template!(Word), WarnFollowing, EncodeCrossCrate::No),
    ungated!(repr, Normal, template!(List: "C"), DuplicatesOk, EncodeCrossCrate::No),
    ungated!(export_name, Normal, template!(NameValueStr: "name"), FutureWarnPreceding, EncodeCrossCrate::No),
    ungated!(link_section, Normal, template!(NameValueStr: "name"), FutureWarnPreceding, EncodeCrossCrate::No),
    ungated!(no_mangle, Normal, template!(Word), WarnFollowing, EncodeCrossCrate::No),
    ungated!(used, Normal, template!(Word, List: "compiler|linker"), WarnFollowing, EncodeCrossCrate::No),
    ungated!(link_ordinal, Normal, template!(List: "ordinal"), ErrorPreceding, EncodeCrossCrate::Yes),

    // Limits:
    ungated!(
        recursion_limit, CrateLevel, template!(NameValueStr: "N"), FutureWarnFollowing,
        EncodeCrossCrate::No
    ),
    ungated!(
        type_length_limit, CrateLevel, template!(NameValueStr: "N"), FutureWarnFollowing,
        EncodeCrossCrate::No
    ),
    gated!(
        move_size_limit, CrateLevel, template!(NameValueStr: "N"), ErrorFollowing,
        EncodeCrossCrate::No, large_assignments, experimental!(move_size_limit)
    ),

    // Entry point:
    gated!(
        unix_sigpipe, Normal, template!(NameValueStr: "inherit|sig_ign|sig_dfl"), ErrorFollowing,
        EncodeCrossCrate::Yes, experimental!(unix_sigpipe)
    ),
    ungated!(start, Normal, template!(Word), WarnFollowing, EncodeCrossCrate::No),
    ungated!(no_start, CrateLevel, template!(Word), WarnFollowing, EncodeCrossCrate::No),
    ungated!(no_main, CrateLevel, template!(Word), WarnFollowing, EncodeCrossCrate::No),

    // Modules, prelude, and resolution:
    ungated!(path, Normal, template!(NameValueStr: "file"), FutureWarnFollowing, EncodeCrossCrate::No),
    ungated!(no_std, CrateLevel, template!(Word), WarnFollowing, EncodeCrossCrate::No),
    ungated!(no_implicit_prelude, Normal, template!(Word), WarnFollowing, EncodeCrossCrate::No),
    ungated!(non_exhaustive, Normal, template!(Word), WarnFollowing, EncodeCrossCrate::Yes),

    // Runtime
    ungated!(
        windows_subsystem, CrateLevel,
        template!(NameValueStr: "windows|console"), FutureWarnFollowing,
        EncodeCrossCrate::No
    ),
    ungated!(panic_handler, Normal, template!(Word), WarnFollowing, EncodeCrossCrate::Yes), // RFC 2070

    // Code generation:
    ungated!(inline, Normal, template!(Word, List: "always|never"), FutureWarnFollowing, EncodeCrossCrate::No),
    ungated!(cold, Normal, template!(Word), WarnFollowing, EncodeCrossCrate::No),
    ungated!(no_builtins, CrateLevel, template!(Word), WarnFollowing, EncodeCrossCrate::Yes),
    ungated!(
        target_feature, Normal, template!(List: r#"enable = "name""#),
        DuplicatesOk, EncodeCrossCrate::No,
    ),
    ungated!(track_caller, Normal, template!(Word), WarnFollowing, EncodeCrossCrate::Yes),
    ungated!(instruction_set, Normal, template!(List: "set"), ErrorPreceding, EncodeCrossCrate::No),
    gated!(
        no_sanitize, Normal,
        template!(List: "address, kcfi, memory, thread"), DuplicatesOk,
        EncodeCrossCrate::No, experimental!(no_sanitize)
    ),
    gated!(
        coverage, Normal, template!(Word, List: "on|off"),
        WarnFollowing, EncodeCrossCrate::No,
        coverage_attribute, experimental!(coverage)
    ),

    ungated!(
        doc, Normal, template!(List: "hidden|inline|...", NameValueStr: "string"), DuplicatesOk,
        EncodeCrossCrate::Yes
    ),

    // Debugging
    ungated!(
        debugger_visualizer, Normal,
        template!(List: r#"natvis_file = "...", gdb_script_file = "...""#),
        DuplicatesOk, EncodeCrossCrate::No
    ),

    // ==========================================================================
    // Unstable attributes:
    // ==========================================================================

    // Linking:
    gated!(
        naked, Normal, template!(Word), WarnFollowing, EncodeCrossCrate::No,
        naked_functions, experimental!(naked)
    ),

    // Testing:
    gated!(
        test_runner, CrateLevel, template!(List: "path"), ErrorFollowing,
        EncodeCrossCrate::Yes, custom_test_frameworks,
        "custom test frameworks are an unstable feature",
    ),
    // RFC #1268
    gated!(
        marker, Normal, template!(Word), WarnFollowing, EncodeCrossCrate::No,
        marker_trait_attr, experimental!(marker)
    ),
    gated!(
        thread_local, Normal, template!(Word), WarnFollowing, EncodeCrossCrate::No,
        "`#[thread_local]` is an experimental feature, and does not currently handle destructors",
    ),
    gated!(
        no_core, CrateLevel, template!(Word), WarnFollowing,
        EncodeCrossCrate::No, experimental!(no_core)
    ),
    // RFC 2412
    gated!(
        optimize, Normal, template!(List: "size|speed"), ErrorPreceding,
        EncodeCrossCrate::No, optimize_attribute, experimental!(optimize)
    ),

    gated!(
        ffi_pure, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::No, experimental!(ffi_pure)
    ),
    gated!(
        ffi_const, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::No, experimental!(ffi_const)
    ),
    gated!(
        register_tool, CrateLevel, template!(List: "tool1, tool2, ..."), DuplicatesOk,
        EncodeCrossCrate::No, experimental!(register_tool),
    ),

    gated!(
        cmse_nonsecure_entry, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::No, experimental!(cmse_nonsecure_entry)
    ),
    // RFC 2632
    gated!(
        const_trait, Normal, template!(Word), WarnFollowing, EncodeCrossCrate::Yes, const_trait_impl,
        "`const_trait` is a temporary placeholder for marking a trait that is suitable for `const` \
        `impls` and all default bodies as `const`, which may be removed or renamed in the \
        future."
    ),
    // lang-team MCP 147
    gated!(
        deprecated_safe, Normal, template!(List: r#"since = "version", note = "...""#), ErrorFollowing,
        EncodeCrossCrate::Yes, experimental!(deprecated_safe),
    ),

    // `#[collapse_debuginfo]`
    gated!(
        collapse_debuginfo, Normal, template!(Word, List: "no|external|yes"), ErrorFollowing,
        EncodeCrossCrate::No, experimental!(collapse_debuginfo)
    ),

    // RFC 2397
    gated!(
        do_not_recommend, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::No, experimental!(do_not_recommend)
    ),

    // `#[cfi_encoding = ""]`
    gated!(
        cfi_encoding, Normal, template!(NameValueStr: "encoding"), ErrorPreceding,
        EncodeCrossCrate::Yes, experimental!(cfi_encoding)
    ),

    // `#[coroutine]` attribute to be applied to closures to make them coroutines instead
    gated!(
        coroutine, Normal, template!(Word), ErrorFollowing,
        EncodeCrossCrate::No, coroutines, experimental!(coroutines)
    ),

    // ==========================================================================
    // Internal attributes: Stability, deprecation, and unsafe:
    // ==========================================================================

    ungated!(
        feature, CrateLevel,
        template!(List: "name1, name2, ..."), DuplicatesOk, EncodeCrossCrate::No,
    ),
    // DuplicatesOk since it has its own validation
    ungated!(
        stable, Normal,
        template!(List: r#"feature = "name", since = "version""#), DuplicatesOk, EncodeCrossCrate::No,
    ),
    ungated!(
        unstable, Normal,
        template!(List: r#"feature = "name", reason = "...", issue = "N""#), DuplicatesOk,
        EncodeCrossCrate::Yes
    ),
    ungated!(
        rustc_const_unstable, Normal, template!(List: r#"feature = "name""#),
        DuplicatesOk, EncodeCrossCrate::Yes
    ),
    ungated!(
        rustc_const_stable, Normal,
        template!(List: r#"feature = "name""#), DuplicatesOk, EncodeCrossCrate::No,
    ),
    ungated!(
        rustc_default_body_unstable, Normal,
        template!(List: r#"feature = "name", reason = "...", issue = "N""#),
        DuplicatesOk, EncodeCrossCrate::No
    ),
    gated!(
        allow_internal_unstable, Normal, template!(Word, List: "feat1, feat2, ..."),
        DuplicatesOk, EncodeCrossCrate::Yes,
        "allow_internal_unstable side-steps feature gating and stability checks",
    ),
    gated!(
        rustc_allow_const_fn_unstable, Normal,
        template!(Word, List: "feat1, feat2, ..."), DuplicatesOk, EncodeCrossCrate::No,
        "rustc_allow_const_fn_unstable side-steps feature gating and stability checks"
    ),
    gated!(
        allow_internal_unsafe, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::No, "allow_internal_unsafe side-steps the unsafe_code lint",
    ),
    rustc_attr!(
        rustc_allowed_through_unstable_modules, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::No,
        "rustc_allowed_through_unstable_modules special cases accidental stabilizations of stable items \
        through unstable paths"
    ),

    // ==========================================================================
    // Internal attributes: Type system related:
    // ==========================================================================

    gated!(fundamental, Normal, template!(Word), WarnFollowing, EncodeCrossCrate::Yes, experimental!(fundamental)),
    gated!(
        may_dangle, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::No, dropck_eyepatch,
        "`may_dangle` has unstable semantics and may be removed in the future",
    ),

    rustc_attr!(
        rustc_never_type_options,
        Normal,
        template!(List: r#"/*opt*/ fallback = "unit|niko|never|no""#),
        ErrorFollowing,
        EncodeCrossCrate::No,
        "`rustc_never_type_options` is used to experiment with never type fallback and work on \
         never type stabilization, and will never be stable"
    ),

    // ==========================================================================
    // Internal attributes: Runtime related:
    // ==========================================================================

    rustc_attr!(
        rustc_allocator, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::No, IMPL_DETAIL
    ),
    rustc_attr!(
        rustc_nounwind, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::No, IMPL_DETAIL
    ),
    rustc_attr!(
        rustc_reallocator, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::No, IMPL_DETAIL
    ),
    rustc_attr!(
        rustc_deallocator, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::No, IMPL_DETAIL
    ),
    rustc_attr!(
        rustc_allocator_zeroed, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::No, IMPL_DETAIL
    ),
    gated!(
        default_lib_allocator, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::No, allocator_internals, experimental!(default_lib_allocator),
    ),
    gated!(
        needs_allocator, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::No, allocator_internals, experimental!(needs_allocator),
    ),
    gated!(
        panic_runtime, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::No, experimental!(panic_runtime)
    ),
    gated!(
        needs_panic_runtime, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::No, experimental!(needs_panic_runtime)
    ),
    gated!(
        compiler_builtins, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::No,
        "the `#[compiler_builtins]` attribute is used to identify the `compiler_builtins` crate \
        which contains compiler-rt intrinsics and will never be stable",
    ),
    gated!(
        profiler_runtime, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::No,
        "the `#[profiler_runtime]` attribute is used to identify the `profiler_builtins` crate \
        which contains the profiler runtime and will never be stable",
    ),

    // ==========================================================================
    // Internal attributes, Linkage:
    // ==========================================================================

    gated!(
        linkage, Normal, template!(NameValueStr: "external|internal|..."),
        ErrorPreceding, EncodeCrossCrate::No,
        "the `linkage` attribute is experimental and not portable across platforms",
    ),
    rustc_attr!(
        rustc_std_internal_symbol, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::No, INTERNAL_UNSTABLE
    ),

    // ==========================================================================
    // Internal attributes, Macro related:
    // ==========================================================================

    rustc_attr!(
        rustc_builtin_macro, Normal,
        template!(Word, List: "name, /*opt*/ attributes(name1, name2, ...)"), ErrorFollowing,
        EncodeCrossCrate::Yes, IMPL_DETAIL
    ),
    rustc_attr!(
        rustc_proc_macro_decls, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::No, INTERNAL_UNSTABLE
    ),
    rustc_attr!(
        rustc_macro_transparency, Normal,
        template!(NameValueStr: "transparent|semitransparent|opaque"), ErrorFollowing,
        EncodeCrossCrate::Yes, "used internally for testing macro hygiene",
    ),

    // ==========================================================================
    // Internal attributes, Diagnostics related:
    // ==========================================================================

    rustc_attr!(
        rustc_on_unimplemented, Normal,
        template!(
            List: r#"/*opt*/ message = "...", /*opt*/ label = "...", /*opt*/ note = "...""#,
            NameValueStr: "message"
        ),
        ErrorFollowing, EncodeCrossCrate::Yes,
        INTERNAL_UNSTABLE
    ),
    rustc_attr!(
        rustc_confusables, Normal,
        template!(List: r#""name1", "name2", ..."#),
        ErrorFollowing, EncodeCrossCrate::Yes,
        INTERNAL_UNSTABLE,
    ),
    // Enumerates "identity-like" conversion methods to suggest on type mismatch.
    rustc_attr!(
        rustc_conversion_suggestion, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::Yes, INTERNAL_UNSTABLE
    ),
    // Prevents field reads in the marked trait or method to be considered
    // during dead code analysis.
    rustc_attr!(
        rustc_trivial_field_reads, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::Yes, INTERNAL_UNSTABLE
    ),
    // Used by the `rustc::potential_query_instability` lint to warn methods which
    // might not be stable during incremental compilation.
    rustc_attr!(
        rustc_lint_query_instability, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::Yes, INTERNAL_UNSTABLE
    ),
    // Used by the `rustc::diagnostic_outside_of_impl` lints to assist in changes to diagnostic
    // APIs. Any function with this attribute will be checked by that lint.
    rustc_attr!(
        rustc_lint_diagnostics, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::Yes, INTERNAL_UNSTABLE
    ),
    // Used by the `rustc::bad_opt_access` lint to identify `DebuggingOptions` and `CodegenOptions`
    // types (as well as any others in future).
    rustc_attr!(
        rustc_lint_opt_ty, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::Yes, INTERNAL_UNSTABLE
    ),
    // Used by the `rustc::bad_opt_access` lint on fields
    // types (as well as any others in future).
    rustc_attr!(
        rustc_lint_opt_deny_field_access, Normal, template!(List: "message"),
        WarnFollowing, EncodeCrossCrate::Yes, INTERNAL_UNSTABLE
    ),

    // ==========================================================================
    // Internal attributes, Const related:
    // ==========================================================================

    rustc_attr!(
        rustc_promotable, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::No, IMPL_DETAIL),
    rustc_attr!(
        rustc_legacy_const_generics, Normal, template!(List: "N"), ErrorFollowing,
        EncodeCrossCrate::Yes, INTERNAL_UNSTABLE
    ),
    // Do not const-check this function's body. It will always get replaced during CTFE.
    rustc_attr!(
        rustc_do_not_const_check, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::Yes, INTERNAL_UNSTABLE
    ),
    // Ensure the argument to this function is &&str during const-check.
    rustc_attr!(
        rustc_const_panic_str, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::Yes, INTERNAL_UNSTABLE
    ),

    // ==========================================================================
    // Internal attributes, Layout related:
    // ==========================================================================

    rustc_attr!(
        rustc_layout_scalar_valid_range_start, Normal, template!(List: "value"), ErrorFollowing,
        EncodeCrossCrate::Yes,
        "the `#[rustc_layout_scalar_valid_range_start]` attribute is just used to enable \
        niche optimizations in libcore and libstd and will never be stable",
    ),
    rustc_attr!(
        rustc_layout_scalar_valid_range_end, Normal, template!(List: "value"), ErrorFollowing,
        EncodeCrossCrate::Yes,
        "the `#[rustc_layout_scalar_valid_range_end]` attribute is just used to enable \
        niche optimizations in libcore and libstd and will never be stable",
    ),
    rustc_attr!(
        rustc_nonnull_optimization_guaranteed, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::Yes,
        "the `#[rustc_nonnull_optimization_guaranteed]` attribute is just used to enable \
        niche optimizations in libcore and libstd and will never be stable",
    ),

    // ==========================================================================
    // Internal attributes, Misc:
    // ==========================================================================
    gated!(
        lang, Normal, template!(NameValueStr: "name"), DuplicatesOk, EncodeCrossCrate::No, lang_items,
        "lang items are subject to change",
    ),
    rustc_attr!(
        rustc_pass_by_value, Normal, template!(Word), ErrorFollowing,
        EncodeCrossCrate::Yes,
        "#[rustc_pass_by_value] is used to mark types that must be passed by value instead of reference."
    ),
    rustc_attr!(
        rustc_never_returns_null_ptr, Normal, template!(Word), ErrorFollowing,
        EncodeCrossCrate::Yes,
        "#[rustc_never_returns_null_ptr] is used to mark functions returning non-null pointers."
    ),
    rustc_attr!(
        rustc_coherence_is_core, AttributeType::CrateLevel, template!(Word), ErrorFollowing, EncodeCrossCrate::No,
        "#![rustc_coherence_is_core] allows inherent methods on builtin types, only intended to be used in `core`."
    ),
    rustc_attr!(
        rustc_coinductive, AttributeType::Normal, template!(Word), WarnFollowing, EncodeCrossCrate::No,
        "#![rustc_coinductive] changes a trait to be coinductive, allowing cycles in the trait solver."
    ),
    rustc_attr!(
        rustc_allow_incoherent_impl, AttributeType::Normal, template!(Word), ErrorFollowing, EncodeCrossCrate::No,
        "#[rustc_allow_incoherent_impl] has to be added to all impl items of an incoherent inherent impl."
    ),
    rustc_attr!(
        rustc_preserve_ub_checks, AttributeType::CrateLevel, template!(Word), ErrorFollowing, EncodeCrossCrate::No,
        "`#![rustc_preserve_ub_checks]` prevents the designated crate from evaluating whether UB checks are enabled when optimizing MIR",
    ),
    rustc_attr!(
        rustc_deny_explicit_impl,
        AttributeType::Normal,
        template!(List: "implement_via_object = (true|false)"),
        ErrorFollowing,
        EncodeCrossCrate::No,
        "#[rustc_deny_explicit_impl] enforces that a trait can have no user-provided impls"
    ),
    rustc_attr!(
        rustc_has_incoherent_inherent_impls, AttributeType::Normal, template!(Word),
        ErrorFollowing, EncodeCrossCrate::Yes,
        "#[rustc_has_incoherent_inherent_impls] allows the addition of incoherent inherent impls for \
         the given type by annotating all impl items with #[rustc_allow_incoherent_impl]."
    ),
    rustc_attr!(
        rustc_box, AttributeType::Normal, template!(Word), ErrorFollowing, EncodeCrossCrate::No,
        "#[rustc_box] allows creating boxes \
        and it is only intended to be used in `alloc`."
    ),

    BuiltinAttribute {
        name: sym::rustc_diagnostic_item,
        // FIXME: This can be `true` once we always use `tcx.is_diagnostic_item`.
        encode_cross_crate: EncodeCrossCrate::Yes,
        type_: Normal,
        template: template!(NameValueStr: "name"),
        duplicates: ErrorFollowing,
        gate: Gated(
            Stability::Unstable,
            sym::rustc_attrs,
            "diagnostic items compiler internal support for linting",
            cfg_fn!(rustc_attrs),
        ),
    },
    gated!(
        // Used in resolve:
        prelude_import, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::No, "`#[prelude_import]` is for use by rustc only",
    ),
    gated!(
        rustc_paren_sugar, Normal, template!(Word), WarnFollowing, EncodeCrossCrate::No,
        unboxed_closures, "unboxed_closures are still evolving",
    ),
    rustc_attr!(
        rustc_inherit_overflow_checks, Normal, template!(Word), WarnFollowing, EncodeCrossCrate::No,
        "the `#[rustc_inherit_overflow_checks]` attribute is just used to control \
        overflow checking behavior of several libcore functions that are inlined \
        across crates and will never be stable",
    ),
    rustc_attr!(
        rustc_reservation_impl, Normal,
        template!(NameValueStr: "reservation message"), ErrorFollowing, EncodeCrossCrate::Yes,
        "the `#[rustc_reservation_impl]` attribute is internally used \
         for reserving for `for<T> From<!> for T` impl"
    ),
    rustc_attr!(
        rustc_test_marker, Normal, template!(NameValueStr: "name"), WarnFollowing,
        EncodeCrossCrate::No, "the `#[rustc_test_marker]` attribute is used internally to track tests",
    ),
    rustc_attr!(
        rustc_unsafe_specialization_marker, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::No,
        "the `#[rustc_unsafe_specialization_marker]` attribute is used to check specializations"
    ),
    rustc_attr!(
        rustc_specialization_trait, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::No,
        "the `#[rustc_specialization_trait]` attribute is used to check specializations"
    ),
    rustc_attr!(
        rustc_main, Normal, template!(Word), WarnFollowing, EncodeCrossCrate::No,
        "the `#[rustc_main]` attribute is used internally to specify test entry point function",
    ),
    rustc_attr!(
        rustc_skip_array_during_method_dispatch, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::No,
        "the `#[rustc_skip_array_during_method_dispatch]` attribute is used to exclude a trait \
        from method dispatch when the receiver is an array, for compatibility in editions < 2021."
    ),
    rustc_attr!(
        rustc_must_implement_one_of, Normal, template!(List: "function1, function2, ..."),
        ErrorFollowing, EncodeCrossCrate::No,
        "the `#[rustc_must_implement_one_of]` attribute is used to change minimal complete \
        definition of a trait, it's currently in experimental form and should be changed before \
        being exposed outside of the std"
    ),
    rustc_attr!(
        rustc_doc_primitive, Normal, template!(NameValueStr: "primitive name"), ErrorFollowing,
        EncodeCrossCrate::Yes, r#"`rustc_doc_primitive` is a rustc internal attribute"#,
    ),
    rustc_attr!(
        rustc_safe_intrinsic, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::No,
        "the `#[rustc_safe_intrinsic]` attribute is used internally to mark intrinsics as safe"
    ),
    rustc_attr!(
        rustc_intrinsic, Normal, template!(Word), ErrorFollowing, EncodeCrossCrate::Yes,
        "the `#[rustc_intrinsic]` attribute is used to declare intrinsics with function bodies",
    ),
    rustc_attr!(
        rustc_no_mir_inline, Normal, template!(Word), WarnFollowing, EncodeCrossCrate::Yes,
        "#[rustc_no_mir_inline] prevents the MIR inliner from inlining a function while not affecting codegen"
    ),
    rustc_attr!(
        rustc_intrinsic_must_be_overridden, Normal, template!(Word), ErrorFollowing, EncodeCrossCrate::Yes,
        "the `#[rustc_intrinsic_must_be_overridden]` attribute is used to declare intrinsics without real bodies",
    ),

    // ==========================================================================
    // Internal attributes, Testing:
    // ==========================================================================

    rustc_attr!(TEST, rustc_effective_visibility, Normal, template!(Word), WarnFollowing, EncodeCrossCrate::Yes),
    rustc_attr!(
        TEST, rustc_outlives, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::No
    ),
    rustc_attr!(
        TEST, rustc_capture_analysis, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::No
    ),
    rustc_attr!(
        TEST, rustc_insignificant_dtor, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::Yes
    ),
    rustc_attr!(
        TEST, rustc_strict_coherence, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::Yes
    ),
    rustc_attr!(
        TEST, rustc_variance, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::No
    ),
    rustc_attr!(
        TEST, rustc_variance_of_opaques, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::No
    ),
    rustc_attr!(
        TEST, rustc_hidden_type_of_opaques, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::No
    ),
    rustc_attr!(
        TEST, rustc_layout, Normal, template!(List: "field1, field2, ..."),
        WarnFollowing, EncodeCrossCrate::Yes
    ),
    rustc_attr!(
        TEST, rustc_abi, Normal, template!(List: "field1, field2, ..."),
        WarnFollowing, EncodeCrossCrate::No
    ),
    rustc_attr!(
        TEST, rustc_regions, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::No
    ),
    rustc_attr!(
        TEST, rustc_error, Normal,
        template!(Word, List: "delayed_bug_from_inside_query"),
        WarnFollowingWordOnly, EncodeCrossCrate::Yes
    ),
    rustc_attr!(
        TEST, rustc_dump_user_args, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::No
    ),
    rustc_attr!(
        TEST, rustc_evaluate_where_clauses, Normal, template!(Word), WarnFollowing,
        EncodeCrossCrate::Yes
    ),
    rustc_attr!(
        TEST, rustc_if_this_changed, Normal, template!(Word, List: "DepNode"), DuplicatesOk,
        EncodeCrossCrate::No
    ),
    rustc_attr!(
        TEST, rustc_then_this_would_need, Normal, template!(List: "DepNode"), DuplicatesOk,
        EncodeCrossCrate::No
    ),
    rustc_attr!(
        TEST, rustc_clean, Normal,
        template!(List: r#"cfg = "...", /*opt*/ label = "...", /*opt*/ except = "...""#),
        DuplicatesOk, EncodeCrossCrate::No
    ),
    rustc_attr!(
        TEST, rustc_partition_reused, Normal,
        template!(List: r#"cfg = "...", module = "...""#), DuplicatesOk, EncodeCrossCrate::No
    ),
    rustc_attr!(
        TEST, rustc_partition_codegened, Normal,
        template!(List: r#"cfg = "...", module = "...""#), DuplicatesOk, EncodeCrossCrate::No
    ),
    rustc_attr!(
        TEST, rustc_expected_cgu_reuse, Normal,
        template!(List: r#"cfg = "...", module = "...", kind = "...""#), DuplicatesOk,
        EncodeCrossCrate::No
    ),
    rustc_attr!(
        TEST, rustc_symbol_name, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::No
    ),
    rustc_attr!(
        TEST, rustc_polymorphize_error, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::Yes
    ),
    rustc_attr!(
        TEST, rustc_def_path, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::No
    ),
    rustc_attr!(
        TEST, rustc_mir, Normal, template!(List: "arg1, arg2, ..."),
        DuplicatesOk, EncodeCrossCrate::Yes
    ),
    gated!(
        custom_mir, Normal, template!(List: r#"dialect = "...", phase = "...""#),
        ErrorFollowing, EncodeCrossCrate::No,
        "the `#[custom_mir]` attribute is just used for the Rust test suite",
    ),
    rustc_attr!(
        TEST, rustc_dump_program_clauses, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::No
    ),
    rustc_attr!(
        TEST, rustc_dump_env_program_clauses, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::No
    ),
    rustc_attr!(
        TEST, rustc_object_lifetime_default, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::No
    ),
    rustc_attr!(
        TEST, rustc_dump_vtable, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::Yes
    ),
    rustc_attr!(
        TEST, rustc_dummy, Normal, template!(Word /* doesn't matter*/),
        DuplicatesOk, EncodeCrossCrate::No
    ),
    gated!(
        omit_gdb_pretty_printer_section, Normal, template!(Word),
        WarnFollowing, EncodeCrossCrate::No,
        "the `#[omit_gdb_pretty_printer_section]` attribute is just used for the Rust test suite",
    ),
    rustc_attr!(
        TEST, pattern_complexity, CrateLevel, template!(NameValueStr: "N"),
        ErrorFollowing, EncodeCrossCrate::No,
    ),
];

pub fn deprecated_attributes() -> Vec<&'static BuiltinAttribute> {
    BUILTIN_ATTRIBUTES.iter().filter(|attr| attr.gate.is_deprecated()).collect()
}

pub fn is_builtin_attr_name(name: Symbol) -> bool {
    BUILTIN_ATTRIBUTE_MAP.get(&name).is_some()
}

/// Whether this builtin attribute is encoded cross crate.
/// This means it can be used cross crate.
pub fn encode_cross_crate(name: Symbol) -> bool {
    if let Some(attr) = BUILTIN_ATTRIBUTE_MAP.get(&name) {
        attr.encode_cross_crate == EncodeCrossCrate::Yes
    } else {
        true
    }
}

pub fn is_valid_for_get_attr(name: Symbol) -> bool {
    BUILTIN_ATTRIBUTE_MAP.get(&name).is_some_and(|attr| match attr.duplicates {
        WarnFollowing | ErrorFollowing | ErrorPreceding | FutureWarnFollowing
        | FutureWarnPreceding => true,
        DuplicatesOk | WarnFollowingWordOnly => false,
    })
}

pub static BUILTIN_ATTRIBUTE_MAP: LazyLock<FxHashMap<Symbol, &BuiltinAttribute>> =
    LazyLock::new(|| {
        let mut map = FxHashMap::default();
        for attr in BUILTIN_ATTRIBUTES.iter() {
            if map.insert(attr.name, attr).is_some() {
                panic!("duplicate builtin attribute `{}`", attr.name);
            }
        }
        map
    });
