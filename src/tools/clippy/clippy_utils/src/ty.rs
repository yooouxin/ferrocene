//! Util methods for [`rustc_middle::ty`]

#![allow(clippy::module_name_repetitions)]

use core::ops::ControlFlow;
use itertools::Itertools;
use rustc_ast::ast::Mutability;
use rustc_data_structures::fx::{FxHashMap, FxHashSet};
use rustc_hir as hir;
use rustc_hir::def::{CtorKind, CtorOf, DefKind, Res};
use rustc_hir::def_id::DefId;
use rustc_hir::{Expr, FnDecl, LangItem, TyKind, Unsafety};
use rustc_infer::infer::type_variable::{TypeVariableOrigin, TypeVariableOriginKind};
use rustc_infer::infer::TyCtxtInferExt;
use rustc_lint::LateContext;
use rustc_middle::mir::interpret::Scalar;
use rustc_middle::mir::ConstValue;
use rustc_middle::traits::EvaluationResult;
use rustc_middle::ty::layout::ValidityRequirement;
use rustc_middle::ty::{
    self, AdtDef, AliasTy, AssocKind, Binder, BoundRegion, FnSig, GenericArg, GenericArgKind, GenericArgsRef,
    GenericParamDefKind, IntTy, List, ParamEnv, Region, RegionKind, ToPredicate, TraitRef, Ty, TyCtxt,
    TypeSuperVisitable, TypeVisitable, TypeVisitableExt, TypeVisitor, UintTy, VariantDef, VariantDiscr,
};
use rustc_span::symbol::Ident;
use rustc_span::{sym, Span, Symbol, DUMMY_SP};
use rustc_target::abi::{Size, VariantIdx};
use rustc_trait_selection::traits::query::evaluate_obligation::InferCtxtExt as _;
use rustc_trait_selection::traits::query::normalize::QueryNormalizeExt;
use rustc_trait_selection::traits::{Obligation, ObligationCause};
use std::assert_matches::debug_assert_matches;
use std::iter;

use crate::{match_def_path, path_res};

mod type_certainty;
pub use type_certainty::expr_type_is_certain;

/// Checks if the given type implements copy.
pub fn is_copy<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    ty.is_copy_modulo_regions(cx.tcx, cx.param_env)
}

/// This checks whether a given type is known to implement Debug.
pub fn has_debug_impl<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    cx.tcx
        .get_diagnostic_item(sym::Debug)
        .map_or(false, |debug| implements_trait(cx, ty, debug, &[]))
}

/// Checks whether a type can be partially moved.
pub fn can_partially_move_ty<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    if has_drop(cx, ty) || is_copy(cx, ty) {
        return false;
    }
    match ty.kind() {
        ty::Param(_) => false,
        ty::Adt(def, subs) => def.all_fields().any(|f| !is_copy(cx, f.ty(cx.tcx, subs))),
        _ => true,
    }
}

/// Walks into `ty` and returns `true` if any inner type is an instance of the given adt
/// constructor.
pub fn contains_adt_constructor<'tcx>(ty: Ty<'tcx>, adt: AdtDef<'tcx>) -> bool {
    ty.walk().any(|inner| match inner.unpack() {
        GenericArgKind::Type(inner_ty) => inner_ty.ty_adt_def() == Some(adt),
        GenericArgKind::Lifetime(_) | GenericArgKind::Const(_) => false,
    })
}

/// Walks into `ty` and returns `true` if any inner type is an instance of the given type, or adt
/// constructor of the same type.
///
/// This method also recurses into opaque type predicates, so call it with `impl Trait<U>` and `U`
/// will also return `true`.
pub fn contains_ty_adt_constructor_opaque<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>, needle: Ty<'tcx>) -> bool {
    fn contains_ty_adt_constructor_opaque_inner<'tcx>(
        cx: &LateContext<'tcx>,
        ty: Ty<'tcx>,
        needle: Ty<'tcx>,
        seen: &mut FxHashSet<DefId>,
    ) -> bool {
        ty.walk().any(|inner| match inner.unpack() {
            GenericArgKind::Type(inner_ty) => {
                if inner_ty == needle {
                    return true;
                }

                if inner_ty.ty_adt_def() == needle.ty_adt_def() {
                    return true;
                }

                if let ty::Alias(ty::Opaque, ty::AliasTy { def_id, .. }) = *inner_ty.kind() {
                    if !seen.insert(def_id) {
                        return false;
                    }

                    for (predicate, _span) in cx.tcx.explicit_item_bounds(def_id).instantiate_identity_iter_copied() {
                        match predicate.kind().skip_binder() {
                            // For `impl Trait<U>`, it will register a predicate of `T: Trait<U>`, so we go through
                            // and check substitutions to find `U`.
                            ty::ClauseKind::Trait(trait_predicate) => {
                                if trait_predicate
                                    .trait_ref
                                    .args
                                    .types()
                                    .skip(1) // Skip the implicit `Self` generic parameter
                                    .any(|ty| contains_ty_adt_constructor_opaque_inner(cx, ty, needle, seen))
                                {
                                    return true;
                                }
                            },
                            // For `impl Trait<Assoc=U>`, it will register a predicate of `<T as Trait>::Assoc = U`,
                            // so we check the term for `U`.
                            ty::ClauseKind::Projection(projection_predicate) => {
                                if let ty::TermKind::Ty(ty) = projection_predicate.term.unpack() {
                                    if contains_ty_adt_constructor_opaque_inner(cx, ty, needle, seen) {
                                        return true;
                                    }
                                };
                            },
                            _ => (),
                        }
                    }
                }

                false
            },
            GenericArgKind::Lifetime(_) | GenericArgKind::Const(_) => false,
        })
    }

    // A hash set to ensure that the same opaque type (`impl Trait` in RPIT or TAIT) is not
    // visited twice.
    let mut seen = FxHashSet::default();
    contains_ty_adt_constructor_opaque_inner(cx, ty, needle, &mut seen)
}

/// Resolves `<T as Iterator>::Item` for `T`
/// Do not invoke without first verifying that the type implements `Iterator`
pub fn get_iterator_item_ty<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
    cx.tcx
        .get_diagnostic_item(sym::Iterator)
        .and_then(|iter_did| cx.get_associated_type(ty, iter_did, "Item"))
}

/// Get the diagnostic name of a type, e.g. `sym::HashMap`. To check if a type
/// implements a trait marked with a diagnostic item use [`implements_trait`].
///
/// For a further exploitation what diagnostic items are see [diagnostic items] in
/// rustc-dev-guide.
///
/// [Diagnostic Items]: https://rustc-dev-guide.rust-lang.org/diagnostics/diagnostic-items.html
pub fn get_type_diagnostic_name(cx: &LateContext<'_>, ty: Ty<'_>) -> Option<Symbol> {
    match ty.kind() {
        ty::Adt(adt, _) => cx.tcx.get_diagnostic_name(adt.did()),
        _ => None,
    }
}

