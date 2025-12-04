//! mano-vm: Bytecode virtual machine for the mano programming language

mod chunk;
mod debug;
mod opcode;
mod value;
mod vm;

pub use chunk::Chunk;
pub use debug::{disassemble_chunk, disassemble_instruction};
pub use opcode::OpCode;

pub fn run(debug: bool) -> String {
    let mut chunk = Chunk::new();

    // Compute: -((1.2 + 3.4) / 2)
    chunk.write_constant(1.2, 0..0);
    chunk.write_constant(3.4, 0..0);
    chunk.write(OpCode::Add.into(), 0..0);
    chunk.write_constant(2.0, 0..0);
    chunk.write(OpCode::Divide.into(), 0..0);
    chunk.write(OpCode::Negate.into(), 0..0);
    chunk.write(OpCode::Return.into(), 0..0);

    let mut output = Vec::new();
    let mut vm = vm::VM::new(&chunk, &mut output);
    vm.set_trace(debug);
    vm.interpret();
    String::from_utf8(output).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_returns_result() {
        let output = run(false);
        assert_eq!(output, "-2.3\n");
    }

    #[test]
    fn run_with_debug_traces_execution() {
        let output = run(true);
        assert!(output.contains("OP_CONSTANT"));
        assert!(output.contains("OP_ADD"));
        assert!(output.contains("OP_DIVIDE"));
        assert!(output.contains("OP_NEGATE"));
        assert!(output.contains("OP_RETURN"));
    }
}
