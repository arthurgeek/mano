use std::ops::Range;

use crate::value::Value;

/// Byte span in source code.
pub type Span = Range<usize>;

/// A chunk of bytecode.
#[derive(Default, Debug)]
pub struct Chunk {
    pub(crate) code: Vec<u8>,
    pub(crate) constants: Vec<Value>,
    /// RLE-compressed spans: (span, count)
    pub(crate) spans: Vec<(Span, usize)>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            spans: Vec::new(),
        }
    }

    pub fn write(&mut self, byte: u8, span: Span) {
        self.code.push(byte);
        if let Some((last_span, count)) = self.spans.last_mut()
            && *last_span == span
        {
            *count += 1;
            return;
        }
        self.spans.push((span, 1));
    }

    pub fn get_span(&self, offset: usize) -> Span {
        let mut remaining = offset;
        for (span, count) in &self.spans {
            if remaining < *count {
                return span.clone();
            }
            remaining -= count;
        }
        panic!("Offset {} out of bounds", offset);
    }

    pub fn add_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }

    pub fn write_constant(&mut self, value: Value, span: Span) {
        let index = self.add_constant(value);
        use crate::OpCode;
        if index < 256 {
            self.write(OpCode::Constant.into(), span.clone());
            self.write(index as u8, span);
        } else {
            self.write(OpCode::ConstantLong.into(), span.clone());
            // 24-bit little-endian
            self.write((index & 0xFF) as u8, span.clone());
            self.write(((index >> 8) & 0xFF) as u8, span.clone());
            self.write(((index >> 16) & 0xFF) as u8, span);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_new_is_empty() {
        let chunk = Chunk::new();
        assert!(chunk.code.is_empty());
    }

    #[test]
    fn chunk_write_adds_byte() {
        let mut chunk = Chunk::new();
        chunk.write(0x42, 0..0);
        assert_eq!(chunk.code.len(), 1);
        assert_eq!(chunk.code[0], 0x42);
    }

    #[test]
    fn chunk_add_constant_returns_index() {
        let mut chunk = Chunk::new();
        let idx = chunk.add_constant(1.2);
        assert_eq!(idx, 0);
    }

    #[test]
    fn chunk_add_constant_stores_value() {
        let mut chunk = Chunk::new();
        chunk.add_constant(1.2);
        assert_eq!(chunk.constants[0], 1.2);
    }

    #[test]
    fn chunk_add_multiple_constants() {
        let mut chunk = Chunk::new();
        assert_eq!(chunk.add_constant(1.0), 0);
        assert_eq!(chunk.add_constant(2.0), 1);
        assert_eq!(chunk.add_constant(3.0), 2);
    }

    #[test]
    fn chunk_write_tracks_span() {
        let mut chunk = Chunk::new();
        chunk.write(0x00, 10..15);
        assert_eq!(chunk.get_span(0), 10..15);
    }

    #[test]
    fn chunk_write_multiple_spans() {
        let mut chunk = Chunk::new();
        chunk.write(0x00, 0..5);
        chunk.write(0x01, 0..5);
        chunk.write(0x02, 10..20);
        assert_eq!(chunk.get_span(0), 0..5);
        assert_eq!(chunk.get_span(1), 0..5);
        assert_eq!(chunk.get_span(2), 10..20);
    }

    #[test]
    fn chunk_spans_are_rle_compressed() {
        let mut chunk = Chunk::new();
        chunk.write(0x00, 0..5);
        chunk.write(0x01, 0..5);
        chunk.write(0x02, 0..5);
        // 3 bytes but only 1 RLE entry
        assert_eq!(chunk.spans.len(), 1);
        assert_eq!(chunk.spans[0], (0..5, 3));
    }

    #[test]
    fn chunk_spans_rle_multiple_runs() {
        let mut chunk = Chunk::new();
        chunk.write(0x00, 0..5);
        chunk.write(0x01, 0..5);
        chunk.write(0x02, 10..15);
        chunk.write(0x03, 10..15);
        chunk.write(0x04, 10..15);
        // 5 bytes but only 2 RLE entries
        assert_eq!(chunk.spans.len(), 2);
        assert_eq!(chunk.spans[0], (0..5, 2));
        assert_eq!(chunk.spans[1], (10..15, 3));
    }

    #[test]
    fn chunk_get_span_decodes_rle() {
        let mut chunk = Chunk::new();
        chunk.write(0x00, 0..5);
        chunk.write(0x01, 0..5);
        chunk.write(0x02, 10..15);
        chunk.write(0x03, 10..15);
        chunk.write(0x04, 20..25);

        assert_eq!(chunk.get_span(0), 0..5);
        assert_eq!(chunk.get_span(1), 0..5);
        assert_eq!(chunk.get_span(2), 10..15);
        assert_eq!(chunk.get_span(3), 10..15);
        assert_eq!(chunk.get_span(4), 20..25);
    }

    #[test]
    #[should_panic(expected = "Offset 5 out of bounds")]
    fn chunk_get_span_panics_on_out_of_bounds() {
        let mut chunk = Chunk::new();
        chunk.write(0x00, 0..5);
        chunk.write(0x01, 0..5);
        chunk.get_span(5); // only 2 bytes written
    }

    #[test]
    fn chunk_write_constant_uses_short_opcode() {
        use crate::OpCode;
        let mut chunk = Chunk::new();
        chunk.write_constant(1.2, 0..5);
        assert_eq!(chunk.code[0], OpCode::Constant as u8);
        assert_eq!(chunk.code[1], 0); // index 0
    }

    #[test]
    fn chunk_write_constant_uses_long_opcode_after_256() {
        use crate::OpCode;
        let mut chunk = Chunk::new();
        // Add 256 constants to fill up the short range
        for i in 0..256 {
            chunk.add_constant(i as f64);
        }
        // Now write constant 256 - should use long opcode
        chunk.write_constant(256.0, 0..5);
        assert_eq!(chunk.code[0], OpCode::ConstantLong as u8);
        // 24-bit little-endian: 256 = 0x000100
        assert_eq!(chunk.code[1], 0); // low byte
        assert_eq!(chunk.code[2], 1); // middle byte
        assert_eq!(chunk.code[3], 0); // high byte
    }
}
