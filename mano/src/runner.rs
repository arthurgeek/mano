//! Runner trait for unified interpreter/VM execution

use std::io::Write;

use crate::ManoError;

/// Trait for running mano source code.
///
/// This trait provides a unified interface for both the tree-walk interpreter
/// and the bytecode VM, allowing the CLI to use either backend.
pub trait Runner {
    /// Run source code and write output to the provided writer.
    ///
    /// Returns `Ok(())` on success, or a vector of errors on failure.
    fn run<W: Write>(&mut self, source: &str, stdout: W) -> Result<(), Vec<ManoError>>;

    /// Get the names of all variables currently defined in the environment.
    ///
    /// Used for REPL autocompletion.
    fn variable_names(&self) -> Vec<String>;

    /// Whether the REPL should auto-print expressions.
    ///
    /// Returns true by default. The VM returns false since it only handles
    /// expressions (no print statement yet).
    fn supports_auto_print(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Mano;

    #[test]
    fn mano_implements_runner() {
        let mut mano = Mano::new();
        let result = Runner::run(&mut mano, "salve 42;", &mut Vec::new());
        assert!(result.is_ok());
    }

    #[test]
    fn mano_runner_returns_errors() {
        let mut mano = Mano::new();
        let result = Runner::run(&mut mano, "@", &mut Vec::new());
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(!errors.is_empty());
    }

    #[test]
    fn mano_runner_writes_output() {
        let mut mano = Mano::new();
        let mut output = Vec::new();
        let _ = Runner::run(&mut mano, "salve 42;", &mut output);
        let output_str = String::from_utf8(output).unwrap();
        assert_eq!(output_str.trim(), "42");
    }

    #[test]
    fn mano_runner_variable_names_returns_defined_vars() {
        let mut mano = Mano::new();
        let _ = Runner::run(&mut mano, "seLiga x = 1;", &mut Vec::new());
        let names = Runner::variable_names(&mano);
        assert!(names.contains(&"x".to_string()));
    }
}