/// Returns true if ty has `iter` or `iter_mut` methods
pub fn has_iter_method(cx: &LateContext<'_>, probably_ref_ty: Ty<'_>) -> Option<Symbol> {
    // FIXME: instead of this hard-coded list, we should check if `<adt>::iter`
    // exists and has the desired signature. Unfortunately FnCtxt is not exported
    // so we can't use its `lookup_method` method.
    let into_iter_collections: &[Symbol] = &[
        sym::Vec,
        sym::Option,
        sym::Result,
        sym::BTreeMap,
        sym::BTreeSet,
        sym::VecDeque,
        sym::LinkedList,
        sym::BinaryHeap,
        sym::HashSet,
        sym::HashMap,
        sym::PathBuf,
        sym::Path,
        sym::Receiver,
    ];

    let ty_to_check = match probably_ref_ty.kind() {
        ty::Ref(_, ty_to_check, _) => *ty_to_check,
        _ => probably_ref_ty,
    };

    let def_id = match ty_to_check.kind() {
        ty::Array(..) => return Some(sym::array),
        ty::Slice(..) => return Some(sym::slice),
        ty::Adt(adt, _) => adt.did(),
        _ => return None,
    };

    for &name in into_iter_collections {
        if cx.tcx.is_diagnostic_item(name, def_id) {
            return Some(cx.tcx.item_name(def_id));
        }
    }
    None
}

/// Checks whether a type implements a trait.
/// The function returns false in case the type contains an inference variable.
///
/// See:
/// * [`get_trait_def_id`](super::get_trait_def_id) to get a trait [`DefId`].
/// * [Common tools for writing lints] for an example how to use this function and other options.
///
/// [Common tools for writing lints]: https://github.com/rust-lang/rust-clippy/blob/master/book/src/development/common_tools_writing_lints.md#checking-if-a-type-implements-a-specific-trait
pub fn implements_trait<'tcx>(
    cx: &LateContext<'tcx>,
    ty: Ty<'tcx>,
    trait_id: DefId,
    args: &[GenericArg<'tcx>],
) -> bool {
    implements_trait_with_env_from_iter(cx.tcx, cx.param_env, ty, trait_id, None, args.iter().map(|&x| Some(x)))
}

/// Same as `implements_trait` but allows using a `ParamEnv` different from the lint context.
///
/// The `callee_id` argument is used to determine whether this is a function call in a `const fn` environment, used for checking const traits.
pub fn implements_trait_with_env<'tcx>(
    tcx: TyCtxt<'tcx>,
    param_env: ParamEnv<'tcx>,
    ty: Ty<'tcx>,
    trait_id: DefId,
    callee_id: Option<DefId>,
    args: &[GenericArg<'tcx>],
) -> bool {
    implements_trait_with_env_from_iter(tcx, param_env, ty, trait_id, callee_id, args.iter().map(|&x| Some(x)))
}

/// Same as `implements_trait_from_env` but takes the arguments as an iterator.
pub fn implements_trait_with_env_from_iter<'tcx>(
    tcx: TyCtxt<'tcx>,
    param_env: ParamEnv<'tcx>,
    ty: Ty<'tcx>,
    trait_id: DefId,
    callee_id: Option<DefId>,
    args: impl IntoIterator<Item = impl Into<Option<GenericArg<'tcx>>>>,
) -> bool {
    // Clippy shouldn't have infer types
    assert!(!ty.has_infer());

    // If a `callee_id` is passed, then we assert that it is a body owner
    // through calling `body_owner_kind`, which would panic if the callee
    // does not have a body.
    if let Some(callee_id) = callee_id {
        let _ = tcx.hir().body_owner_kind(callee_id);
    }

    let ty = tcx.erase_regions(ty);
    if ty.has_escaping_bound_vars() {
        return false;
    }

    let infcx = tcx.infer_ctxt().build();
    let args = args
        .into_iter()
        .map(|arg| {
            arg.into().unwrap_or_else(|| {
                let orig = TypeVariableOrigin {
                    kind: TypeVariableOriginKind::MiscVariable,
                    span: DUMMY_SP,
                };
                infcx.next_ty_var(orig).into()
            })
        })
        .collect::<Vec<_>>();

    // If an effect arg was not specified, we need to specify it.
    let effect_arg = if tcx
        .generics_of(trait_id)
        .host_effect_index
        .is_some_and(|x| args.get(x - 1).is_none())
    {
        Some(GenericArg::from(callee_id.map_or(tcx.consts.true_, |def_id| {
            tcx.expected_host_effect_param_for_body(def_id)
        })))
    } else {
        None
    };

    let trait_ref = TraitRef::new(
        tcx,
        trait_id,
        Some(GenericArg::from(ty)).into_iter().chain(args).chain(effect_arg),
    );

    debug_assert_matches!(
        tcx.def_kind(trait_id),
        DefKind::Trait | DefKind::TraitAlias,
        "`DefId` must belong to a trait or trait alias"
    );
    #[cfg(debug_assertions)]
    assert_generic_args_match(tcx, trait_id, trait_ref.args);

    let obligation = Obligation {
        cause: ObligationCause::dummy(),
        param_env,
        recursion_depth: 0,
        predicate: ty::Binder::dummy(trait_ref).to_predicate(tcx),
    };
    infcx
        .evaluate_obligation(&obligation)
        .is_ok_and(EvaluationResult::must_apply_modulo_regions)
}

/// Checks whether this type implements `Drop`.
pub fn has_drop<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    match ty.ty_adt_def() {
        Some(def) => def.has_dtor(cx.tcx),
        None => false,
    }
}

// Returns whether the type has #[must_use] attribute
pub fn is_must_use_ty<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    match ty.kind() {
        ty::Adt(adt, _) => cx.tcx.has_attr(adt.did(), sym::must_use),
        ty::Foreign(did) => cx.tcx.has_attr(*did, sym::must_use),
        ty::Slice(ty) | ty::Array(ty, _) | ty::RawPtr(ty::TypeAndMut { ty, .. }) | ty::Ref(_, ty, _) => {
            // for the Array case we don't need to care for the len == 0 case
            // because we don't want to lint functions returning empty arrays
            is_must_use_ty(cx, *ty)
        },
        ty::Tuple(args) => args.iter().any(|ty| is_must_use_ty(cx, ty)),
        ty::Alias(ty::Opaque, ty::AliasTy { def_id, .. }) => {
            for (predicate, _) in cx.tcx.explicit_item_bounds(def_id).skip_binder() {
                if let ty::ClauseKind::Trait(trait_predicate) = predicate.kind().skip_binder() {
                    if cx.tcx.has_attr(trait_predicate.trait_ref.def_id, sym::must_use) {
                        return true;
                    }
                }
            }
            false
        },
        ty::Dynamic(binder, _, _) => {
            for predicate in *binder {
                if let ty::ExistentialPredicate::Trait(ref trait_ref) = predicate.skip_binder() {
                    if cx.tcx.has_attr(trait_ref.def_id, sym::must_use) {
                        return true;
                    }
                }
            }
            false
        },
        _ => false,
    }
}

