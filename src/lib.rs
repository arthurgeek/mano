mod ast;
mod environment;
mod error;
mod interpreter;
mod parser;
mod scanner;
mod token;

use std::fs;
use std::io::Write;
use std::path::Path;

pub use error::ManoError;

pub struct Mano {
    had_error: bool,
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
            had_error: false,
            interpreter: interpreter::Interpreter::new(),
        }
    }

    pub fn reset_error(&mut self) {
        self.had_error = false;
    }

    pub fn run_file(&mut self, file: &Path) -> Result<(), ManoError> {
        let source = fs::read_to_string(file)?;
        self.run_with_output(&source, std::io::stdout(), std::io::stderr())
    }

    pub fn run_with_output<O: Write, E: Write>(
        &mut self,
        source: &str,
        mut stdout: O,
        mut stderr: E,
    ) -> Result<(), ManoError> {
        let scanner = scanner::Scanner::new(source);

        let mut tokens = Vec::new();
        for result in scanner {
            match result {
                Ok(token) => tokens.push(token),
                Err(e) => {
                    self.had_error = true;
                    writeln!(stderr, "{}", e)?;
                }
            }
        }

        if self.had_error {
            return Ok(());
        }

        let mut parser = parser::Parser::new(tokens);
        let statements = parser.parse().unwrap();

        for e in parser.errors() {
            self.had_error = true;
            writeln!(stderr, "{}", e)?;
        }

        if self.had_error {
            return Ok(());
        }

        for stmt in &statements {
            if let Err(e) = self.interpreter.execute(stmt, &mut stdout) {
                self.had_error = true;
                writeln!(stderr, "{}", e)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn run_empty_source_succeeds() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let result = mano.run_with_output("", &mut stdout, &mut stderr);
        assert!(result.is_ok());
    }

    #[test]
    fn run_comment_only_succeeds() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let result = mano.run_with_output("// só um comentário", &mut stdout, &mut stderr);
        assert!(result.is_ok());
        assert!(!mano.had_error);
    }

    #[test]
    fn run_parses_valid_statement() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let result = mano.run_with_output("salve 42;", &mut stdout, &mut stderr);
        assert!(result.is_ok());
        assert!(!mano.had_error);
    }

    #[test]
    fn run_sets_had_error_on_invalid_token() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let result = mano.run_with_output("@", &mut stdout, &mut stderr);
        assert!(result.is_ok()); // still returns Ok, but sets flag
        assert!(mano.had_error);
    }

    #[test]
    fn run_continues_scanning_to_find_more_errors() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let result = mano.run_with_output("@$", &mut stdout, &mut stderr);
        assert!(result.is_ok());
        assert!(mano.had_error);
        let errors = String::from_utf8(stderr).unwrap();
        assert!(errors.contains("@"));
        assert!(errors.contains("$"));
    }

    #[test]
    fn run_file_reads_and_runs_file() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, "salve \"oi\";").unwrap();

        let mut mano = Mano::new();
        let result = mano.run_file(file.path());
        assert!(result.is_ok());
    }

    #[test]
    fn run_file_returns_error_for_missing_file() {
        let mut mano = Mano::new();
        let result = mano.run_file(Path::new("nao_existe.mano"));
        assert!(matches!(result, Err(ManoError::Io(_))));
    }

    #[test]
    fn run_executes_print_statement() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = mano.run_with_output("salve 1 + 2;", &mut stdout, &mut stderr);

        assert!(result.is_ok());
        let output = String::from_utf8(stdout).unwrap();
        assert_eq!(output.trim(), "3");
    }

    #[test]
    fn run_outputs_errors_to_stderr() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = mano.run_with_output("@", &mut stdout, &mut stderr);

        assert!(result.is_ok());
        assert!(mano.had_error);
        let errors = String::from_utf8(stderr).unwrap();
        assert!(errors.contains("@"));
        assert!(errors.contains("Tá na nóia?"));
    }

    #[test]
    fn run_captures_multiple_errors() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = mano.run_with_output("@$", &mut stdout, &mut stderr);

        assert!(result.is_ok());
        let errors = String::from_utf8(stderr).unwrap();
        // Both invalid characters should be reported
        assert!(errors.contains("@"));
        assert!(errors.contains("$"));
    }

    #[test]
    fn default_creates_new_mano() {
        let mano: Mano = Default::default();
        assert!(!mano.had_error);
    }

    #[test]
    fn run_outputs_parser_errors_to_stderr() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        // Missing right operand - scans OK but fails to parse
        let result = mano.run_with_output("1 +", &mut stdout, &mut stderr);

        assert!(result.is_ok());
        assert!(mano.had_error);
        let errors = String::from_utf8(stderr).unwrap();
        assert!(!errors.is_empty());
    }

    #[test]
    fn run_outputs_runtime_errors_to_stderr() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        // Negating a string - scans and parses OK but fails at runtime
        let result = mano.run_with_output("salve -\"mano\";", &mut stdout, &mut stderr);

        assert!(result.is_ok());
        assert!(mano.had_error);
        let errors = String::from_utf8(stderr).unwrap();
        assert!(errors.contains("número")); // Error message mentions "número"
    }

    #[test]
    fn repl_resets_error_between_lines() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let mut mano = Mano::new();
        // First line has error
        mano.run_with_output("@", &mut stdout, &mut stderr).unwrap();
        assert!(mano.had_error);

        // Reset for next line (this is what run_prompt does)
        stdout.clear();
        stderr.clear();
        mano.had_error = false;

        // Second line should work
        mano.run_with_output("salve 1 + 2;", &mut stdout, &mut stderr)
            .unwrap();
        assert!(!mano.had_error);
        let output = String::from_utf8(stdout).unwrap();
        assert_eq!(output.trim(), "3");
    }

    #[test]
    fn run_reports_multiple_parser_errors() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        // Two parse errors: missing identifier, then missing semicolon
        let result = mano.run_with_output("seLiga = 1; seLiga y", &mut stdout, &mut stderr);

        assert!(result.is_ok());
        assert!(mano.had_error);
        let errors = String::from_utf8(stderr).unwrap();
        // Should report both errors
        assert!(errors.contains("linha"));
        assert!(errors.lines().count() >= 2);
    }

    #[test]
    fn repl_persists_variables_across_lines() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        // First line: declare variable
        mano.run_with_output("seLiga x = 42;", &mut stdout, &mut stderr)
            .unwrap();
        assert!(!mano.had_error);

        // Second line: access variable
        stdout.clear();
        mano.run_with_output("salve x;", &mut stdout, &mut stderr)
            .unwrap();
        assert!(!mano.had_error);
        let output = String::from_utf8(stdout).unwrap();
        assert_eq!(output.trim(), "42");
    }
}
