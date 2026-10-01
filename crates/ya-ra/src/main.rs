use std::io::{BufRead, Write};
use ya_ra::lexer::lex;
use ya_ra::parser::Parser;
use ya_ra::Interpreter;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() >= 2 {
        // File mode
        let path = &args[1];
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error reading {path}: {e}");
                std::process::exit(1);
            }
        };
        let mut interp = Interpreter::new();
        match run_source(&src, &mut interp) {
            Ok(()) => {
                eprintln!();
                eprintln!("registry: {} entries", interp.registry.len());
                eprintln!("distinct intents : {}", interp.registry.distinct_intents());
                eprintln!("distinct patterns: {}", interp.registry.distinct_patterns());
            }
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // REPL mode
    println!("ya|ra");
    println!("type an equation (e.g. 2 = 1 + 1) or a query (e.g. ?intent 2)");
    println!("empty line or :quit to exit");
    println!();

    let stdin = std::io::stdin();
    let mut interp = Interpreter::new();

    loop {
        print!("> ");
        std::io::stdout().flush().ok();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                eprintln!("input error: {e}");
                break;
            }
        }

        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed == ":quit" {
            break;
        }

        match run_source(&line, &mut interp) {
            Ok(()) => {}
            Err(e) => eprintln!("error: {e}"),
        }
    }

    println!();
    println!("registry held {} entries.", interp.registry.len());
}

fn run_source(src: &str, interp: &mut Interpreter) -> Result<(), ya_ra::Error> {
    let tokens = lex(src)?;
    let stmts = Parser::new(tokens).parse_program()?;
    let mut stdout = std::io::stdout();
    interp.run(&stmts, &mut stdout)?;
    Ok(())
}