// FIXME: Per https://doc.rust-lang.org/nightly/nightly-rustc/rustc_trait_selection/infer/at/struct.At.html#method.normalize
// this function can be removed once the `normalize` method does not panic when normalization does
// not succeed
/// Checks if `Ty` is normalizable. This function is useful
/// to avoid crashes on `layout_of`.
pub fn is_normalizable<'tcx>(cx: &LateContext<'tcx>, param_env: ty::ParamEnv<'tcx>, ty: Ty<'tcx>) -> bool {
    is_normalizable_helper(cx, param_env, ty, &mut FxHashMap::default())
}

fn is_normalizable_helper<'tcx>(
    cx: &LateContext<'tcx>,
    param_env: ty::ParamEnv<'tcx>,
    ty: Ty<'tcx>,
    cache: &mut FxHashMap<Ty<'tcx>, bool>,
) -> bool {
    if let Some(&cached_result) = cache.get(&ty) {
        return cached_result;
    }
    // prevent recursive loops, false-negative is better than endless loop leading to stack overflow
    cache.insert(ty, false);
    let infcx = cx.tcx.infer_ctxt().build();
    let cause = rustc_middle::traits::ObligationCause::dummy();
    let result = if infcx.at(&cause, param_env).query_normalize(ty).is_ok() {
        match ty.kind() {
            ty::Adt(def, args) => def.variants().iter().all(|variant| {
                variant
                    .fields
                    .iter()
                    .all(|field| is_normalizable_helper(cx, param_env, field.ty(cx.tcx, args), cache))
            }),
            _ => ty.walk().all(|generic_arg| match generic_arg.unpack() {
                GenericArgKind::Type(inner_ty) if inner_ty != ty => {
                    is_normalizable_helper(cx, param_env, inner_ty, cache)
                },
                _ => true, // if inner_ty == ty, we've already checked it
            }),
        }
    } else {
        false
    };
    cache.insert(ty, result);
    result
}

/// Returns `true` if the given type is a non aggregate primitive (a `bool` or `char`, any
/// integer or floating-point number type). For checking aggregation of primitive types (e.g.
/// tuples and slices of primitive type) see `is_recursively_primitive_type`
pub fn is_non_aggregate_primitive_type(ty: Ty<'_>) -> bool {
    matches!(ty.kind(), ty::Bool | ty::Char | ty::Int(_) | ty::Uint(_) | ty::Float(_))
}

/// Returns `true` if the given type is a primitive (a `bool` or `char`, any integer or
/// floating-point number type, a `str`, or an array, slice, or tuple of those types).
pub fn is_recursively_primitive_type(ty: Ty<'_>) -> bool {
    match *ty.kind() {
        ty::Bool | ty::Char | ty::Int(_) | ty::Uint(_) | ty::Float(_) | ty::Str => true,
        ty::Ref(_, inner, _) if inner.is_str() => true,
        ty::Array(inner_type, _) | ty::Slice(inner_type) => is_recursively_primitive_type(inner_type),
        ty::Tuple(inner_types) => inner_types.iter().all(is_recursively_primitive_type),
        _ => false,
    }
}

/// Checks if the type is a reference equals to a diagnostic item
pub fn is_type_ref_to_diagnostic_item(cx: &LateContext<'_>, ty: Ty<'_>, diag_item: Symbol) -> bool {
    match ty.kind() {
        ty::Ref(_, ref_ty, _) => match ref_ty.kind() {
            ty::Adt(adt, _) => cx.tcx.is_diagnostic_item(diag_item, adt.did()),
            _ => false,
        },
        _ => false,
    }
}

/// Checks if the type is equal to a diagnostic item. To check if a type implements a
/// trait marked with a diagnostic item use [`implements_trait`].
///
/// For a further exploitation what diagnostic items are see [diagnostic items] in
/// rustc-dev-guide.
///
/// ---
///
/// If you change the signature, remember to update the internal lint `MatchTypeOnDiagItem`
///
/// [Diagnostic Items]: https://rustc-dev-guide.rust-lang.org/diagnostics/diagnostic-items.html
pub fn is_type_diagnostic_item(cx: &LateContext<'_>, ty: Ty<'_>, diag_item: Symbol) -> bool {
    match ty.kind() {
        ty::Adt(adt, _) => cx.tcx.is_diagnostic_item(diag_item, adt.did()),
        _ => false,
    }
}

/// Checks if the type is equal to a lang item.
///
/// Returns `false` if the `LangItem` is not defined.
pub fn is_type_lang_item(cx: &LateContext<'_>, ty: Ty<'_>, lang_item: hir::LangItem) -> bool {
    match ty.kind() {
        ty::Adt(adt, _) => cx.tcx.lang_items().get(lang_item) == Some(adt.did()),
        _ => false,
    }
}

/// Gets the diagnostic name of the type, if it has one
pub fn type_diagnostic_name(cx: &LateContext<'_>, ty: Ty<'_>) -> Option<Symbol> {
    ty.ty_adt_def().and_then(|adt| cx.tcx.get_diagnostic_name(adt.did()))
}

/// Return `true` if the passed `typ` is `isize` or `usize`.
pub fn is_isize_or_usize(typ: Ty<'_>) -> bool {
    matches!(typ.kind(), ty::Int(IntTy::Isize) | ty::Uint(UintTy::Usize))
}

/// Checks if type is struct, enum or union type with the given def path.
///
/// If the type is a diagnostic item, use `is_type_diagnostic_item` instead.
/// If you change the signature, remember to update the internal lint `MatchTypeOnDiagItem`
pub fn match_type(cx: &LateContext<'_>, ty: Ty<'_>, path: &[&str]) -> bool {
    match ty.kind() {
        ty::Adt(adt, _) => match_def_path(cx, adt.did(), path),
        _ => false,
    }
}

