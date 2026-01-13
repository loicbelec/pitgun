use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value as JsonValue;

#[derive(Clone, Debug)]
pub(crate) enum Expr {
    Number(f64),
    String(String),
    Var(VarPath),
    Unary { op: UnOp, expr: Box<Expr> },
    Binary { op: BinOp, left: Box<Expr>, right: Box<Expr> },
    IfThen { cond: Box<Expr>, then_expr: Box<Expr> },
    Call { name: String, args: Vec<Expr> },
}

#[derive(Clone, Debug)]
pub(crate) enum VarPath {
    Parameter(Vec<String>),
    Ident(String),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum BinOp {
    Add,
    Sub,
    Gt,
    Ge,
    Lt,
    Le,
    Eq,
    Ne,
    And,
    Or,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum UnOp {
    Not,
    Neg,
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Ident(String),
    Number(f64),
    Str(String),
    LParen,
    RParen,
    Dot,
    Comma,
    Plus,
    Minus,
    Gt,
    Ge,
    Lt,
    Le,
    Eq,
    Ne,
    And,
    Or,
    Not,
    If,
    Then,
}

pub(crate) fn parse_expression(input: &str) -> Result<Expr, String> {
    let tokens = tokenize(input)?;
    let mut parser = Parser::new(tokens);
    let expr = parser.parse_expr()?;
    if parser.peek().is_some() {
        return Err("unexpected trailing tokens".to_string());
    }
    Ok(expr)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    fn consume(&mut self, expected: Token) -> bool {
        if self.peek() == Some(&expected) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_if_then()
    }

    fn parse_if_then(&mut self) -> Result<Expr, String> {
        if self.consume(Token::If) {
            let cond = self.parse_or()?;
            if !self.consume(Token::Then) {
                return Err("expected 'then'".to_string());
            }
            let then_expr = self.parse_or()?;
            Ok(Expr::IfThen {
                cond: Box::new(cond),
                then_expr: Box::new(then_expr),
            })
        } else {
            self.parse_or()
        }
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_and()?;
        while self.consume(Token::Or) {
            let right = self.parse_and()?;
            expr = Expr::Binary {
                op: BinOp::Or,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_comparison()?;
        while self.consume(Token::And) {
            let right = self.parse_comparison()?;
            expr = Expr::Binary {
                op: BinOp::And,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let left = self.parse_additive()?;
        let op = match self.peek() {
            Some(Token::Gt) => BinOp::Gt,
            Some(Token::Ge) => BinOp::Ge,
            Some(Token::Lt) => BinOp::Lt,
            Some(Token::Le) => BinOp::Le,
            Some(Token::Eq) => BinOp::Eq,
            Some(Token::Ne) => BinOp::Ne,
            _ => return Ok(left),
        };
        self.next();
        let right = self.parse_additive()?;
        Ok(Expr::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    fn parse_additive(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_unary()?;
        loop {
            let op = if self.consume(Token::Plus) {
                Some(BinOp::Add)
            } else if self.consume(Token::Minus) {
                Some(BinOp::Sub)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_unary()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if self.consume(Token::Not) {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnOp::Not,
                expr: Box::new(expr),
            });
        }
        if self.consume(Token::Minus) {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnOp::Neg,
                expr: Box::new(expr),
            });
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.next() {
            Some(Token::Number(value)) => Ok(Expr::Number(value)),
            Some(Token::Str(value)) => Ok(Expr::String(value)),
            Some(Token::Ident(name)) => {
                if self.peek() == Some(&Token::LParen) {
                    self.next();
                    let mut args = Vec::new();
                    if self.peek() != Some(&Token::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if self.consume(Token::Comma) {
                                continue;
                            }
                            break;
                        }
                    }
                    if !self.consume(Token::RParen) {
                        return Err("expected ')'".to_string());
                    }
                    Ok(Expr::Call { name, args })
                } else if name == "parameters" {
                    if !self.consume(Token::Dot) {
                        return Err("parameters must be followed by a path".to_string());
                    }
                    let mut segments = Vec::new();
                    loop {
                        match self.next() {
                            Some(Token::Ident(segment)) => segments.push(segment),
                            other => {
                                return Err(format!("expected identifier after '.', found {other:?}"))
                            }
                        }
                        if !self.consume(Token::Dot) {
                            break;
                        }
                    }
                    Ok(Expr::Var(VarPath::Parameter(segments)))
                } else {
                    Ok(Expr::Var(VarPath::Ident(name)))
                }
            }
            Some(Token::LParen) => {
                let expr = self.parse_expr()?;
                if !self.consume(Token::RParen) {
                    return Err("expected ')'".to_string());
                }
                Ok(expr)
            }
            other => Err(format!("unexpected token {other:?}")),
        }
    }
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }

        if ch.is_ascii_digit()
            || (ch == '.'
                && chars
                    .clone()
                    .nth(1)
                    .map(|next| next.is_ascii_digit())
                    .unwrap_or(false))
        {
            let mut number = String::new();
            let mut seen_dot = false;
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() {
                    number.push(c);
                    chars.next();
                } else if c == '.' && !seen_dot {
                    seen_dot = true;
                    number.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            let value = number
                .parse::<f64>()
                .map_err(|_| format!("invalid number '{number}'"))?;
            if !value.is_finite() {
                return Err(format!("number '{number}' must be finite"));
            }
            tokens.push(Token::Number(value));
            continue;
        }

        if ch.is_ascii_alphabetic() || ch == '_' {
            let mut ident = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_alphanumeric() || c == '_' {
                    ident.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            let token = match ident.as_str() {
                "and" => Token::And,
                "or" => Token::Or,
                "not" => Token::Not,
                "if" => Token::If,
                "then" => Token::Then,
                _ => Token::Ident(ident),
            };
            tokens.push(token);
            continue;
        }

        match ch {
            '\'' => {
                chars.next();
                let mut value = String::new();
                let mut terminated = false;
                while let Some(c) = chars.next() {
                    if c == '\'' {
                        terminated = true;
                        break;
                    }
                    if c == '\\' {
                        if let Some(escaped) = chars.next() {
                            value.push(escaped);
                        } else {
                            return Err("unterminated string literal".to_string());
                        }
                    } else {
                        value.push(c);
                    }
                }
                if !terminated {
                    return Err("unterminated string literal".to_string());
                }
                tokens.push(Token::Str(value));
            }
            '(' => {
                chars.next();
                tokens.push(Token::LParen);
            }
            ')' => {
                chars.next();
                tokens.push(Token::RParen);
            }
            '.' => {
                chars.next();
                tokens.push(Token::Dot);
            }
            ',' => {
                chars.next();
                tokens.push(Token::Comma);
            }
            '+' => {
                chars.next();
                tokens.push(Token::Plus);
            }
            '-' => {
                chars.next();
                tokens.push(Token::Minus);
            }
            '>' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Ge);
                } else {
                    tokens.push(Token::Gt);
                }
            }
            '<' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Le);
                } else {
                    tokens.push(Token::Lt);
                }
            }
            '=' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Eq);
                } else {
                    return Err("expected '=' after '='".to_string());
                }
            }
            '!' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Ne);
                } else {
                    return Err("expected '=' after '!'".to_string());
                }
            }
            other => {
                return Err(format!("unexpected character '{other}'"));
            }
        }
    }
    Ok(tokens)
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ExprMode {
    Unlock,
    Constraint,
}

