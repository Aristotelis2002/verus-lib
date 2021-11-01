//! VIR-AST -> VIR-AST transformation to simplify away some complicated features

use crate::ast::{
    BinaryOp, Constant, Expr, ExprX, Function, FunctionX, Ident, Krate, KrateX, Mode, Path,
    Pattern, PatternX, SpannedTyped, Stmt, StmtX, Typ, TypX, UnaryOp, UnaryOpr, VirErr,
};
use crate::ast_util::err_str;
use crate::context::GlobalCtx;
use crate::def::Spanned;
use crate::util::vec_map_result;
use std::sync::Arc;

pub(crate) struct State {
    // Counter to generate temporary variables
    next_var: u64,
}

impl State {
    pub fn new() -> Self {
        State { next_var: 0 }
    }

    fn next_temp(&mut self) -> Ident {
        self.next_var += 1;
        crate::def::prefix_simplify_temp_var(self.next_var)
    }
}

fn is_small_expr(expr: &Expr) -> bool {
    match &expr.x {
        ExprX::Const(_) => true,
        ExprX::Var(_) => true,
        ExprX::Unary(UnaryOp::Not | UnaryOp::Clip(_), e) => is_small_expr(e),
        ExprX::UnaryOpr(UnaryOpr::Box(_) | UnaryOpr::Unbox(_), e) => is_small_expr(e),
        _ => false,
    }
}

fn small_or_temp(state: &mut State, expr: &Expr) -> (Option<Stmt>, Expr) {
    if is_small_expr(&expr) {
        (None, expr.clone())
    } else {
        // put expr into a temp variable to avoid duplicating it
        let temp = state.next_temp();
        let name = temp.clone();
        let patternx = PatternX::Var { name, mutable: false };
        let pattern = SpannedTyped::new(&expr.span, &expr.typ, patternx);
        let decl = StmtX::Decl { pattern, mode: Mode::Exec, init: Some(expr.clone()) };
        let temp_decl = Some(Spanned::new(expr.span.clone(), decl));
        (temp_decl, SpannedTyped::new(&expr.span, &expr.typ, ExprX::Var(temp)))
    }
}

fn datatype_field_typ(ctx: &GlobalCtx, path: &Path, variant: &Ident, field: &Ident) -> Typ {
    let fields =
        &ctx.datatypes[path].iter().find(|v| v.name == *variant).expect("couldn't find variant").a;
    let (typ, _) = &fields.iter().find(|f| f.name == *field).expect("couldn't find field").a;
    typ.clone()
}

// Compute:
// - expression that tests whether exp matches pattern
// - bindings of pattern variables to fields of exp
fn pattern_to_exprs(
    ctx: &GlobalCtx,
    expr: &Expr,
    pattern: &Pattern,
    decls: &mut Vec<Stmt>,
) -> Result<Expr, VirErr> {
    let t_bool = Arc::new(TypX::Bool);
    match &pattern.x {
        PatternX::Wildcard => {
            Ok(SpannedTyped::new(&pattern.span, &t_bool, ExprX::Const(Constant::Bool(true))))
        }
        PatternX::Var { name: x, mutable } => {
            let patternx = PatternX::Var { name: x.clone(), mutable: *mutable };
            let pattern = SpannedTyped::new(&expr.span, &expr.typ, patternx);
            let decl = StmtX::Decl { pattern, mode: Mode::Exec, init: Some(expr.clone()) };
            decls.push(Spanned::new(expr.span.clone(), decl));
            Ok(SpannedTyped::new(&expr.span, &t_bool, ExprX::Const(Constant::Bool(true))))
        }
        PatternX::Constructor(path, variant, patterns) => {
            let is_variant_opr =
                UnaryOpr::IsVariant { datatype: path.clone(), variant: variant.clone() };
            let test_variant = ExprX::UnaryOpr(is_variant_opr, expr.clone());
            let mut test = SpannedTyped::new(&pattern.span, &t_bool, test_variant);
            for binder in patterns.iter() {
                let field_op = UnaryOpr::Field {
                    datatype: path.clone(),
                    variant: variant.clone(),
                    field: binder.name.clone(),
                };
                let field = ExprX::UnaryOpr(field_op, expr.clone());
                let field_typ = datatype_field_typ(ctx, path, variant, &binder.name);
                let field_exp = SpannedTyped::new(&pattern.span, &field_typ, field);
                let field_exp = match (&*field_typ, &*binder.a.typ) {
                    (TypX::TypParam(_), TypX::TypParam(_)) => field_exp,
                    (TypX::TypParam(_), TypX::Boxed(_)) => field_exp,
                    (TypX::TypParam(_), _) => {
                        let op = UnaryOpr::Unbox(binder.a.typ.clone());
                        let unbox = ExprX::UnaryOpr(op, field_exp);
                        SpannedTyped::new(&pattern.span, &binder.a.typ, unbox)
                    }
                    _ => field_exp,
                };
                let pattern_test = pattern_to_exprs(ctx, &field_exp, &binder.a, decls)?;
                let and = ExprX::Binary(BinaryOp::And, test, pattern_test);
                test = SpannedTyped::new(&pattern.span, &t_bool, and);
            }
            Ok(test)
        }
    }
}

