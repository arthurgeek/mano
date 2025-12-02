mod ast;
mod environment;
mod error;
mod interpreter;
mod parser;
mod resolver;
mod scanner;
mod token;
mod value;

use std::io::Write;

pub use ast::Stmt;
pub use error::ManoError;
pub use parser::Parser;
pub use scanner::{KEYWORDS, Scanner, is_identifier_char, is_identifier_start};
pub use token::{Literal, Token, TokenType};

/// Native functions available in the interpreter
pub const NATIVE_FUNCTIONS: &[&str] = &["fazTeuCorre"];

pub struct Mano {
    interpreter: interpreter::Interpreter,
}

impl Default for Mano {
    fn default() -> Self {
        Self::new()
    }
}

impl Mano {
    pub fn new() -> Self {
        Self {
            interpreter: interpreter::Interpreter::new(),
        }
    }

    pub fn variable_names(&self) -> Vec<String> {
        self.interpreter.variable_names()
    }

    pub fn run<O: Write>(&mut self, source: &str, mut stdout: O) -> Vec<ManoError> {
        let mut errors = Vec::new();
        let scanner = scanner::Scanner::new(source);

        let mut tokens = Vec::new();
        for result in scanner {
            match result {
                Ok(token) => tokens.push(token),
                Err(e) => errors.push(e),
            }
        }

        if !errors.is_empty() {
            return errors;
        }

        let mut parser = parser::Parser::new(tokens);
        let statements = parser.parse().unwrap();

        errors.extend(parser.take_errors());

        if !errors.is_empty() {
            return errors;
        }

        // Resolve variable bindings
        let resolver = resolver::Resolver::new();
        let resolutions = match resolver.resolve(&statements) {
            Ok(r) => r,
            Err(errs) => {
                return errs;
            }
        };

        self.interpreter.set_resolutions(resolutions);

        for stmt in &statements {
            if let Err(e) = self.interpreter.execute(stmt, &mut stdout) {
                errors.push(e);
            }
        }

        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_empty_source_returns_no_errors() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let errors = mano.run("", &mut stdout);
        assert!(errors.is_empty());
    }