/// Checks if the drop order for a type matters. Some std types implement drop solely to
/// deallocate memory. For these types, and composites containing them, changing the drop order
/// won't result in any observable side effects.
pub fn needs_ordered_drop<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    fn needs_ordered_drop_inner<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>, seen: &mut FxHashSet<Ty<'tcx>>) -> bool {
        if !seen.insert(ty) {
            return false;
        }
        if !ty.has_significant_drop(cx.tcx, cx.param_env) {
            false
        }
        // Check for std types which implement drop, but only for memory allocation.
        else if is_type_lang_item(cx, ty, LangItem::OwnedBox)
            || matches!(
                get_type_diagnostic_name(cx, ty),
                Some(sym::HashSet | sym::Rc | sym::Arc | sym::cstring_type | sym::RcWeak | sym::ArcWeak)
            )
        {
            // Check all of the generic arguments.
            if let ty::Adt(_, subs) = ty.kind() {
                subs.types().any(|ty| needs_ordered_drop_inner(cx, ty, seen))
            } else {
                true
            }
        } else if !cx
            .tcx
            .lang_items()
            .drop_trait()
            .map_or(false, |id| implements_trait(cx, ty, id, &[]))
        {
            // This type doesn't implement drop, so no side effects here.
            // Check if any component type has any.
            match ty.kind() {
                ty::Tuple(fields) => fields.iter().any(|ty| needs_ordered_drop_inner(cx, ty, seen)),
                ty::Array(ty, _) => needs_ordered_drop_inner(cx, *ty, seen),
                ty::Adt(adt, subs) => adt
                    .all_fields()
                    .map(|f| f.ty(cx.tcx, subs))
                    .any(|ty| needs_ordered_drop_inner(cx, ty, seen)),
                _ => true,
            }
        } else {
            true
        }
    }

    needs_ordered_drop_inner(cx, ty, &mut FxHashSet::default())
}

/// Peels off all references on the type. Returns the underlying type and the number of references
/// removed.
pub fn peel_mid_ty_refs(ty: Ty<'_>) -> (Ty<'_>, usize) {
    fn peel(ty: Ty<'_>, count: usize) -> (Ty<'_>, usize) {
        if let ty::Ref(_, ty, _) = ty.kind() {
            peel(*ty, count + 1)
        } else {
            (ty, count)
        }
    }
    peel(ty, 0)
}

/// Peels off all references on the type. Returns the underlying type, the number of references
/// removed, and whether the pointer is ultimately mutable or not.
pub fn peel_mid_ty_refs_is_mutable(ty: Ty<'_>) -> (Ty<'_>, usize, Mutability) {
    fn f(ty: Ty<'_>, count: usize, mutability: Mutability) -> (Ty<'_>, usize, Mutability) {
        match ty.kind() {
            ty::Ref(_, ty, Mutability::Mut) => f(*ty, count + 1, mutability),
            ty::Ref(_, ty, Mutability::Not) => f(*ty, count + 1, Mutability::Not),
            _ => (ty, count, mutability),
        }
    }
    f(ty, 0, Mutability::Mut)
}

/// Returns `true` if the given type is an `unsafe` function.
pub fn type_is_unsafe_function<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    match ty.kind() {
        ty::FnDef(..) | ty::FnPtr(_) => ty.fn_sig(cx.tcx).unsafety() == Unsafety::Unsafe,
        _ => false,
    }
}

/// Returns the base type for HIR references and pointers.
pub fn walk_ptrs_hir_ty<'tcx>(ty: &'tcx hir::Ty<'tcx>) -> &'tcx hir::Ty<'tcx> {
    match ty.kind {
        TyKind::Ptr(ref mut_ty) | TyKind::Ref(_, ref mut_ty) => walk_ptrs_hir_ty(mut_ty.ty),
        _ => ty,
    }
}

/// Returns the base type for references and raw pointers, and count reference
/// depth.
pub fn walk_ptrs_ty_depth(ty: Ty<'_>) -> (Ty<'_>, usize) {
    fn inner(ty: Ty<'_>, depth: usize) -> (Ty<'_>, usize) {
        match ty.kind() {
            ty::Ref(_, ty, _) => inner(*ty, depth + 1),
            _ => (ty, depth),
        }
    }
    inner(ty, 0)
}

/// Returns `true` if types `a` and `b` are same types having same `Const` generic args,
/// otherwise returns `false`
pub fn same_type_and_consts<'tcx>(a: Ty<'tcx>, b: Ty<'tcx>) -> bool {
    match (&a.kind(), &b.kind()) {
        (&ty::Adt(did_a, args_a), &ty::Adt(did_b, args_b)) => {
            if did_a != did_b {
                return false;
            }

            args_a
                .iter()
                .zip(args_b.iter())
                .all(|(arg_a, arg_b)| match (arg_a.unpack(), arg_b.unpack()) {
                    (GenericArgKind::Const(inner_a), GenericArgKind::Const(inner_b)) => inner_a == inner_b,
                    (GenericArgKind::Type(type_a), GenericArgKind::Type(type_b)) => {
                        same_type_and_consts(type_a, type_b)
                    },
                    _ => true,
                })
        },
        _ => a == b,
    }
}

/// Checks if a given type looks safe to be uninitialized.
pub fn is_uninit_value_valid_for_ty<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    cx.tcx
        .check_validity_requirement((ValidityRequirement::Uninit, cx.param_env.and(ty)))
        .unwrap_or_else(|_| is_uninit_value_valid_for_ty_fallback(cx, ty))
}

/// A fallback for polymorphic types, which are not supported by `check_validity_requirement`.
fn is_uninit_value_valid_for_ty_fallback<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    match *ty.kind() {
        // The array length may be polymorphic, let's try the inner type.
        ty::Array(component, _) => is_uninit_value_valid_for_ty(cx, component),
        // Peek through tuples and try their fallbacks.
        ty::Tuple(types) => types.iter().all(|ty| is_uninit_value_valid_for_ty(cx, ty)),
        // Unions are always fine right now.
        // This includes MaybeUninit, the main way people use uninitialized memory.
        // For ADTs, we could look at all fields just like for tuples, but that's potentially
        // exponential, so let's avoid doing that for now. Code doing that is sketchy enough to
        // just use an `#[allow()]`.
        ty::Adt(adt, _) => adt.is_union(),
        // For the rest, conservatively assume that they cannot be uninit.
        _ => false,
    }
}

/// Gets an iterator over all predicates which apply to the given item.
pub fn all_predicates_of(tcx: TyCtxt<'_>, id: DefId) -> impl Iterator<Item = &(ty::Clause<'_>, Span)> {
    let mut next_id = Some(id);
    iter::from_fn(move || {
        next_id.take().map(|id| {
            let preds = tcx.predicates_of(id);
            next_id = preds.parent;
            preds.predicates.iter()
        })
    })
    .flatten()
}

