mod ast;
mod environment;
mod error;
mod interpreter;
mod parser;
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
        let errors = mano.run("salve -\"mano\";", &mut stdout);
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ManoError::Runtime { .. }));
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
}
