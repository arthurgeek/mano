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
    /// Negate the top value on the stack.
    Negate = 3,
    /// Add top two values on the stack.
    Add = 4,
    /// Subtract top two values on the stack.
    Subtract = 5,
    /// Multiply top two values on the stack.
    Multiply = 6,
    /// Divide top two values on the stack.
    Divide = 7,
    /// Modulo top two values on the stack.
    Modulo = 8,
}

impl From<u8> for OpCode {
    fn from(byte: u8) -> Self {
        match byte {
            0 => OpCode::Return,
            1 => OpCode::Constant,
            2 => OpCode::ConstantLong,
            3 => OpCode::Negate,
            4 => OpCode::Add,
            5 => OpCode::Subtract,
            6 => OpCode::Multiply,
            7 => OpCode::Divide,
            8 => OpCode::Modulo,
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

    #[test]
    fn opcode_negate_has_value_three() {
        assert_eq!(OpCode::Negate as u8, 3);
    }

    #[test]
    fn opcode_from_byte_three_is_negate() {
        assert_eq!(OpCode::from(3), OpCode::Negate);
    }

    #[test]
    fn opcode_add_has_value_four() {
        assert_eq!(OpCode::Add as u8, 4);
    }

    #[test]
    fn opcode_subtract_has_value_five() {
        assert_eq!(OpCode::Subtract as u8, 5);
    }

    #[test]
    fn opcode_multiply_has_value_six() {
        assert_eq!(OpCode::Multiply as u8, 6);
    }

    #[test]
    fn opcode_divide_has_value_seven() {
        assert_eq!(OpCode::Divide as u8, 7);
    }

    #[test]
    fn opcode_modulo_has_value_eight() {
        assert_eq!(OpCode::Modulo as u8, 8);
    }

    #[test]
    fn opcode_from_byte_four_is_add() {
        assert_eq!(OpCode::from(4), OpCode::Add);
    }

    #[test]
    fn opcode_from_byte_five_is_subtract() {
        assert_eq!(OpCode::from(5), OpCode::Subtract);
    }

    #[test]
    fn opcode_from_byte_six_is_multiply() {
        assert_eq!(OpCode::from(6), OpCode::Multiply);
    }

    #[test]
    fn opcode_from_byte_seven_is_divide() {
        assert_eq!(OpCode::from(7), OpCode::Divide);
    }

    #[test]
    fn opcode_from_byte_eight_is_modulo() {
        assert_eq!(OpCode::from(8), OpCode::Modulo);
    }
}