pub(crate) fn validate_expression(
    expr: &Expr,
    mode: ExprMode,
    param_paths: &BTreeSet<String>,
) -> Result<(), String> {
    match expr {
        Expr::Number(_) | Expr::String(_) => Ok(()),
        Expr::Var(path) => match path {
            VarPath::Parameter(segments) => {
                if let ExprMode::Unlock = mode {
                    return Err(
                        "parameter references are not allowed in unlock expressions".to_string(),
                    );
                }
                if segments.len() < 2 {
                    return Err(
                        "parameter path must include subsystem and parameter name".to_string(),
                    );
                }
                let key = format!("parameters.{}", segments.join("."));
                if !param_paths.contains(&key) {
                    return Err(format!("unknown parameter reference '{key}'"));
                }
                Ok(())
            }
            VarPath::Ident(name) => {
                if name == "era" || name.ends_with("_lvl") {
                    Ok(())
                } else {
                    Err(format!("unknown identifier '{name}'"))
                }
            }
        },
        Expr::Unary { expr, .. } => validate_expression(expr, mode, param_paths),
        Expr::Binary { left, right, .. } => {
            validate_expression(left, mode, param_paths)?;
            validate_expression(right, mode, param_paths)?;
            Ok(())
        }
        Expr::IfThen { cond, then_expr } => {
            validate_expression(cond, mode, param_paths)?;
            validate_expression(then_expr, mode, param_paths)
        }
        Expr::Call { name, args } => match name.as_str() {
            "has_upgrade" => {
                if args.len() != 1 {
                    return Err("has_upgrade expects exactly one argument".to_string());
                }
                validate_expression(&args[0], mode, param_paths)
            }
            "abs" => {
                if let ExprMode::Unlock = mode {
                    return Err("abs is not allowed in unlock expressions".to_string());
                }
                if args.len() != 1 {
                    return Err("abs expects exactly one argument".to_string());
                }
                validate_expression(&args[0], mode, param_paths)
            }
            other => Err(format!("unknown function '{other}'")),
        },
    }
}

