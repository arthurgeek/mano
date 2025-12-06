use std::io::Write;

use crate::{Chunk, OpCode, disassemble_instruction};

use mano::ManoError;

pub type InterpretResult = Result<(), Vec<ManoError>>;

pub struct VM<'a, W: Write> {
    chunk: &'a Chunk,
    ip: usize,
    output: &'a mut W,
    trace: bool,
    stack: Vec<f64>,
}

impl<'a, W: Write> VM<'a, W> {
    pub fn new(chunk: &'a Chunk, output: &'a mut W) -> Self {
        Self {
            chunk,
            ip: 0,
            output,
            trace: false,
            stack: Vec::new(),
        }
    }

    pub fn set_trace(&mut self, trace: bool) {
        self.trace = trace;
    }

    pub fn push(&mut self, value: f64) {
        self.stack.push(value);
    }

    pub fn pop(&mut self) -> f64 {
        self.stack.pop().expect("Stack underflow")
    }

    pub fn trace_stack(&mut self) {
        write!(self.output, "          ").unwrap();
        for value in &self.stack {
            write!(self.output, "[ {value} ]").unwrap();
        }
        writeln!(self.output).unwrap();
    }

    pub fn interpret(&mut self) -> InterpretResult {
        if self.trace {
            writeln!(self.output, "== trace ==").unwrap();
        }
        self.run()
    }

    fn run(&mut self) -> InterpretResult {
        loop {
            if self.trace {
                self.trace_stack();
                let (line, _) = disassemble_instruction(self.chunk, self.ip);
                write!(self.output, "{line}").unwrap();
            }
            let byte = self.read_byte();
            match byte {
                b if b == OpCode::Constant as u8 => {
                    let constant = self.read_constant();
                    self.push(constant);
                }
                b if b == OpCode::ConstantLong as u8 => {
                    let constant = self.read_constant_long();
                    self.push(constant);
                }
                b if b == OpCode::Negate as u8 => {
                    let value = self.pop();
                    self.push(-value);
                }
                b if b == OpCode::Add as u8 => {
                    let b = self.pop();
                    let a = self.pop();
                    self.push(a + b);
                }
                b if b == OpCode::Subtract as u8 => {
                    let b = self.pop();
                    let a = self.pop();
                    self.push(a - b);
                }
                b if b == OpCode::Multiply as u8 => {
                    let b = self.pop();
                    let a = self.pop();
                    self.push(a * b);
                }
                b if b == OpCode::Divide as u8 => {
                    let b = self.pop();
                    let a = self.pop();
                    self.push(a / b);
                }
                b if b == OpCode::Modulo as u8 => {
                    let b = self.pop();
                    let a = self.pop();
                    self.push(a % b);
                }
                b if b == OpCode::Return as u8 => {
                    let value = self.pop();
                    writeln!(self.output, "{value}").unwrap();
                    return Ok(());
                }
                _ => unreachable!("Unknown opcode: {}", byte),
            }
        }
    }

    fn read_byte(&mut self) -> u8 {
        let byte = self.chunk.code[self.ip];
        self.ip += 1;
        byte
    }

    fn read_constant(&mut self) -> f64 {
        let index = self.read_byte() as usize;
        self.chunk.constants[index]
    }

