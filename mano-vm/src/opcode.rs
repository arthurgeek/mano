/// Bytecode opcodes for the mano VM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OpCode {
    /// Return from the current function.
    Return = 0,
    /// Load a constant from the constant pool (1-byte index).
    Constant = 1,
    /// Load a constant from the constant pool (24-bit index).
    ConstantLong = 2,
}

impl From<u8> for OpCode {
    fn from(byte: u8) -> Self {
        match byte {
            0 => OpCode::Return,
            1 => OpCode::Constant,
            2 => OpCode::ConstantLong,
            _ => panic!("Unknown opcode: {}", byte),
        }
    }
}

impl From<OpCode> for u8 {
    fn from(op: OpCode) -> Self {
        op as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opcode_return_has_value_zero() {
        assert_eq!(OpCode::Return as u8, 0);
    }

    #[test]
    fn opcode_from_byte_zero_is_return() {
        assert_eq!(OpCode::from(0), OpCode::Return);
    }

    #[test]
    fn opcode_to_byte_return_is_zero() {
        assert_eq!(u8::from(OpCode::Return), 0);
    }

    #[test]
    #[should_panic(expected = "Unknown opcode: 255")]
    fn opcode_from_unknown_byte_panics() {
        let _ = OpCode::from(255);
    }

    #[test]
    fn opcode_constant_has_value_one() {
        assert_eq!(OpCode::Constant as u8, 1);
    }

    #[test]
    fn opcode_from_byte_one_is_constant() {
        assert_eq!(OpCode::from(1), OpCode::Constant);
    }

    #[test]
    fn opcode_constant_long_has_value_two() {
        assert_eq!(OpCode::ConstantLong as u8, 2);
    }

    #[test]
    fn opcode_from_byte_two_is_constant_long() {
        assert_eq!(OpCode::from(2), OpCode::ConstantLong);
    }
}