/// A signature for a function like type.
#[derive(Clone, Copy)]
pub enum ExprFnSig<'tcx> {
    Sig(Binder<'tcx, FnSig<'tcx>>, Option<DefId>),
    Closure(Option<&'tcx FnDecl<'tcx>>, Binder<'tcx, FnSig<'tcx>>),
    Trait(Binder<'tcx, Ty<'tcx>>, Option<Binder<'tcx, Ty<'tcx>>>, Option<DefId>),
}
impl<'tcx> ExprFnSig<'tcx> {
    /// Gets the argument type at the given offset. This will return `None` when the index is out of
    /// bounds only for variadic functions, otherwise this will panic.
    pub fn input(self, i: usize) -> Option<Binder<'tcx, Ty<'tcx>>> {
        match self {
            Self::Sig(sig, _) => {
                if sig.c_variadic() {
                    sig.inputs().map_bound(|inputs| inputs.get(i).copied()).transpose()
                } else {
                    Some(sig.input(i))
                }
            },
            Self::Closure(_, sig) => Some(sig.input(0).map_bound(|ty| ty.tuple_fields()[i])),
            Self::Trait(inputs, _, _) => Some(inputs.map_bound(|ty| ty.tuple_fields()[i])),
        }
    }

    /// Gets the argument type at the given offset. For closures this will also get the type as
    /// written. This will return `None` when the index is out of bounds only for variadic
    /// functions, otherwise this will panic.
    pub fn input_with_hir(self, i: usize) -> Option<(Option<&'tcx hir::Ty<'tcx>>, Binder<'tcx, Ty<'tcx>>)> {
        match self {
            Self::Sig(sig, _) => {
                if sig.c_variadic() {
                    sig.inputs()
                        .map_bound(|inputs| inputs.get(i).copied())
                        .transpose()
                        .map(|arg| (None, arg))
                } else {
                    Some((None, sig.input(i)))
                }
            },
            Self::Closure(decl, sig) => Some((
                decl.and_then(|decl| decl.inputs.get(i)),
                sig.input(0).map_bound(|ty| ty.tuple_fields()[i]),
            )),
            Self::Trait(inputs, _, _) => Some((None, inputs.map_bound(|ty| ty.tuple_fields()[i]))),
        }
    }

    /// Gets the result type, if one could be found. Note that the result type of a trait may not be
    /// specified.
    pub fn output(self) -> Option<Binder<'tcx, Ty<'tcx>>> {
        match self {
            Self::Sig(sig, _) | Self::Closure(_, sig) => Some(sig.output()),
            Self::Trait(_, output, _) => output,
        }
    }

    pub fn predicates_id(&self) -> Option<DefId> {
        if let ExprFnSig::Sig(_, id) | ExprFnSig::Trait(_, _, id) = *self {
            id
        } else {
            None
        }
    }
}

/// If the expression is function like, get the signature for it.
pub fn expr_sig<'tcx>(cx: &LateContext<'tcx>, expr: &Expr<'_>) -> Option<ExprFnSig<'tcx>> {
    if let Res::Def(DefKind::Fn | DefKind::Ctor(_, CtorKind::Fn) | DefKind::AssocFn, id) = path_res(cx, expr) {
        Some(ExprFnSig::Sig(cx.tcx.fn_sig(id).instantiate_identity(), Some(id)))
    } else {
        ty_sig(cx, cx.typeck_results().expr_ty_adjusted(expr).peel_refs())
    }
}

/// If the type is function like, get the signature for it.
pub fn ty_sig<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> Option<ExprFnSig<'tcx>> {
    if ty.is_box() {
        return ty_sig(cx, ty.boxed_ty());
    }
    match *ty.kind() {
        ty::Closure(id, subs) => {
            let decl = id
                .as_local()
                .and_then(|id| cx.tcx.hir().fn_decl_by_hir_id(cx.tcx.local_def_id_to_hir_id(id)));
            Some(ExprFnSig::Closure(decl, subs.as_closure().sig()))
        },
        ty::FnDef(id, subs) => Some(ExprFnSig::Sig(cx.tcx.fn_sig(id).instantiate(cx.tcx, subs), Some(id))),
        ty::Alias(ty::Opaque, ty::AliasTy { def_id, args, .. }) => sig_from_bounds(
            cx,
            ty,
            cx.tcx.item_bounds(def_id).iter_instantiated(cx.tcx, args),
            cx.tcx.opt_parent(def_id),
        ),
        ty::FnPtr(sig) => Some(ExprFnSig::Sig(sig, None)),
        ty::Dynamic(bounds, _, _) => {
            let lang_items = cx.tcx.lang_items();
            match bounds.principal() {
                Some(bound)
                    if Some(bound.def_id()) == lang_items.fn_trait()
                        || Some(bound.def_id()) == lang_items.fn_once_trait()
                        || Some(bound.def_id()) == lang_items.fn_mut_trait() =>
                {
                    let output = bounds
                        .projection_bounds()
                        .find(|p| lang_items.fn_once_output().map_or(false, |id| id == p.item_def_id()))
                        .map(|p| p.map_bound(|p| p.term.ty().unwrap()));
                    Some(ExprFnSig::Trait(bound.map_bound(|b| b.args.type_at(0)), output, None))
                },
                _ => None,
            }
        },
        ty::Alias(ty::Projection, proj) => match cx.tcx.try_normalize_erasing_regions(cx.param_env, ty) {
            Ok(normalized_ty) if normalized_ty != ty => ty_sig(cx, normalized_ty),
            _ => sig_for_projection(cx, proj).or_else(|| sig_from_bounds(cx, ty, cx.param_env.caller_bounds(), None)),
        },
        ty::Param(_) => sig_from_bounds(cx, ty, cx.param_env.caller_bounds(), None),
        _ => None,
    }
}

fn sig_from_bounds<'tcx>(
    cx: &LateContext<'tcx>,
    ty: Ty<'tcx>,
    predicates: impl IntoIterator<Item = ty::Clause<'tcx>>,
    predicates_id: Option<DefId>,
) -> Option<ExprFnSig<'tcx>> {
    let mut inputs = None;
    let mut output = None;
    let lang_items = cx.tcx.lang_items();

    for pred in predicates {
        match pred.kind().skip_binder() {
            ty::ClauseKind::Trait(p)
                if (lang_items.fn_trait() == Some(p.def_id())
                    || lang_items.fn_mut_trait() == Some(p.def_id())
                    || lang_items.fn_once_trait() == Some(p.def_id()))
                    && p.self_ty() == ty =>
            {
                let i = pred.kind().rebind(p.trait_ref.args.type_at(1));
                if inputs.map_or(false, |inputs| i != inputs) {
                    // Multiple different fn trait impls. Is this even allowed?
                    return None;
                }
                inputs = Some(i);
            },
            ty::ClauseKind::Projection(p)
                if Some(p.projection_ty.def_id) == lang_items.fn_once_output() && p.projection_ty.self_ty() == ty =>
            {
                if output.is_some() {
                    // Multiple different fn trait impls. Is this even allowed?
                    return None;
                }
                output = Some(pred.kind().rebind(p.term.ty().unwrap()));
            },
            _ => (),
        }
    }

    inputs.map(|ty| ExprFnSig::Trait(ty, output, predicates_id))
}

