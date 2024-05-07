use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::is_from_proc_macro;
use clippy_utils::macros::macro_backtrace;
use clippy_utils::source::snippet;
use rustc_hir::{ArrayLen, Expr, ExprKind, Item, ItemKind, Node};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::layout::LayoutOf;
use rustc_middle::ty::{self, ConstKind};
use rustc_session::impl_lint_pass;
use rustc_span::{sym, Span};

declare_clippy_lint! {
    /// ### What it does
    /// Checks for local arrays that may be too large.
    ///
    /// ### Why is this bad?
    /// Large local arrays may cause stack overflow.
    ///
    /// ### Example
    /// ```rust,ignore
    /// let a = [0u32; 1_000_000];
    /// ```
    #[clippy::version = "1.41.0"]
    pub LARGE_STACK_ARRAYS,
    pedantic,
    "allocating large arrays on stack may cause stack overflow"
}

pub struct LargeStackArrays {
    maximum_allowed_size: u128,
    prev_vec_macro_callsite: Option<Span>,
}

impl LargeStackArrays {
    #[must_use]
    pub fn new(maximum_allowed_size: u128) -> Self {
        Self {
            maximum_allowed_size,
            prev_vec_macro_callsite: None,
        }
    }

    /// Check if the given span of an expr is already in a `vec!` call.
    fn is_from_vec_macro(&mut self, cx: &LateContext<'_>, span: Span) -> bool {
        // First, we check if this is span is within the last encountered `vec!` macro's root callsite.
        self.prev_vec_macro_callsite
            .is_some_and(|vec_mac| vec_mac.contains(span))
            || {
                // Then, we try backtracking the macro expansions, to see if there's a `vec!` macro,
                // and update the `prev_vec_macro_callsite`.
                let res = macro_backtrace(span).any(|mac| cx.tcx.is_diagnostic_item(sym::vec_macro, mac.def_id));
                if res {
                    self.prev_vec_macro_callsite = Some(span.source_callsite());
                }
                res
            }
    }
}

impl_lint_pass!(LargeStackArrays => [LARGE_STACK_ARRAYS]);

impl<'tcx> LateLintPass<'tcx> for LargeStackArrays {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &Expr<'tcx>) {
        if let ExprKind::Repeat(_, _) | ExprKind::Array(_) = expr.kind
            && !self.is_from_vec_macro(cx, expr.span)
            && let ty::Array(element_type, cst) = cx.typeck_results().expr_ty(expr).kind()
            && let ConstKind::Value(ty::ValTree::Leaf(element_count)) = cst.kind()
            && let Ok(element_count) = element_count.try_to_target_usize(cx.tcx)
            && let Ok(element_size) = cx.layout_of(*element_type).map(|l| l.size.bytes())
            && !cx.tcx.hir().parent_iter(expr.hir_id).any(|(_, node)| {
                matches!(
                    node,
                    Node::Item(Item {
                        kind: ItemKind::Static(..),
                        ..
                    })
                )
            })
            && self.maximum_allowed_size < u128::from(element_count) * u128::from(element_size)
        {
            span_lint_and_then(
                cx,
                LARGE_STACK_ARRAYS,
                expr.span,
                format!(
                    "allocating a local array larger than {} bytes",
                    self.maximum_allowed_size
                ),
                |diag| {
                    if !might_be_expanded(cx, expr) {
                        diag.help(format!(
                            "consider allocating on the heap with `vec!{}.into_boxed_slice()`",
                            snippet(cx, expr.span, "[...]")
                        ));
                    }
                },
            );
        }
    }
}

/// Only giving help messages if the expr does not contains macro expanded codes.
fn might_be_expanded<'tcx>(cx: &LateContext<'tcx>, expr: &Expr<'tcx>) -> bool {
    /// Check if the span of `ArrayLen` of a repeat expression is within the expr's span,
    /// if not, meaning this repeat expr is definitely from some proc-macro.
    ///
    /// This is a fail-safe to a case where even the `is_from_proc_macro` is unable to determain the
    /// correct result.
    fn repeat_expr_might_be_expanded<'tcx>(cx: &LateContext<'tcx>, expr: &Expr<'tcx>) -> bool {
        let ExprKind::Repeat(_, ArrayLen::Body(anon_const)) = expr.kind else {
            return false;
        };
        let len_span = cx.tcx.def_span(anon_const.def_id);
        !expr.span.contains(len_span)
    }

    expr.span.from_expansion() || is_from_proc_macro(cx, expr) || repeat_expr_might_be_expanded(cx, expr)
}