fn simplify_one_expr(ctx: &GlobalCtx, state: &mut State, expr: &Expr) -> Result<Expr, VirErr> {
    match &expr.x {
        ExprX::Match(expr0, arms1) => {
            let (temp_decl, expr0) = small_or_temp(state, &expr0);
            // Translate into If expression
            let t_bool = Arc::new(TypX::Bool);
            let mut if_expr: Option<Expr> = None;
            for arm in arms1.iter().rev() {
                let mut decls: Vec<Stmt> = Vec::new();
                let test_pattern = pattern_to_exprs(ctx, &expr0, &arm.x.pattern, &mut decls)?;
                let test = match &arm.x.guard.x {
                    ExprX::Const(Constant::Bool(true)) => test_pattern,
                    _ => {
                        let guard = arm.x.guard.clone();
                        let test_exp = ExprX::Binary(BinaryOp::And, test_pattern, guard);
                        let test = SpannedTyped::new(&arm.x.pattern.span, &t_bool, test_exp);
                        let block = ExprX::Block(Arc::new(decls.clone()), Some(test));
                        SpannedTyped::new(&arm.x.pattern.span, &t_bool, block)
                    }
                };
                let block = ExprX::Block(Arc::new(decls), Some(arm.x.body.clone()));
                let body = SpannedTyped::new(&arm.x.pattern.span, &t_bool, block);
                if let Some(prev) = if_expr {
                    // if pattern && guard then body else prev
                    let ifx = ExprX::If(test.clone(), body, Some(prev));
                    if_expr = Some(SpannedTyped::new(&test.span, &expr.typ.clone(), ifx));
                } else {
                    // last arm is unconditional
                    if_expr = Some(body);
                }
            }
            if let Some(if_expr) = if_expr {
                let if_expr = if let Some(decl) = temp_decl {
                    let block = ExprX::Block(Arc::new(vec![decl]), Some(if_expr));
                    SpannedTyped::new(&expr.span, &expr.typ, block)
                } else {
                    if_expr
                };
                Ok(if_expr)
            } else {
                err_str(&expr.span, "not yet implemented: zero-arm match expressions")
            }
        }
        _ => Ok(expr.clone()),
    }
}

fn simplify_one_stmt(ctx: &GlobalCtx, state: &mut State, stmt: &Stmt) -> Result<Vec<Stmt>, VirErr> {
    match &stmt.x {
        StmtX::Decl { pattern, mode: _, init: None } => match &pattern.x {
            PatternX::Var { .. } => Ok(vec![stmt.clone()]),
            _ => err_str(&stmt.span, "let-pattern declaration must have an initializer"),
        },
        StmtX::Decl { pattern, mode: _, init: Some(init) } => {
            let mut decls: Vec<Stmt> = Vec::new();
            let (temp_decl, init) = small_or_temp(state, init);
            if let Some(temp_decl) = temp_decl {
                decls.push(temp_decl);
            }
            let _ = pattern_to_exprs(ctx, &init, &pattern, &mut decls)?;
            Ok(decls)
        }
        _ => Ok(vec![stmt.clone()]),
    }
}

fn simplify_expr(ctx: &GlobalCtx, state: &mut State, expr: &Expr) -> Result<Expr, VirErr> {
    crate::ast_visitor::map_expr_visitor_env(
        expr,
        state,
        &|state, expr| simplify_one_expr(ctx, state, expr),
        &|state, stmt| simplify_one_stmt(ctx, state, stmt),
    )
}

pub fn simplify_function(ctx: &GlobalCtx, function: &Function) -> Result<Function, VirErr> {
    let functionx = function.x.clone();
    let mut state = State::new();
    let body =
        functionx.body.as_ref().map(|expr| simplify_expr(ctx, &mut state, expr)).transpose()?;
    Ok(Spanned::new(function.span.clone(), FunctionX { body, ..functionx }))
}

pub fn simplify_krate(ctx: &GlobalCtx, krate: &Krate) -> Result<Krate, VirErr> {
    let kratex = (**krate).clone();
    let functions = vec_map_result(&kratex.functions, |f| simplify_function(ctx, f))?;
    Ok(Arc::new(KrateX { functions, ..kratex }))
}