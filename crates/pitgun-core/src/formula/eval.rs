use std::collections::HashMap;

use crate::formula::{BinaryOp, Expr, UnaryOp};

pub struct EvalContext<'a> {
    pub values: &'a HashMap<String, f64>,
}

pub fn eval_expr(expr: &Expr, ctx: &EvalContext<'_>) -> f64 {
    match expr {
        Expr::Literal(v) => *v,
        Expr::Var(name) => ctx.values.get(name).copied().unwrap_or(f64::NAN),
        Expr::UnaryOp { op, expr } => match op {
            UnaryOp::Neg => -eval_expr(expr, ctx),
        },
        Expr::BinaryOp { op, left, right } => {
            let l = eval_expr(left, ctx);
            let r = eval_expr(right, ctx);
            match op {
                BinaryOp::Add => l + r,
                BinaryOp::Sub => l - r,
                BinaryOp::Mul => l * r,
                BinaryOp::Div => l / r,
            }
        }
        Expr::FuncCall { name, args } => {
            let evaluated: Vec<f64> = args.iter().map(|e| eval_expr(e, ctx)).collect();
            eval_func(name, &evaluated)
        }
    }
}

fn eval_func(name: &str, args: &[f64]) -> f64 {
    match (name, args) {
        ("sin", [x]) => x.sin(),
        ("cos", [x]) => x.cos(),
        ("tan", [x]) => x.tan(),
        ("sqrt", [x]) => x.sqrt(),
        ("exp", [x]) => x.exp(),
        ("ln", [x]) => x.ln(),
        ("log10", [x]) => x.log10(),
        ("abs", [x]) => x.abs(),
        ("pow", [x, y]) => x.powf(*y),
        _ => {
            eprintln!(
                "pitgun-core: unsupported function '{}' with {} args",
                name,
                args.len()
            );
            f64::NAN
        }
    }
}
