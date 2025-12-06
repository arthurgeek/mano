//! mano-vm: Bytecode virtual machine for the mano programming language

mod chunk;
mod compiler;
mod debug;
mod opcode;
mod value;
mod vm;

use std::io::Write;

use mano::ManoError;

pub use chunk::Chunk;
pub use compiler::compile;
pub use debug::{disassemble_chunk, disassemble_instruction};
pub use opcode::OpCode;
pub use vm::{InterpretResult, VM};

/// Run mano source code.
///
/// Compiles the source to bytecode and executes it in the VM.
/// When trace is enabled, dumps the compiled chunk before execution.
pub fn run<W: Write>(source: &str, output: &mut W, trace: bool) -> Result<(), Vec<ManoError>> {
    let chunk = compile(source)?;
    if trace {
        write!(output, "{}", disassemble_chunk(&chunk, "code")).unwrap();
    }
    let mut vm = VM::new(&chunk, output);
    vm.set_trace(trace);
    vm.interpret()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_with_trace_dumps_chunk() {
        let mut output = Vec::new();
        run("42", &mut output, true).unwrap();
        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("== code =="));
        assert!(output_str.contains("OP_CONSTANT"));
    }
}