fn sig_for_projection<'tcx>(cx: &LateContext<'tcx>, ty: AliasTy<'tcx>) -> Option<ExprFnSig<'tcx>> {
    let mut inputs = None;
    let mut output = None;
    let lang_items = cx.tcx.lang_items();

    for (pred, _) in cx
        .tcx
        .explicit_item_bounds(ty.def_id)
        .iter_instantiated_copied(cx.tcx, ty.args)
    {
        match pred.kind().skip_binder() {
            ty::ClauseKind::Trait(p)
                if (lang_items.fn_trait() == Some(p.def_id())
                    || lang_items.fn_mut_trait() == Some(p.def_id())
                    || lang_items.fn_once_trait() == Some(p.def_id())) =>
            {
                let i = pred.kind().rebind(p.trait_ref.args.type_at(1));

                if inputs.map_or(false, |inputs| inputs != i) {
                    // Multiple different fn trait impls. Is this even allowed?
                    return None;
                }
                inputs = Some(i);
            },
            ty::ClauseKind::Projection(p) if Some(p.projection_ty.def_id) == lang_items.fn_once_output() => {
                if output.is_some() {
                    // Multiple different fn trait impls. Is this even allowed?
                    return None;
                }
                output = pred.kind().rebind(p.term.ty()).transpose();
            },
            _ => (),
        }
    }

    inputs.map(|ty| ExprFnSig::Trait(ty, output, None))
}

#[derive(Clone, Copy)]
pub enum EnumValue {
    Unsigned(u128),
    Signed(i128),
}
impl core::ops::Add<u32> for EnumValue {
    type Output = Self;
    fn add(self, n: u32) -> Self::Output {
        match self {
            Self::Unsigned(x) => Self::Unsigned(x + u128::from(n)),
            Self::Signed(x) => Self::Signed(x + i128::from(n)),
        }
    }
}

/// Attempts to read the given constant as though it were an enum value.
#[expect(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
pub fn read_explicit_enum_value(tcx: TyCtxt<'_>, id: DefId) -> Option<EnumValue> {
    if let Ok(ConstValue::Scalar(Scalar::Int(value))) = tcx.const_eval_poly(id) {
        match tcx.type_of(id).instantiate_identity().kind() {
            ty::Int(_) => Some(EnumValue::Signed(match value.size().bytes() {
                1 => i128::from(value.assert_bits(Size::from_bytes(1)) as u8 as i8),
                2 => i128::from(value.assert_bits(Size::from_bytes(2)) as u16 as i16),
                4 => i128::from(value.assert_bits(Size::from_bytes(4)) as u32 as i32),
                8 => i128::from(value.assert_bits(Size::from_bytes(8)) as u64 as i64),
                16 => value.assert_bits(Size::from_bytes(16)) as i128,
                _ => return None,
            })),
            ty::Uint(_) => Some(EnumValue::Unsigned(match value.size().bytes() {
                1 => value.assert_bits(Size::from_bytes(1)),
                2 => value.assert_bits(Size::from_bytes(2)),
                4 => value.assert_bits(Size::from_bytes(4)),
                8 => value.assert_bits(Size::from_bytes(8)),
                16 => value.assert_bits(Size::from_bytes(16)),
                _ => return None,
            })),
            _ => None,
        }
    } else {
        None
    }
}

/// Gets the value of the given variant.
pub fn get_discriminant_value(tcx: TyCtxt<'_>, adt: AdtDef<'_>, i: VariantIdx) -> EnumValue {
    let variant = &adt.variant(i);
    match variant.discr {
        VariantDiscr::Explicit(id) => read_explicit_enum_value(tcx, id).unwrap(),
        VariantDiscr::Relative(x) => match adt.variant((i.as_usize() - x as usize).into()).discr {
            VariantDiscr::Explicit(id) => read_explicit_enum_value(tcx, id).unwrap() + x,
            VariantDiscr::Relative(_) => EnumValue::Unsigned(x.into()),
        },
    }
}

/// Check if the given type is either `core::ffi::c_void`, `std::os::raw::c_void`, or one of the
/// platform specific `libc::<platform>::c_void` types in libc.
pub fn is_c_void(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
    if let ty::Adt(adt, _) = ty.kind()
        && let &[krate, .., name] = &*cx.get_def_path(adt.did())
        && let sym::libc | sym::core | sym::std = krate
        && name == rustc_span::sym::c_void
    {
        true
    } else {
        false
    }
}

pub fn for_each_top_level_late_bound_region<B>(
    ty: Ty<'_>,
    f: impl FnMut(BoundRegion) -> ControlFlow<B>,
) -> ControlFlow<B> {
    struct V<F> {
        index: u32,
        f: F,
    }
    impl<'tcx, B, F: FnMut(BoundRegion) -> ControlFlow<B>> TypeVisitor<TyCtxt<'tcx>> for V<F> {
        type BreakTy = B;
        fn visit_region(&mut self, r: Region<'tcx>) -> ControlFlow<Self::BreakTy> {
            if let RegionKind::ReBound(idx, bound) = r.kind()
                && idx.as_u32() == self.index
            {
                (self.f)(bound)
            } else {
                ControlFlow::Continue(())
            }
        }
        fn visit_binder<T: TypeVisitable<TyCtxt<'tcx>>>(&mut self, t: &Binder<'tcx, T>) -> ControlFlow<Self::BreakTy> {
            self.index += 1;
            let res = t.super_visit_with(self);
            self.index -= 1;
            res
        }
    }
    ty.visit_with(&mut V { index: 0, f })
}

pub struct AdtVariantInfo {
    pub ind: usize,
    pub size: u64,

    /// (ind, size)
    pub fields_size: Vec<(usize, u64)>,
}

impl AdtVariantInfo {
    /// Returns ADT variants ordered by size
    pub fn new<'tcx>(cx: &LateContext<'tcx>, adt: AdtDef<'tcx>, subst: &'tcx List<GenericArg<'tcx>>) -> Vec<Self> {
        let mut variants_size = adt
            .variants()
            .iter()
            .enumerate()
            .map(|(i, variant)| {
                let mut fields_size = variant
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(i, f)| (i, approx_ty_size(cx, f.ty(cx.tcx, subst))))
                    .collect::<Vec<_>>();
                fields_size.sort_by(|(_, a_size), (_, b_size)| (a_size.cmp(b_size)));

                Self {
                    ind: i,
                    size: fields_size.iter().map(|(_, size)| size).sum(),
                    fields_size,
                }
            })
            .collect::<Vec<_>>();
        variants_size.sort_by(|a, b| (b.size.cmp(&a.size)));
        variants_size
    }
}

