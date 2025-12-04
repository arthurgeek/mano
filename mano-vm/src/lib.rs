//! mano-vm: Bytecode virtual machine for the mano programming language

mod chunk;
mod debug;
mod opcode;
mod value;

pub use chunk::Chunk;
pub use debug::{disassemble_chunk, disassemble_instruction};
pub use opcode::OpCode;

pub fn run() -> String {
    let mut chunk = Chunk::new();

    let constant = chunk.add_constant(1.2);
    chunk.write(OpCode::Constant.into(), 0..0);
    chunk.write(constant as u8, 0..0);

    chunk.write(OpCode::Return.into(), 0..0);
    disassemble_chunk(&chunk, "test chunk")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_returns_disassembly() {
        let output = run();
        assert_eq!(
            output,
            "== test chunk ==\n0000 0..0 OP_CONSTANT         0 '1.2'\n0002    | OP_RETURN\n"
        );
    }
}
