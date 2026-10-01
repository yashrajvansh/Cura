use crate::ast::{leaves, operators, Expr, Op, Stmt};
use crate::error::{Error, Result};
use crate::registry::{pattern_to_string, to_binary, weave_to_string, Intent, Registry};

pub struct Interpreter {
    pub registry: Registry,
    pub verbose: bool,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            registry: Registry::new(),
            verbose: true,
        }
    }

    pub fn run(&mut self, stmts: &[Stmt], out: &mut dyn std::io::Write) -> Result<()> {
        for stmt in stmts {
            match stmt {
                Stmt::Equation(lhs, rhs) => {
                    self.eval_equation(lhs, rhs, out)?;
                }
                Stmt::QueryIntent(e) => {
                    let v = eval_expr(e)?;
                    self.print_intent_query(v, out)?;
                }
                Stmt::QueryPattern(p) => {
                    self.print_pattern_query(p, out)?;
                }
                Stmt::QueryWeave(w) => {
                    self.print_weave_query(w, out)?;
                }
            }
        }
        Ok(())
    }

    fn eval_equation(
        &mut self,
        lhs: &Expr,
        rhs: &Expr,
        out: &mut dyn std::io::Write,
    ) -> Result<()> {
        let lv = eval_expr(lhs)?;
        let rv = eval_expr(rhs)?;

        if lv != rv {
            return Err(Error::Eval(format!("equation does not hold: {lv} != {rv}")));
        }

        // Pattern: leaf literals of RHS + the answer.
        let mut pattern = leaves(rhs);
        pattern.push(rv);

        // Weave: operators of RHS + '=' (the equation itself).
        let weave = operators(rhs);
        // '=' has no Op variant; it is implicit in every equation.
        // weave_to_string appends it at display time.

        let intent = Intent {
            value: rv,
            binary: to_binary(rv),
        };

        self.registry
            .record(intent.clone(), pattern.clone(), weave.clone());

        if self.verbose {
            writeln!(out, "[+] {lv} = {}", render_expr(rhs))
                .map_err(|e| Error::Eval(e.to_string()))?;
            writeln!(
                out,
                "    intent  : {}  (binary {})",
                intent.value, intent.binary
            )
            .map_err(|e| Error::Eval(e.to_string()))?;
            writeln!(out, "    pattern : {}", pattern_to_string(&pattern))
                .map_err(|e| Error::Eval(e.to_string()))?;
            writeln!(out, "    weave   : {}", weave_to_string(&weave))
                .map_err(|e| Error::Eval(e.to_string()))?;
            writeln!(out).map_err(|e| Error::Eval(e.to_string()))?;
        }

        Ok(())
    }

    fn print_intent_query(&self, value: i64, out: &mut dyn std::io::Write) -> Result<()> {
        let entries = self.registry.by_intent(value);
        writeln!(out, "?intent {value}").map_err(|e| Error::Eval(e.to_string()))?;
        if entries.is_empty() {
            writeln!(out, "  (nothing recorded)").map_err(|e| Error::Eval(e.to_string()))?;
            return Ok(());
        }
        writeln!(out, "  binary : {}", to_binary(value)).map_err(|e| Error::Eval(e.to_string()))?;
        writeln!(out, "  ways   :").map_err(|e| Error::Eval(e.to_string()))?;
        for e in entries {
            writeln!(
                out,
                "    pattern {:<16}  weave {}",
                pattern_to_string(&e.pattern),
                weave_to_string(&e.weave)
            )
            .map_err(|err| Error::Eval(err.to_string()))?;
        }
        Ok(())
    }

    fn print_pattern_query(&self, pattern: &[i64], out: &mut dyn std::io::Write) -> Result<()> {
        let entries = self.registry.by_pattern(pattern);
        writeln!(out, "?pattern {}", pattern_to_string(pattern))
            .map_err(|e| Error::Eval(e.to_string()))?;
        if entries.is_empty() {
            writeln!(out, "  (nothing recorded)").map_err(|e| Error::Eval(e.to_string()))?;
            return Ok(());
        }
        for e in entries {
            writeln!(
                out,
                "  intent {}  weave {}",
                e.intent.value,
                weave_to_string(&e.weave)
            )
            .map_err(|err| Error::Eval(err.to_string()))?;
        }
        Ok(())
    }

    fn print_weave_query(
        &self,
        weave: &std::collections::BTreeSet<Op>,
        out: &mut dyn std::io::Write,
    ) -> Result<()> {
        let entries = self.registry.by_weave(weave);
        writeln!(out, "?weave {}", weave_to_string(weave))
            .map_err(|e| Error::Eval(e.to_string()))?;
        if entries.is_empty() {
            writeln!(out, "  (nothing recorded)").map_err(|e| Error::Eval(e.to_string()))?;
            return Ok(());
        }
        for e in entries {
            writeln!(
                out,
                "  intent {:<8}  pattern {}",
                e.intent.value,
                pattern_to_string(&e.pattern)
            )
            .map_err(|err| Error::Eval(err.to_string()))?;
        }
        Ok(())
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

pub fn eval_expr(expr: &Expr) -> Result<i64> {
    match expr {
        Expr::Int(n) => Ok(*n),
        Expr::BinOp(l, op, r) => {
            let lv = eval_expr(l)?;
            let rv = eval_expr(r)?;
            match op {
                Op::Add => lv.checked_add(rv).ok_or(Error::Overflow),
                Op::Sub => lv.checked_sub(rv).ok_or(Error::Overflow),
                Op::Mul => lv.checked_mul(rv).ok_or(Error::Overflow),
                Op::Div => {
                    if rv == 0 {
                        return Err(Error::DivisionByZero);
                    }
                    lv.checked_div(rv).ok_or(Error::Overflow)
                }
                Op::Mod => {
                    if rv == 0 {
                        return Err(Error::DivisionByZero);
                    }
                    lv.checked_rem(rv).ok_or(Error::Overflow)
                }
                Op::Pow => {
                    if rv < 0 {
                        return Err(Error::InvalidPower);
                    }
                    lv.checked_pow(rv as u32).ok_or(Error::Overflow)
                }
            }
        }
    }
}

pub fn render_expr(expr: &Expr) -> String {
    match expr {
        Expr::Int(n) => n.to_string(),
        Expr::BinOp(l, op, r) => {
            format!("{} {} {}", render_expr(l), op.symbol(), render_expr(r))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::Parser;

    fn run(src: &str) -> (Registry, String) {
        let toks = lex(src).unwrap();
        let stmts = Parser::new(toks).parse_program().unwrap();
        let mut interp = Interpreter::new();
        let mut buf: Vec<u8> = Vec::new();
        interp.run(&stmts, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        (std::mem::take(&mut interp.registry), out)
    }

    #[test]
    fn records_1_plus_1() {
        let (reg, _) = run("2 = 1 + 1");
        assert_eq!(reg.len(), 1);
        let e = &reg.all()[0];
        assert_eq!(e.intent.value, 2);
        assert_eq!(e.intent.binary, "10");
        assert_eq!(e.pattern, vec![1, 1, 2]);
        assert_eq!(e.weave.len(), 1);
    }

    #[test]
    fn three_ways_to_two() {
        let (reg, _) = run("2 = 1 + 1\n\
             2 = 3 - 1\n\
             2 = 2 * 1\n");
        assert_eq!(reg.len(), 3);
        assert_eq!(reg.by_intent(2).len(), 3);
        assert_eq!(reg.distinct_intents(), 1);
        assert_eq!(reg.distinct_patterns(), 3);
    }

    #[test]
    fn pattern_query() {
        let (_, out) = run("2 = 1 + 1\n?pattern 1_1_2\n");
        assert!(out.contains("intent 2"));
    }

    #[test]
    fn weave_query() {
        let (_, out) = run("2 = 1 + 1\n2 = 3 - 1\n?weave +,=\n");
        // Only the first equation has weave {+,=}
        assert!(out.contains("intent 2"));
    }

    #[test]
    fn non_holding_equation_errors() {
        let toks = lex("2 = 1 + 2").unwrap();
        let stmts = Parser::new(toks).parse_program().unwrap();
        let mut interp = Interpreter::new();
        let mut buf: Vec<u8> = Vec::new();
        assert!(interp.run(&stmts, &mut buf).is_err());
    }
}