/// Gets the struct or enum variant from the given `Res`
pub fn adt_and_variant_of_res<'tcx>(cx: &LateContext<'tcx>, res: Res) -> Option<(AdtDef<'tcx>, &'tcx VariantDef)> {
    match res {
        Res::Def(DefKind::Struct, id) => {
            let adt = cx.tcx.adt_def(id);
            Some((adt, adt.non_enum_variant()))
        },
        Res::Def(DefKind::Variant, id) => {
            let adt = cx.tcx.adt_def(cx.tcx.parent(id));
            Some((adt, adt.variant_with_id(id)))
        },
        Res::Def(DefKind::Ctor(CtorOf::Struct, _), id) => {
            let adt = cx.tcx.adt_def(cx.tcx.parent(id));
            Some((adt, adt.non_enum_variant()))
        },
        Res::Def(DefKind::Ctor(CtorOf::Variant, _), id) => {
            let var_id = cx.tcx.parent(id);
            let adt = cx.tcx.adt_def(cx.tcx.parent(var_id));
            Some((adt, adt.variant_with_id(var_id)))
        },
        Res::SelfCtor(id) => {
            let adt = cx.tcx.type_of(id).instantiate_identity().ty_adt_def().unwrap();
            Some((adt, adt.non_enum_variant()))
        },
        _ => None,
    }
}

/// Checks if the type is a type parameter implementing `FnOnce`, but not `FnMut`.
pub fn ty_is_fn_once_param<'tcx>(tcx: TyCtxt<'_>, ty: Ty<'tcx>, predicates: &'tcx [ty::Clause<'_>]) -> bool {
    let ty::Param(ty) = *ty.kind() else {
        return false;
    };
    let lang = tcx.lang_items();
    let (Some(fn_once_id), Some(fn_mut_id), Some(fn_id)) = (lang.fn_once_trait(), lang.fn_mut_trait(), lang.fn_trait())
    else {
        return false;
    };
    predicates
        .iter()
        .try_fold(false, |found, p| {
            if let ty::ClauseKind::Trait(p) = p.kind().skip_binder()
                && let ty::Param(self_ty) = p.trait_ref.self_ty().kind()
                && ty.index == self_ty.index
            {
                // This should use `super_traits_of`, but that's a private function.
                if p.trait_ref.def_id == fn_once_id {
                    return Some(true);
                } else if p.trait_ref.def_id == fn_mut_id || p.trait_ref.def_id == fn_id {
                    return None;
                }
            }
            Some(found)
        })
        .unwrap_or(false)
}

/// Comes up with an "at least" guesstimate for the type's size, not taking into
/// account the layout of type parameters.
pub fn approx_ty_size<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> u64 {
    use rustc_middle::ty::layout::LayoutOf;
    if !is_normalizable(cx, cx.param_env, ty) {
        return 0;
    }
    match (cx.layout_of(ty).map(|layout| layout.size.bytes()), ty.kind()) {
        (Ok(size), _) => size,
        (Err(_), ty::Tuple(list)) => list.iter().map(|t| approx_ty_size(cx, t)).sum(),
        (Err(_), ty::Array(t, n)) => {
            n.try_eval_target_usize(cx.tcx, cx.param_env).unwrap_or_default() * approx_ty_size(cx, *t)
        },
        (Err(_), ty::Adt(def, subst)) if def.is_struct() => def
            .variants()
            .iter()
            .map(|v| {
                v.fields
                    .iter()
                    .map(|field| approx_ty_size(cx, field.ty(cx.tcx, subst)))
                    .sum::<u64>()
            })
            .sum(),
        (Err(_), ty::Adt(def, subst)) if def.is_enum() => def
            .variants()
            .iter()
            .map(|v| {
                v.fields
                    .iter()
                    .map(|field| approx_ty_size(cx, field.ty(cx.tcx, subst)))
                    .sum::<u64>()
            })
            .max()
            .unwrap_or_default(),
        (Err(_), ty::Adt(def, subst)) if def.is_union() => def
            .variants()
            .iter()
            .map(|v| {
                v.fields
                    .iter()
                    .map(|field| approx_ty_size(cx, field.ty(cx.tcx, subst)))
                    .max()
                    .unwrap_or_default()
            })
            .max()
            .unwrap_or_default(),
        (Err(_), _) => 0,
    }
}

/// Asserts that the given arguments match the generic parameters of the given item.
#[allow(dead_code)]
fn assert_generic_args_match<'tcx>(tcx: TyCtxt<'tcx>, did: DefId, args: &[GenericArg<'tcx>]) {
    let g = tcx.generics_of(did);
    let parent = g.parent.map(|did| tcx.generics_of(did));
    let count = g.parent_count + g.params.len();
    let params = parent
        .map_or([].as_slice(), |p| p.params.as_slice())
        .iter()
        .chain(&g.params)
        .map(|x| &x.kind);

    assert!(
        count == args.len(),
        "wrong number of arguments for `{did:?}`: expected `{count}`, found {}\n\
            note: the expected arguments are: `[{}]`\n\
            the given arguments are: `{args:#?}`",
        args.len(),
        params.clone().map(GenericParamDefKind::descr).format(", "),
    );

    if let Some((idx, (param, arg))) =
        params
            .clone()
            .zip(args.iter().map(|&x| x.unpack()))
            .enumerate()
            .find(|(_, (param, arg))| match (param, arg) {
                (GenericParamDefKind::Lifetime, GenericArgKind::Lifetime(_))
                | (GenericParamDefKind::Type { .. }, GenericArgKind::Type(_))
                | (GenericParamDefKind::Const { .. }, GenericArgKind::Const(_)) => false,
                (
                    GenericParamDefKind::Lifetime
                    | GenericParamDefKind::Type { .. }
                    | GenericParamDefKind::Const { .. },
                    _,
                ) => true,
            })
    {
        panic!(
            "incorrect argument for `{did:?}` at index `{idx}`: expected a {}, found `{arg:?}`\n\
                note: the expected arguments are `[{}]`\n\
                the given arguments are `{args:#?}`",
            param.descr(),
            params.clone().map(GenericParamDefKind::descr).format(", "),
        );
    }
}

/// Returns whether `ty` is never-like; i.e., `!` (never) or an enum with zero variants.
pub fn is_never_like(ty: Ty<'_>) -> bool {
    ty.is_never() || (ty.is_enum() && ty.ty_adt_def().is_some_and(|def| def.variants().is_empty()))
}

