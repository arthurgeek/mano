use mano::KEYWORDS;

pub struct ReplState {
    buffer: String,
    brace_depth: usize,
}

impl ReplState {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            brace_depth: 0,
        }
    }

    pub fn prompt(&self) -> String {
        if self.brace_depth == 0 {
            "> ".to_string()
        } else {
            format!("..{} ", self.brace_depth)
        }
    }

    /// Returns true if ready to execute (braces balanced)
    pub fn process_line(&mut self, line: &str) -> bool {
        for ch in line.chars() {
            match ch {
                '{' => self.brace_depth += 1,
                '}' => self.brace_depth = self.brace_depth.saturating_sub(1),
                _ => {}
            }
        }

        self.buffer.push_str(line);
        self.buffer.push('\n');

        self.brace_depth == 0
    }

    pub fn take_buffer(&mut self) -> String {
        self.brace_depth = 0;
        std::mem::take(&mut self.buffer)
    }

    pub fn cancel(&mut self) {
        self.buffer.clear();
        self.brace_depth = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Check if input should be auto-printed (expression without semicolon)
    pub fn should_auto_print(input: &str) -> bool {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return false;
        }
        // Line comments are not auto-printed
        if trimmed.starts_with("//") {
            return false;
        }
        // Block comments are not auto-printed
        if trimmed.starts_with("/*") && trimmed.ends_with("*/") {
            return false;
        }

        // Strip trailing line comment for further checks
        let code = if let Some(idx) = trimmed.find("//") {
            trimmed[..idx].trim()
        } else {
            trimmed
        };

        // Strip trailing block comment
        let code = if let Some(start) = code.rfind("/*") {
            if code.ends_with("*/") {
                code[..start].trim()
            } else {
                code
            }
        } else {
            code
        };

        if code.is_empty() {
            return false;
        }

        // Blocks are not auto-printed
        if code.ends_with('}') {
            return false;
        }
        // If it ends with semicolon, it's a statement
        if code.ends_with(';') {
            return false;
        }

        // Don't auto-print if it starts with a keyword (incomplete statement)
        // Let the parser give proper error message
        for (keyword, _) in KEYWORDS {
            if let Some(after_keyword) = code.strip_prefix(keyword) {
                // Make sure it's followed by a space or end of string (not just a prefix match)
                if after_keyword.is_empty() || after_keyword.starts_with(' ') {
                    return false;
                }
            }
        }

        true
    }

    /// Wrap input in a print statement for auto-printing
    pub fn wrap_for_print(input: &str) -> String {
        format!("salve {};", input.trim())
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_empty_state() {
        let state = ReplState::new();
        assert!(state.is_empty());
        assert_eq!(state.brace_depth, 0);
    }

    #[test]
    fn prompt_returns_normal_when_not_in_block() {
        let state = ReplState::new();
        assert_eq!(state.prompt(), "> ");
    }

    #[test]
    fn prompt_shows_depth_when_in_block() {
        let mut state = ReplState::new();
        state.process_line("{");
        assert_eq!(state.prompt(), "..1 ");

        state.process_line("{");
        assert_eq!(state.prompt(), "..2 ");
    }

    #[test]
    fn process_line_ready_when_braces_balanced() {
        let mut state = ReplState::new();
        assert!(state.process_line("salve 1;"));
    }

    #[test]
    fn process_line_not_ready_when_braces_unbalanced() {
        let mut state = ReplState::new();
        assert!(!state.process_line("{"));
        assert!(!state.process_line("salve 1;"));
    }

    #[test]
    fn process_line_ready_when_block_closes() {
        let mut state = ReplState::new();
        state.process_line("{");
        state.process_line("salve 1;");
        assert!(state.process_line("}"));
    }

    #[test]
    fn process_line_handles_nested_blocks() {
        let mut state = ReplState::new();
        state.process_line("{");
        assert!(!state.process_line("{"));
        assert!(!state.process_line("}"));
        assert!(state.process_line("}"));
    }

    #[test]
    fn take_buffer_returns_accumulated_lines() {
        let mut state = ReplState::new();
        state.process_line("{");
        state.process_line("salve 1;");
        state.process_line("}");

        let buffer = state.take_buffer();
        assert!(buffer.contains("{"));
        assert!(buffer.contains("salve 1;"));
        assert!(buffer.contains("}"));
    }

    #[test]
    fn take_buffer_clears_state() {
        let mut state = ReplState::new();
        state.process_line("salve 1;");
        state.take_buffer();
        assert!(state.is_empty());
    }

    #[test]
    fn cancel_clears_buffer_and_depth() {
        let mut state = ReplState::new();
        state.process_line("{");
        state.process_line("salve 1;");
        state.cancel();

        assert!(state.is_empty());
        assert_eq!(state.brace_depth, 0);
        assert_eq!(state.prompt(), "> ");
    }

    #[test]
    fn is_empty_false_when_has_content() {
        let mut state = ReplState::new();
        state.process_line("salve 1;");
        assert!(!state.is_empty());
    }

    #[test]
    fn handles_unmatched_closing_brace() {
        let mut state = ReplState::new();
        assert!(state.process_line("}"));
        assert_eq!(state.brace_depth, 0);
    }

    #[test]
    fn should_auto_print_expression_without_semicolon() {
        assert!(ReplState::should_auto_print("1 + 2"));
        assert!(ReplState::should_auto_print("\"mano\""));
        assert!(ReplState::should_auto_print("x"));
    }

    #[test]
    fn should_not_auto_print_statement_with_semicolon() {
        assert!(!ReplState::should_auto_print("salve 1;"));
        assert!(!ReplState::should_auto_print("1 + 2;"));
        assert!(!ReplState::should_auto_print("seLiga x = 1;"));
    }

    #[test]
    fn should_not_auto_print_blocks() {
        assert!(!ReplState::should_auto_print("{ salve 1; }"));
        assert!(!ReplState::should_auto_print("{\n}"));
    }

    #[test]
    fn should_not_auto_print_empty_or_whitespace() {
        assert!(!ReplState::should_auto_print(""));
        assert!(!ReplState::should_auto_print("   "));
        assert!(!ReplState::should_auto_print("\n"));
    }

    #[test]
    fn should_not_auto_print_comments() {
        assert!(!ReplState::should_auto_print("// comentário"));
        assert!(!ReplState::should_auto_print("  // indented comment"));
        assert!(!ReplState::should_auto_print("/* block comment */"));
        assert!(!ReplState::should_auto_print("  /* indented */"));
        assert!(!ReplState::should_auto_print("/* multi\nline\ncomment */"));
    }

    #[test]
    fn should_not_auto_print_code_with_trailing_comment() {
        // Code with trailing comment should respect the code, not the comment
        assert!(!ReplState::should_auto_print("salve \"mundo\"; // print"));
        assert!(!ReplState::should_auto_print("seLiga x = 1; /* inline */"));
    }

    #[test]
    fn should_auto_print_expression_with_unclosed_block_comment() {
        // Unclosed block comment shouldn't prevent auto-print of expression
        assert!(ReplState::should_auto_print("1 + 2 /* unclosed"));
    }

    #[test]
    fn should_not_auto_print_mixed_comments_only() {
        // Block comment followed by line comment = no code
        assert!(!ReplState::should_auto_print("/* a */ // b"));
    }

    #[test]
    fn should_not_auto_print_statements_missing_semicolon() {
        // Incomplete statements should not be wrapped - let parser error properly
        assert!(!ReplState::should_auto_print("salve a"));
        assert!(!ReplState::should_auto_print("oiSumida a"));
        assert!(!ReplState::should_auto_print("seLiga x = 1"));
        assert!(!ReplState::should_auto_print("olhaEssaFita foo() { }"));
        assert!(!ReplState::should_auto_print("bagulho Pessoa {}"));
        assert!(!ReplState::should_auto_print("sePá x"));
        assert!(!ReplState::should_auto_print("segueOFluxo x"));
        assert!(!ReplState::should_auto_print("seVira x"));
        assert!(!ReplState::should_auto_print("toma x"));
    }

    #[test]
    fn wrap_for_print_adds_salve() {
        assert_eq!(ReplState::wrap_for_print("a"), "salve a;");
        assert_eq!(ReplState::wrap_for_print("\"mano\""), "salve \"mano\";");
    }
}
