use crate::{Chunk, OpCode};

pub fn disassemble_chunk(chunk: &Chunk, name: &str) -> String {
    let mut output = format!("== {} ==\n", name);
    let mut offset = 0;
    while offset < chunk.code.len() {
        let (line, next_offset) = disassemble_instruction(chunk, offset);
        output.push_str(&line);
        offset = next_offset;
    }
    output
}

pub fn disassemble_instruction(chunk: &Chunk, offset: usize) -> (String, usize) {
    let span = chunk.get_span(offset);
    let span_str = if offset > 0 && chunk.get_span(offset - 1) == span {
        "   |".to_string()
    } else {
        format!("{}..{}", span.start, span.end)
    };

    let byte = chunk.code[offset];
    match byte {
        b if b == OpCode::Return as u8 => (
            format!("{:04} {} OP_RETURN\n", offset, span_str),
            offset + 1,
        ),
        b if b == OpCode::Negate as u8 => (
            format!("{:04} {} OP_NEGATE\n", offset, span_str),
            offset + 1,
        ),
        b if b == OpCode::Add as u8 => (format!("{:04} {} OP_ADD\n", offset, span_str), offset + 1),
        b if b == OpCode::Subtract as u8 => (
            format!("{:04} {} OP_SUBTRACT\n", offset, span_str),
            offset + 1,
        ),
        b if b == OpCode::Multiply as u8 => (
            format!("{:04} {} OP_MULTIPLY\n", offset, span_str),
            offset + 1,
        ),
        b if b == OpCode::Divide as u8 => (
            format!("{:04} {} OP_DIVIDE\n", offset, span_str),
            offset + 1,
        ),
        b if b == OpCode::Modulo as u8 => (
            format!("{:04} {} OP_MODULO\n", offset, span_str),
            offset + 1,
        ),
        b if b == OpCode::Constant as u8 => {
            let constant_idx = chunk.code[offset + 1];
            let value = chunk.constants[constant_idx as usize];
            (
                format!(
                    "{:04} {} OP_CONSTANT {:>9} '{}'\n",
                    offset, span_str, constant_idx, value
                ),
                offset + 2,
            )
        }
        b if b == OpCode::ConstantLong as u8 => {
            let constant_idx = chunk.code[offset + 1] as usize
                | (chunk.code[offset + 2] as usize) << 8
                | (chunk.code[offset + 3] as usize) << 16;
            let value = chunk.constants[constant_idx];
            (
                format!(
                    "{:04} {} OP_CONSTANT_LONG {:>9} '{}'\n",
                    offset, span_str, constant_idx, value
                ),
                offset + 4,
            )
        }
        _ => (
            format!("{:04} {} Unknown opcode {}\n", offset, span_str, byte),
            offset + 1,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OpCode;

    #[test]
    fn disassemble_empty_chunk() {
        let chunk = Chunk::new();
        let output = disassemble_chunk(&chunk, "test chunk");
        assert_eq!(output, "== test chunk ==\n");
    }

    #[test]
    fn disassemble_instruction_return() {
        let mut chunk = Chunk::new();
        chunk.write(OpCode::Return.into(), 0..0);

        let (output, next_offset) = disassemble_instruction(&chunk, 0);

        assert_eq!(output, "0000 0..0 OP_RETURN\n");
        assert_eq!(next_offset, 1);
    }

    #[test]
    fn disassemble_instruction_unknown_opcode() {
        let mut chunk = Chunk::new();
        chunk.write(0xFF, 0..0);

        let (output, next_offset) = disassemble_instruction(&chunk, 0);

        assert_eq!(output, "0000 0..0 Unknown opcode 255\n");
        assert_eq!(next_offset, 1);
    }

    #[test]
    fn disassemble_chunk_with_return() {
        let mut chunk = Chunk::new();
        chunk.write(OpCode::Return.into(), 0..0);

        let output = disassemble_chunk(&chunk, "test");

        assert_eq!(output, "== test ==\n0000 0..0 OP_RETURN\n");
    }

    #[test]
    fn disassemble_instruction_constant() {
        let mut chunk = Chunk::new();
        let constant_idx = chunk.add_constant(1.2);
        chunk.write(OpCode::Constant.into(), 0..0);
        chunk.write(constant_idx as u8, 0..0);

        let (output, next_offset) = disassemble_instruction(&chunk, 0);

        assert_eq!(output, "0000 0..0 OP_CONSTANT         0 '1.2'\n");
        assert_eq!(next_offset, 2);
    }

    #[test]
    fn disassemble_shows_span_for_first_instruction() {
        let mut chunk = Chunk::new();
        chunk.write(OpCode::Return.into(), 10..15);

        let (output, _) = disassemble_instruction(&chunk, 0);

        assert_eq!(output, "0000 10..15 OP_RETURN\n");
    }

    #[test]
    fn disassemble_shows_pipe_for_same_span() {
        let mut chunk = Chunk::new();
        chunk.write(OpCode::Return.into(), 10..15);
        chunk.write(OpCode::Return.into(), 10..15);

        let (output, _) = disassemble_instruction(&chunk, 1);

        assert_eq!(output, "0001    | OP_RETURN\n");
    }

    #[test]
    fn disassemble_shows_new_span_when_different() {
        let mut chunk = Chunk::new();
        chunk.write(OpCode::Return.into(), 10..15);
        chunk.write(OpCode::Return.into(), 20..25);

        let (output, _) = disassemble_instruction(&chunk, 1);

        assert_eq!(output, "0001 20..25 OP_RETURN\n");
    }

    #[test]
    fn disassemble_instruction_constant_long() {
        let mut chunk = Chunk::new();
        // Add 256 constants
        for i in 0..256 {
            chunk.add_constant(i as f64);
        }
        // Write constant at index 256
        chunk.write_constant(999.0, 0..0);

        let (output, next_offset) = disassemble_instruction(&chunk, 0);

        assert_eq!(output, "0000 0..0 OP_CONSTANT_LONG       256 '999'\n");
        assert_eq!(next_offset, 4); // opcode + 3 bytes
    }

    #[test]
    fn disassemble_instruction_negate() {
        let mut chunk = Chunk::new();
        chunk.write(OpCode::Negate.into(), 0..0);

        let (output, next_offset) = disassemble_instruction(&chunk, 0);

        assert_eq!(output, "0000 0..0 OP_NEGATE\n");
        assert_eq!(next_offset, 1);
    }

    #[test]
    fn disassemble_instruction_add() {
        let mut chunk = Chunk::new();
        chunk.write(OpCode::Add.into(), 0..0);

        let (output, next_offset) = disassemble_instruction(&chunk, 0);

        assert_eq!(output, "0000 0..0 OP_ADD\n");
        assert_eq!(next_offset, 1);
    }

    #[test]
    fn disassemble_instruction_subtract() {
        let mut chunk = Chunk::new();
        chunk.write(OpCode::Subtract.into(), 0..0);

        let (output, next_offset) = disassemble_instruction(&chunk, 0);

        assert_eq!(output, "0000 0..0 OP_SUBTRACT\n");
        assert_eq!(next_offset, 1);
    }

    #[test]
    fn disassemble_instruction_multiply() {
        let mut chunk = Chunk::new();
        chunk.write(OpCode::Multiply.into(), 0..0);

        let (output, next_offset) = disassemble_instruction(&chunk, 0);

        assert_eq!(output, "0000 0..0 OP_MULTIPLY\n");
        assert_eq!(next_offset, 1);
    }

    #[test]
    fn disassemble_instruction_divide() {
        let mut chunk = Chunk::new();
        chunk.write(OpCode::Divide.into(), 0..0);

        let (output, next_offset) = disassemble_instruction(&chunk, 0);

        assert_eq!(output, "0000 0..0 OP_DIVIDE\n");
        assert_eq!(next_offset, 1);
    }

    #[test]
    fn disassemble_instruction_modulo() {
        let mut chunk = Chunk::new();
        chunk.write(OpCode::Modulo.into(), 0..0);

        let (output, next_offset) = disassemble_instruction(&chunk, 0);

        assert_eq!(output, "0000 0..0 OP_MODULO\n");
        assert_eq!(next_offset, 1);
    }
}