#[derive(Clone, Debug)]
enum Value {
    Number(f64),
    String(String),
    Bool(bool),
}

pub(crate) struct EvalContext<'a> {
    pub(crate) era: u32,
    pub(crate) category_levels: &'a BTreeMap<String, i64>,
    pub(crate) owned_upgrades: &'a BTreeSet<String>,
    pub(crate) parameters: Option<&'a JsonValue>,
}

pub(crate) fn eval_bool(expr: &Expr, ctx: &EvalContext<'_>) -> Result<bool, String> {
    match eval(expr, ctx)? {
        Value::Bool(value) => Ok(value),
        _ => Err("expression must evaluate to boolean".to_string()),
    }
}

fn eval(expr: &Expr, ctx: &EvalContext<'_>) -> Result<Value, String> {
    match expr {
        Expr::Number(value) => Ok(Value::Number(*value)),
        Expr::String(value) => Ok(Value::String(value.clone())),
        Expr::Var(path) => match path {
            VarPath::Parameter(segments) => {
                let params = ctx
                    .parameters
                    .ok_or_else(|| "parameter references are not available".to_string())?;
                resolve_parameter(params, segments)
            }
            VarPath::Ident(name) => {
                if name == "era" {
                    Ok(Value::Number(ctx.era as f64))
                } else if name.ends_with("_lvl") {
                    let level = ctx.category_levels.get(name).copied().unwrap_or(0);
                    Ok(Value::Number(level as f64))
                } else {
                    Err(format!("unknown identifier '{name}'"))
                }
            }
        },
        Expr::Unary { op, expr } => {
            let value = eval(expr, ctx)?;
            match op {
                UnOp::Not => match value {
                    Value::Bool(value) => Ok(Value::Bool(!value)),
                    _ => Err("not expects a boolean".to_string()),
                },
                UnOp::Neg => match value {
                    Value::Number(value) => Ok(Value::Number(-value)),
                    _ => Err("unary '-' expects a number".to_string()),
                },
            }
        }
        Expr::Binary { op, left, right } => match op {
            BinOp::And => {
                let left_val = eval_bool(left, ctx)?;
                if !left_val {
                    return Ok(Value::Bool(false));
                }
                let right_val = eval_bool(right, ctx)?;
                Ok(Value::Bool(right_val))
            }
            BinOp::Or => {
                let left_val = eval_bool(left, ctx)?;
                if left_val {
                    return Ok(Value::Bool(true));
                }
                let right_val = eval_bool(right, ctx)?;
                Ok(Value::Bool(right_val))
            }
            BinOp::Add | BinOp::Sub => {
                let left_val = eval(left, ctx)?;
                let right_val = eval(right, ctx)?;
                let left_num = match left_val {
                    Value::Number(value) => value,
                    _ => return Err("arithmetic expects numbers".to_string()),
                };
                let right_num = match right_val {
                    Value::Number(value) => value,
                    _ => return Err("arithmetic expects numbers".to_string()),
                };
                let value = match op {
                    BinOp::Add => left_num + right_num,
                    BinOp::Sub => left_num - right_num,
                    _ => unreachable!(),
                };
                Ok(Value::Number(value))
            }
            BinOp::Gt | BinOp::Ge | BinOp::Lt | BinOp::Le => {
                let left_val = eval(left, ctx)?;
                let right_val = eval(right, ctx)?;
                let left_num = match left_val {
                    Value::Number(value) => value,
                    _ => return Err("comparison expects numbers".to_string()),
                };
                let right_num = match right_val {
                    Value::Number(value) => value,
                    _ => return Err("comparison expects numbers".to_string()),
                };
                let result = match op {
                    BinOp::Gt => left_num > right_num,
                    BinOp::Ge => left_num >= right_num,
                    BinOp::Lt => left_num < right_num,
                    BinOp::Le => left_num <= right_num,
                    _ => unreachable!(),
                };
                Ok(Value::Bool(result))
            }
            BinOp::Eq | BinOp::Ne => {
                let left_val = eval(left, ctx)?;
                let right_val = eval(right, ctx)?;
                let result = match (left_val, right_val) {
                    (Value::Number(lhs), Value::Number(rhs)) => lhs == rhs,
                    (Value::String(lhs), Value::String(rhs)) => lhs == rhs,
                    (Value::Bool(lhs), Value::Bool(rhs)) => lhs == rhs,
                    _ => return Err("equality expects matching types".to_string()),
                };
                let result = if matches!(op, BinOp::Ne) { !result } else { result };
                Ok(Value::Bool(result))
            }
        },
        Expr::IfThen { cond, then_expr } => {
            let cond_val = eval_bool(cond, ctx)?;
            if !cond_val {
                return Ok(Value::Bool(true));
            }
            let then_val = eval_bool(then_expr, ctx)?;
            Ok(Value::Bool(then_val))
        }
        Expr::Call { name, args } => match name.as_str() {
            "has_upgrade" => {
                if args.len() != 1 {
                    return Err("has_upgrade expects exactly one argument".to_string());
                }
                let arg = eval(&args[0], ctx)?;
                let id = match arg {
                    Value::String(value) => value,
                    _ => return Err("has_upgrade expects a string argument".to_string()),
                };
                Ok(Value::Bool(ctx.owned_upgrades.contains(&id)))
            }
            "abs" => {
                if args.len() != 1 {
                    return Err("abs expects exactly one argument".to_string());
                }
                let arg = eval(&args[0], ctx)?;
                let value = match arg {
                    Value::Number(value) => value,
                    _ => return Err("abs expects a number argument".to_string()),
                };
                Ok(Value::Number(value.abs()))
            }
            other => Err(format!("unknown function '{other}'")),
        },
    }
}

fn resolve_parameter(params: &JsonValue, segments: &[String]) -> Result<Value, String> {
    let mut current = params;
    for segment in segments {
        current = current
            .get(segment)
            .ok_or_else(|| format!("unknown parameter 'parameters.{}'", segments.join(".")))?;
    }
    match current {
        JsonValue::Number(number) => number
            .as_f64()
            .ok_or_else(|| "parameter must be a finite number".to_string())
            .map(Value::Number),
        JsonValue::String(value) => Ok(Value::String(value.clone())),
        _ => Err("parameter must be a number or string".to_string()),
    }
}