/// Makes the projection type for the named associated type in the given impl or trait impl.
///
/// This function is for associated types which are "known" to exist, and as such, will only return
/// `None` when debug assertions are disabled in order to prevent ICE's. With debug assertions
/// enabled this will check that the named associated type exists, the correct number of
/// arguments are given, and that the correct kinds of arguments are given (lifetime,
/// constant or type). This will not check if type normalization would succeed.
pub fn make_projection<'tcx>(
    tcx: TyCtxt<'tcx>,
    container_id: DefId,
    assoc_ty: Symbol,
    args: impl IntoIterator<Item = impl Into<GenericArg<'tcx>>>,
) -> Option<AliasTy<'tcx>> {
    fn helper<'tcx>(
        tcx: TyCtxt<'tcx>,
        container_id: DefId,
        assoc_ty: Symbol,
        args: GenericArgsRef<'tcx>,
    ) -> Option<AliasTy<'tcx>> {
        let Some(assoc_item) = tcx.associated_items(container_id).find_by_name_and_kind(
            tcx,
            Ident::with_dummy_span(assoc_ty),
            AssocKind::Type,
            container_id,
        ) else {
            debug_assert!(false, "type `{assoc_ty}` not found in `{container_id:?}`");
            return None;
        };
        #[cfg(debug_assertions)]
        assert_generic_args_match(tcx, assoc_item.def_id, args);

        Some(ty::AliasTy::new(tcx, assoc_item.def_id, args))
    }
    helper(
        tcx,
        container_id,
        assoc_ty,
        tcx.mk_args_from_iter(args.into_iter().map(Into::into)),
    )
}

/// Normalizes the named associated type in the given impl or trait impl.
///
/// This function is for associated types which are "known" to be valid with the given
/// arguments, and as such, will only return `None` when debug assertions are disabled in order
/// to prevent ICE's. With debug assertions enabled this will check that type normalization
/// succeeds as well as everything checked by `make_projection`.
pub fn make_normalized_projection<'tcx>(
    tcx: TyCtxt<'tcx>,
    param_env: ParamEnv<'tcx>,
    container_id: DefId,
    assoc_ty: Symbol,
    args: impl IntoIterator<Item = impl Into<GenericArg<'tcx>>>,
) -> Option<Ty<'tcx>> {
    fn helper<'tcx>(tcx: TyCtxt<'tcx>, param_env: ParamEnv<'tcx>, ty: AliasTy<'tcx>) -> Option<Ty<'tcx>> {
        #[cfg(debug_assertions)]
        if let Some((i, arg)) = ty
            .args
            .iter()
            .enumerate()
            .find(|(_, arg)| arg.has_escaping_bound_vars())
        {
            debug_assert!(
                false,
                "args contain late-bound region at index `{i}` which can't be normalized.\n\
                    use `TyCtxt::instantiate_bound_regions_with_erased`\n\
                    note: arg is `{arg:#?}`",
            );
            return None;
        }
        match tcx.try_normalize_erasing_regions(param_env, Ty::new_projection(tcx, ty.def_id, ty.args)) {
            Ok(ty) => Some(ty),
            Err(e) => {
                debug_assert!(false, "failed to normalize type `{ty}`: {e:#?}");
                None
            },
        }
    }
    helper(tcx, param_env, make_projection(tcx, container_id, assoc_ty, args)?)
}

/// Check if given type has inner mutability such as [`std::cell::Cell`] or [`std::cell::RefCell`]
/// etc.
pub fn is_interior_mut_ty<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    match *ty.kind() {
        ty::Ref(_, inner_ty, mutbl) => mutbl == Mutability::Mut || is_interior_mut_ty(cx, inner_ty),
        ty::Slice(inner_ty) => is_interior_mut_ty(cx, inner_ty),
        ty::Array(inner_ty, size) => {
            size.try_eval_target_usize(cx.tcx, cx.param_env)
                .map_or(true, |u| u != 0)
                && is_interior_mut_ty(cx, inner_ty)
        },
        ty::Tuple(fields) => fields.iter().any(|ty| is_interior_mut_ty(cx, ty)),
        ty::Adt(def, args) => {
            // Special case for collections in `std` who's impl of `Hash` or `Ord` delegates to
            // that of their type parameters.  Note: we don't include `HashSet` and `HashMap`
            // because they have no impl for `Hash` or `Ord`.
            let def_id = def.did();
            let is_std_collection = [
                sym::Option,
                sym::Result,
                sym::LinkedList,
                sym::Vec,
                sym::VecDeque,
                sym::BTreeMap,
                sym::BTreeSet,
                sym::Rc,
                sym::Arc,
            ]
            .iter()
            .any(|diag_item| cx.tcx.is_diagnostic_item(*diag_item, def_id));
            let is_box = Some(def_id) == cx.tcx.lang_items().owned_box();
            if is_std_collection || is_box {
                // The type is mutable if any of its type parameters are
                args.types().any(|ty| is_interior_mut_ty(cx, ty))
            } else {
                !ty.has_escaping_bound_vars()
                    && cx.tcx.layout_of(cx.param_env.and(ty)).is_ok()
                    && !ty.is_freeze(cx.tcx, cx.param_env)
            }
        },
        _ => false,
    }
}

pub fn make_normalized_projection_with_regions<'tcx>(
    tcx: TyCtxt<'tcx>,
    param_env: ParamEnv<'tcx>,
    container_id: DefId,
    assoc_ty: Symbol,
    args: impl IntoIterator<Item = impl Into<GenericArg<'tcx>>>,
) -> Option<Ty<'tcx>> {
    fn helper<'tcx>(tcx: TyCtxt<'tcx>, param_env: ParamEnv<'tcx>, ty: AliasTy<'tcx>) -> Option<Ty<'tcx>> {
        #[cfg(debug_assertions)]
        if let Some((i, arg)) = ty
            .args
            .iter()
            .enumerate()
            .find(|(_, arg)| arg.has_escaping_bound_vars())
        {
            debug_assert!(
                false,
                "args contain late-bound region at index `{i}` which can't be normalized.\n\
                    use `TyCtxt::instantiate_bound_regions_with_erased`\n\
                    note: arg is `{arg:#?}`",
            );
            return None;
        }
        let cause = rustc_middle::traits::ObligationCause::dummy();
        match tcx
            .infer_ctxt()
            .build()
            .at(&cause, param_env)
            .query_normalize(Ty::new_projection(tcx, ty.def_id, ty.args))
        {
            Ok(ty) => Some(ty.value),
            Err(e) => {
                debug_assert!(false, "failed to normalize type `{ty}`: {e:#?}");
                None
            },
        }
    }
    helper(tcx, param_env, make_projection(tcx, container_id, assoc_ty, args)?)
}

pub fn normalize_with_regions<'tcx>(tcx: TyCtxt<'tcx>, param_env: ParamEnv<'tcx>, ty: Ty<'tcx>) -> Ty<'tcx> {
    let cause = rustc_middle::traits::ObligationCause::dummy();
    match tcx.infer_ctxt().build().at(&cause, param_env).query_normalize(ty) {
        Ok(ty) => ty.value,
        Err(_) => ty,
    }
}

/// Checks if the type is `core::mem::ManuallyDrop<_>`
pub fn is_manually_drop(ty: Ty<'_>) -> bool {
    ty.ty_adt_def().map_or(false, AdtDef::is_manually_drop)
}