    fn read_constant_long(&mut self) -> f64 {
        let b0 = self.read_byte() as usize;
        let b1 = self.read_byte() as usize;
        let b2 = self.read_byte() as usize;
        let index = b0 | (b1 << 8) | (b2 << 16);
        self.chunk.constants[index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vm_new_creates_vm_with_chunk() {
        let chunk = Chunk::new();
        let mut output = Vec::new();
        let vm = VM::new(&chunk, &mut output);
        assert_eq!(vm.ip, 0);
    }

    #[test]
    fn vm_interpret_returns_ok() {
        let mut chunk = Chunk::new();
        chunk.write_constant(0.0, 0..0);
        chunk.write(crate::OpCode::Return.into(), 0..0);
        let mut output = Vec::new();
        let mut vm = VM::new(&chunk, &mut output);
        assert!(vm.interpret().is_ok());
    }

    #[test]
    fn vm_return_pops_and_prints() {
        let mut chunk = Chunk::new();
        chunk.write_constant(1.2, 0..0);
        chunk.write(crate::OpCode::Return.into(), 0..0);
        let mut output = Vec::new();
        let mut vm = VM::new(&chunk, &mut output);
        assert!(vm.interpret().is_ok());
        assert_eq!(String::from_utf8(output).unwrap(), "1.2\n");
    }

    #[test]
    fn vm_return_pops_and_prints_long_constant() {
        let mut chunk = Chunk::new();
        // Fill up 256 constants to force OP_CONSTANT_LONG
        for i in 0..256 {
            chunk.add_constant(i as f64);
        }
        chunk.write_constant(999.0, 0..0);
        chunk.write(crate::OpCode::Return.into(), 0..0);
        let mut output = Vec::new();
        let mut vm = VM::new(&chunk, &mut output);
        assert!(vm.interpret().is_ok());
        assert_eq!(String::from_utf8(output).unwrap(), "999\n");
    }

    #[test]
    fn vm_trace_prints_instructions() {
        let mut chunk = Chunk::new();
        chunk.write_constant(1.2, 0..0);
        chunk.write(crate::OpCode::Return.into(), 0..0);
        let mut output = Vec::new();
        let mut vm = VM::new(&chunk, &mut output);
        vm.set_trace(true);
        let _ = vm.interpret();
        let out = String::from_utf8(output).unwrap();
        assert!(out.contains("OP_CONSTANT"));
        assert!(out.contains("OP_RETURN"));
    }

    #[test]
    fn vm_trace_prints_header() {
        let mut chunk = Chunk::new();
        chunk.write_constant(1.2, 0..0);
        chunk.write(crate::OpCode::Return.into(), 0..0);
        let mut output = Vec::new();
        let mut vm = VM::new(&chunk, &mut output);
        vm.set_trace(true);
        let _ = vm.interpret();
        let out = String::from_utf8(output).unwrap();
        assert!(out.starts_with("== trace ==\n"));
    }

    #[test]
    fn vm_push_and_pop() {
        let chunk = Chunk::new();
        let mut output = Vec::new();
        let mut vm = VM::new(&chunk, &mut output);
        vm.push(1.2);
        vm.push(3.4);
        assert_eq!(vm.pop(), 3.4);
        assert_eq!(vm.pop(), 1.2);
    }

    #[test]
    #[should_panic(expected = "Stack underflow")]
    fn vm_pop_empty_stack_panics() {
        let chunk = Chunk::new();
        let mut output = Vec::new();
        let mut vm = VM::new(&chunk, &mut output);
        vm.pop();
    }

    #[test]
    fn vm_trace_shows_stack() {
        let chunk = Chunk::new();
        let mut output = Vec::new();
        let mut vm = VM::new(&chunk, &mut output);
        vm.set_trace(true);
        vm.push(1.2);
        vm.push(3.4);
        vm.trace_stack();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "          [ 1.2 ][ 3.4 ]\n"
        );
    }

    #[test]
    fn vm_constant_pushes_to_stack() {
        let mut chunk = Chunk::new();
        chunk.write_constant(1.2, 0..0);
        chunk.write_constant(3.4, 0..0);
        chunk.write(crate::OpCode::Return.into(), 0..0);
        let mut output = Vec::new();
        let mut vm = VM::new(&chunk, &mut output);
        let _ = vm.interpret();
        // Return popped 3.4, 1.2 remains
        assert_eq!(vm.stack, vec![1.2]);
    }

    #[test]
    fn vm_constant_long_pushes_to_stack() {
        let mut chunk = Chunk::new();
        for i in 0..256 {
            chunk.add_constant(i as f64);
        }
        chunk.write_constant(888.0, 0..0);
        chunk.write_constant(999.0, 0..0);
        chunk.write(crate::OpCode::Return.into(), 0..0);
        let mut output = Vec::new();
        let mut vm = VM::new(&chunk, &mut output);
        let _ = vm.interpret();
        // Return popped 999.0, 888.0 remains
        assert_eq!(vm.stack, vec![888.0]);
    }

    #[test]
    fn vm_run_traces_stack_before_instruction() {
        let mut chunk = Chunk::new();
        chunk.write_constant(1.2, 0..0);
        chunk.write(crate::OpCode::Return.into(), 0..0);
        let mut output = Vec::new();
        let mut vm = VM::new(&chunk, &mut output);
        vm.set_trace(true);
        let _ = vm.interpret();
        let out = String::from_utf8(output).unwrap();
        // First instruction: empty stack, then OP_CONSTANT
        // Second instruction: stack has 1.2, then OP_RETURN
        assert!(out.contains("          \n")); // empty stack before first
        assert!(out.contains("[ 1.2 ]")); // stack before return
    }

    #[test]
    fn vm_negate_negates_top_of_stack() {
        let mut chunk = Chunk::new();
        chunk.write_constant(3.4, 0..0);
        chunk.write(crate::OpCode::Negate.into(), 0..0);
        chunk.write(crate::OpCode::Return.into(), 0..0);
        let mut output = Vec::new();
        let mut vm = VM::new(&chunk, &mut output);
        let _ = vm.interpret();
        assert_eq!(String::from_utf8(output).unwrap(), "-3.4\n");
    }

    #[test]
    fn vm_add_adds_top_two_values() {
        let mut chunk = Chunk::new();
        chunk.write_constant(1.2, 0..0);
        chunk.write_constant(3.4, 0..0);
        chunk.write(crate::OpCode::Add.into(), 0..0);
        chunk.write(crate::OpCode::Return.into(), 0..0);
        let mut output = Vec::new();
        let mut vm = VM::new(&chunk, &mut output);
        let _ = vm.interpret();
        assert_eq!(String::from_utf8(output).unwrap(), "4.6\n");
    }

    #[test]
    fn vm_subtract_subtracts_top_two_values() {
        let mut chunk = Chunk::new();
        chunk.write_constant(5.0, 0..0);
        chunk.write_constant(3.0, 0..0);
        chunk.write(crate::OpCode::Subtract.into(), 0..0);
        chunk.write(crate::OpCode::Return.into(), 0..0);
        let mut output = Vec::new();
        let mut vm = VM::new(&chunk, &mut output);
        let _ = vm.interpret();
        assert_eq!(String::from_utf8(output).unwrap(), "2\n");
    }

    #[test]
    fn vm_multiply_multiplies_top_two_values() {
        let mut chunk = Chunk::new();
        chunk.write_constant(3.0, 0..0);
        chunk.write_constant(4.0, 0..0);
        chunk.write(crate::OpCode::Multiply.into(), 0..0);
        chunk.write(crate::OpCode::Return.into(), 0..0);
        let mut output = Vec::new();
        let mut vm = VM::new(&chunk, &mut output);
        let _ = vm.interpret();
        assert_eq!(String::from_utf8(output).unwrap(), "12\n");
    }

    #[test]
    fn vm_divide_divides_top_two_values() {
        let mut chunk = Chunk::new();
        chunk.write_constant(10.0, 0..0);
        chunk.write_constant(4.0, 0..0);
        chunk.write(crate::OpCode::Divide.into(), 0..0);
        chunk.write(crate::OpCode::Return.into(), 0..0);
        let mut output = Vec::new();
        let mut vm = VM::new(&chunk, &mut output);
        let _ = vm.interpret();
        assert_eq!(String::from_utf8(output).unwrap(), "2.5\n");
    }

    #[test]
    fn vm_modulo_computes_remainder() {
        let mut chunk = Chunk::new();
        chunk.write_constant(10.0, 0..0);
        chunk.write_constant(3.0, 0..0);
        chunk.write(crate::OpCode::Modulo.into(), 0..0);
        chunk.write(crate::OpCode::Return.into(), 0..0);
        let mut output = Vec::new();
        let mut vm = VM::new(&chunk, &mut output);
        let _ = vm.interpret();
        assert_eq!(String::from_utf8(output).unwrap(), "1\n");
    }
}
