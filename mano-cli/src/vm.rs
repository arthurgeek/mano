//! VM wrapper that implements the Runner trait

use std::io::Write;

use mano::{ManoError, Runner};

/// Bytecode VM wrapper that implements the Runner trait.
pub struct Vm {
    debug: bool,
}

impl Vm {
    /// Create a new VM.
    pub fn new() -> Self {
        Self { debug: false }
    }

    /// Enable debug tracing during execution.
    pub fn set_debug(&mut self, debug: bool) {
        self.debug = debug;
    }
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

impl Runner for Vm {
    fn run<W: Write>(&mut self, source: &str, mut stdout: W) -> Result<(), Vec<ManoError>> {
        mano_vm::run(source, &mut stdout, self.debug)
    }

    fn variable_names(&self) -> Vec<String> {
        // TODO: Return actual variable names once globals are implemented
        Vec::new()
    }

    fn supports_auto_print(&self) -> bool {
        false // VM only handles expressions, no print statement yet
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vm_new_creates_instance() {
        let vm = Vm::new();
        assert!(!vm.debug);
    }

    #[test]
    fn vm_default_creates_instance() {
        let vm = Vm::default();
        assert!(!vm.debug);
    }

    #[test]
    fn vm_set_debug_enables_tracing() {
        let mut vm = Vm::new();
        vm.set_debug(true);
        assert!(vm.debug);
    }

    // TODO: Add vm_implements_runner test once compile() produces proper bytecode
    // Currently empty chunk causes infinite loop in VM

    #[test]
    fn vm_variable_names_returns_empty() {
        let vm = Vm::new();
        assert!(Runner::variable_names(&vm).is_empty());
    }

    #[test]
    fn vm_run_executes_expression() {
        let mut vm = Vm::new();
        let mut output = Vec::new();
        let result = Runner::run(&mut vm, "1 + 2", &mut output);
        assert!(result.is_ok());
        assert_eq!(String::from_utf8(output).unwrap(), "3\n");
    }

    #[test]
    fn vm_run_with_debug_traces() {
        let mut vm = Vm::new();
        vm.set_debug(true);
        let mut output = Vec::new();
        let result = Runner::run(&mut vm, "42", &mut output);
        assert!(result.is_ok());
        let out = String::from_utf8(output).unwrap();
        assert!(out.contains("== code =="));
        assert!(out.contains("== trace =="));
    }

    #[test]
    fn vm_supports_auto_print_returns_false() {
        let vm = Vm::new();
        assert!(!Runner::supports_auto_print(&vm));
    }
}
