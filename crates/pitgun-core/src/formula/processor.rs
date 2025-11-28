use std::collections::{HashMap, HashSet};

use crate::{Event, EventBatch, Processor};

use super::{EvalContext, Expr, eval_expr};

pub struct FormulaProcessor {
    output: String,
    expr: Expr,
    deps: Vec<String>,
}

impl FormulaProcessor {
    pub fn new(output: String, expr: Expr) -> Self {
        let mut deps = HashSet::new();
        collect_vars(&expr, &mut deps);
        Self {
            output,
            expr,
            deps: deps.into_iter().collect(),
        }
    }

    fn context_from_batch(&self, batch: &EventBatch) -> (HashMap<String, f64>, Option<u64>) {
        let mut values = HashMap::new();
        let mut ts_max: Option<u64> = None;
        for event in &batch.events {
            if self.deps.is_empty() || self.deps.contains(&event.channel) {
                values.insert(event.channel.clone(), event.value);
            }
            ts_max = Some(ts_max.map_or(event.ts_ns, |cur| cur.max(event.ts_ns)));
        }
        (values, ts_max)
    }
}

impl Processor for FormulaProcessor {
    fn process(&mut self, batch: &mut EventBatch) {
        if batch.events.is_empty() {
            return;
        }

        let (values, ts_max) = self.context_from_batch(batch);
        if !self.deps.is_empty() && !self.deps.iter().all(|d| values.contains_key(d)) {
            // Skip if required inputs are missing in this batch
            return;
        }

        let ctx = EvalContext { values: &values };
        let value = eval_expr(&self.expr, &ctx);
        let ts_ns = ts_max.unwrap_or(0);

        batch.events.push(Event {
            channel: self.output.clone(),
            ts_ns,
            value,
        });
    }
}

fn collect_vars(expr: &Expr, out: &mut HashSet<String>) {
    match expr {
        Expr::Literal(_) => {}
        Expr::Var(name) => {
            out.insert(name.clone());
        }
        Expr::UnaryOp { expr, .. } => collect_vars(expr, out),
        Expr::BinaryOp { left, right, .. } => {
            collect_vars(left, out);
            collect_vars(right, out);
        }
        Expr::FuncCall { args, .. } => {
            for arg in args {
                collect_vars(arg, out);
            }
        }
    }
}
