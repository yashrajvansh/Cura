use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Int(i64),
    BinOp(Box<Expr>, Op, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
}

impl Op {
    pub fn symbol(self) -> &'static str {
        match self {
            Op::Add => "+",
            Op::Sub => "-",
            Op::Mul => "*",
            Op::Div => "/",
            Op::Mod => "%",
            Op::Pow => "**",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    /// `LHS = RHS` — an equation. When evaluated, records three facts.
    Equation(Expr, Expr),
    /// `?intent <expr>` — all patterns and weaves that reach this intent.
    QueryIntent(Expr),
    /// `?pattern <literal>` — the intent and weave for this pattern.
    QueryPattern(Vec<i64>),
    /// `?weave <op-set>` — all intents and patterns using this weave.
    QueryWeave(BTreeSet<Op>),
}

/// Collect all leaf integers in an expression, in left-to-right order.
pub fn leaves(expr: &Expr) -> Vec<i64> {
    let mut out = Vec::new();
    collect(expr, &mut out);
    out
}

fn collect(expr: &Expr, out: &mut Vec<i64>) {
    match expr {
        Expr::Int(n) => out.push(*n),
        Expr::BinOp(l, _, r) => {
            collect(l, out);
            collect(r, out);
        }
    }
}

/// Collect all operators in an expression as a set.
pub fn operators(expr: &Expr) -> BTreeSet<Op> {
    let mut out = BTreeSet::new();
    collect_ops(expr, &mut out);
    out
}

fn collect_ops(expr: &Expr, out: &mut BTreeSet<Op>) {
    match expr {
        Expr::Int(_) => {}
        Expr::BinOp(l, op, r) => {
            out.insert(*op);
            collect_ops(l, out);
            collect_ops(r, out);
        }
    }
}