    #[test]
    fn run_comment_only_returns_no_errors() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let errors = mano.run("// só um comentário", &mut stdout);
        assert!(errors.is_empty());
    }

    #[test]
    fn run_valid_statement_returns_no_errors() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let errors = mano.run("salve 42;", &mut stdout);
        assert!(errors.is_empty());
    }

    #[test]
    fn run_returns_scanner_error() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let errors = mano.run("@", &mut stdout);
        assert_eq!(errors.len(), 1);
        if let ManoError::Scan { message, .. } = &errors[0] {
            assert!(message.contains('@'));
        } else {
            panic!("Expected Scan error");
        }
    }

    #[test]
    fn run_returns_multiple_scanner_errors() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let errors = mano.run("@$", &mut stdout);
        assert_eq!(errors.len(), 2);
        if let ManoError::Scan { message, .. } = &errors[0] {
            assert!(message.contains('@'));
        } else {
            panic!("Expected Scan error");
        }
        if let ManoError::Scan { message, .. } = &errors[1] {
            assert!(message.contains('$'));
        } else {
            panic!("Expected Scan error");
        }
    }

    #[test]
    fn run_executes_print_statement() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        mano.run("salve 1 + 2;", &mut stdout);
        let output = String::from_utf8(stdout).unwrap();
        assert_eq!(output.trim(), "3");
    }

    #[test]
    fn run_returns_parser_error() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let errors = mano.run("1 +", &mut stdout);
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ManoError::Parse { .. }));
    }

    #[test]
    fn run_returns_runtime_error() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        // Use variable so type error is only caught at runtime
        let errors = mano.run("seLiga x = \"mano\"; salve -x;", &mut stdout);
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ManoError::Runtime { .. }));
    }

    #[test]
    fn run_returns_resolution_error_for_literal_type_mismatch() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        // Literal type errors are caught at resolution time
        let errors = mano.run("salve -\"mano\";", &mut stdout);
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ManoError::Resolution { .. }));
    }

    #[test]
    fn run_returns_multiple_parser_errors() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let errors = mano.run("seLiga = 1; seLiga y", &mut stdout);
        assert!(errors.len() >= 2);
        assert!(errors.iter().all(|e| matches!(e, ManoError::Parse { .. })));
    }

    #[test]
    fn repl_persists_variables_across_runs() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();

        mano.run("seLiga x = 42;", &mut stdout);

        stdout.clear();
        mano.run("salve x;", &mut stdout);
        let output = String::from_utf8(stdout).unwrap();
        assert_eq!(output.trim(), "42");
    }

    #[test]
    fn errors_dont_affect_subsequent_runs() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();

        // First run has error
        let errors = mano.run("@", &mut stdout);
        assert_eq!(errors.len(), 1);

        // Second run should work fine (no reset needed)
        stdout.clear();
        let errors = mano.run("salve 42;", &mut stdout);
        assert!(errors.is_empty());
        assert_eq!(String::from_utf8(stdout).unwrap().trim(), "42");
    }

    #[test]
    fn default_creates_new_mano() {
        let _mano: Mano = Default::default();
    }

    #[test]
    fn run_returns_resolution_error_for_self_reference() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        // seLiga a = a; -> self-reference in initializer
        let errors = mano.run("{ seLiga a = a; }", &mut stdout);
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ManoError::Resolution { .. }));
    }

    #[test]
    fn run_returns_resolution_error_for_duplicate_var() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        // Two vars with same name in same scope (use underscore prefix to avoid unused var error)
        let errors = mano.run("{ seLiga _x = 1; seLiga _x = 2; }", &mut stdout);
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ManoError::Resolution { .. }));
    }

    // === closure semantics tests ===

    #[test]
    fn closure_captures_variable_at_definition_time() {
        // This is the key test from Chapter 11 - the closure bug
        // The closure should capture 'a' from the enclosing scope
        // where it was DEFINED, not where it's CALLED.
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let code = r#"
            seLiga a = "global";
            {
                olhaEssaFita mostra() {
                    salve a;
                }

                mostra();
                seLiga _a = "block";
                mostra();
            }
        "#;
        let errors = mano.run(code, &mut stdout);
        assert!(errors.is_empty(), "Got errors: {:?}", errors);
        let output = String::from_utf8(stdout).unwrap();
        let lines: Vec<&str> = output.trim().lines().collect();
        // Both calls should print "global" because the closure
        // was resolved to the outer 'a', not the inner shadowing '_a'
        assert_eq!(lines, vec!["global", "global"]);
    }

    #[test]
    fn closure_captures_enclosing_scope_correctly() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let code = r#"
            olhaEssaFita makeCounter() {
                seLiga i = 0;
                olhaEssaFita count() {
                    i = i + 1;
                    salve i;
                }
                toma count;
            }

            seLiga counter = makeCounter();
            counter();
            counter();
            counter();
        "#;
        let errors = mano.run(code, &mut stdout);
        assert!(errors.is_empty(), "Got errors: {:?}", errors);
        let output = String::from_utf8(stdout).unwrap();
        let lines: Vec<&str> = output.trim().lines().collect();
        assert_eq!(lines, vec!["1", "2", "3"]);
    }

    #[test]
    fn nested_closures_resolve_correctly() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let code = r#"
            seLiga x = "outer";
            olhaEssaFita outer() {
                seLiga x = "middle";
                olhaEssaFita inner() {
                    salve x;
                }
                inner();
            }
            outer();
        "#;
        let errors = mano.run(code, &mut stdout);
        assert!(errors.is_empty(), "Got errors: {:?}", errors);
        let output = String::from_utf8(stdout).unwrap();
        assert_eq!(output.trim(), "middle");
    }
}
