//! Compiler - compiles source code to bytecode

use mano::{ManoError, Scanner, Token, TokenType};

use crate::Chunk;

/// Result type for compilation.
pub type CompileResult = Result<Chunk, Vec<ManoError>>;

/// Precedence levels from lowest to highest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
#[allow(dead_code)] // Variants used incrementally as we implement more operators
enum Precedence {
    None,
    Assignment, // =
    Or,         // ow
    And,        // tamoJunto
    Equality,   // == !=
    Comparison, // < > <= >=
    Term,       // + -
    Factor,     // * / %
    Unary,      // ! -
    Call,       // . ()
    Primary,
}

impl Precedence {
    fn next(self) -> Self {
        match self {
            Self::None => Self::Assignment,
            Self::Assignment => Self::Or,
            Self::Or => Self::And,
            Self::And => Self::Equality,
            Self::Equality => Self::Comparison,
            Self::Comparison => Self::Term,
            Self::Term => Self::Factor,
            Self::Factor => Self::Unary,
            Self::Unary => Self::Call,
            Self::Call => Self::Primary,
            Self::Primary => Self::Primary,
        }
    }
}

/// Parse function identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseFn {
    Grouping,
    Unary,
    Binary,
    Number,
    Ternary,
}

impl ParseFn {
    fn call(self, compiler: &mut Compiler) {
        match self {
            Self::Grouping => compiler.grouping(),
            Self::Unary => compiler.unary(),
            Self::Binary => compiler.binary(),
            Self::Number => compiler.number(),
            Self::Ternary => compiler.ternary(),
        }
    }
}

/// A parse rule mapping a token to its prefix/infix parsers and precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ParseRule {
    prefix: Option<ParseFn>,
    infix: Option<ParseFn>,
    precedence: Precedence,
}

impl From<TokenType> for ParseRule {
    fn from(token_type: TokenType) -> Self {
        match token_type {
            TokenType::LeftParen => Self {
                prefix: Some(ParseFn::Grouping),
                infix: None,
                precedence: Precedence::None,
            },
            TokenType::Minus => Self {
                prefix: Some(ParseFn::Unary),
                infix: Some(ParseFn::Binary),
                precedence: Precedence::Term,
            },
            TokenType::Plus => Self {
                prefix: None,
                infix: Some(ParseFn::Binary),
                precedence: Precedence::Term,
            },
            TokenType::Slash => Self {
                prefix: None,
                infix: Some(ParseFn::Binary),
                precedence: Precedence::Factor,
            },
            TokenType::Star => Self {
                prefix: None,
                infix: Some(ParseFn::Binary),
                precedence: Precedence::Factor,
            },
            TokenType::Percent => Self {
                prefix: None,
                infix: Some(ParseFn::Binary),
                precedence: Precedence::Factor,
            },
            TokenType::Question => Self {
                prefix: None,
                infix: Some(ParseFn::Ternary),
                precedence: Precedence::Assignment,
            },
            TokenType::Number => Self {
                prefix: Some(ParseFn::Number),
                infix: None,
                precedence: Precedence::None,
            },
            _ => Self {
                prefix: None,
                infix: None,
                precedence: Precedence::None,
            },
        }
    }
}

/// The compiler - holds parser state and emits bytecode.
struct Compiler<'a> {
    scanner: Scanner<'a>,
    current: Token,
    previous: Token,
    chunk: Chunk,
    errors: Vec<ManoError>,
}

impl<'a> Compiler<'a> {
    fn new(source: &'a str) -> Self {
        let scanner = Scanner::new(source);
        let placeholder = Token {
            token_type: TokenType::Eof,
            lexeme: String::new(),
            literal: None,
            span: 0..0,
        };
        Self {
            scanner,
            current: placeholder.clone(),
            previous: placeholder,
            chunk: Chunk::new(),
            errors: Vec::new(),
        }
    }

    fn advance(&mut self) {
        let eof_placeholder = Token {
            token_type: TokenType::Eof,
            lexeme: String::new(),
            literal: None,
            span: self.current.span.end..self.current.span.end,
        };
        self.previous = std::mem::replace(&mut self.current, eof_placeholder);

        loop {
            match self.scanner.next() {
                Some(Ok(token)) => {
                    self.current = token;
                    break;
                }
                Some(Err(error)) => {
                    self.errors.push(error);
                }
                None => {
                    // Already have EOF placeholder in current
                    break;
                }
            }
        }
    }

    fn error_at_current(&mut self, message: &str) {
        self.errors.push(ManoError::Parse {
            message: message.to_string(),
            span: self.current.span.clone(),
        });
    }

    fn consume(&mut self, expected: TokenType, message: &str) {
        if self.current.token_type == expected {
            self.advance();
        } else {
            self.error_at_current(message);
        }
    }

    fn emit_byte(&mut self, byte: u8) {
        let span = self.previous.span.clone();
        self.chunk.write(byte, span);
    }

    fn emit_return(&mut self) {
        self.emit_byte(crate::OpCode::Return as u8);
    }

    fn emit_constant(&mut self, value: f64) {
        let span = self.previous.span.clone();
        self.chunk.write_constant(value, span);
    }

    fn error_at_previous(&mut self, message: &str) {
        self.errors.push(ManoError::Parse {
            message: message.to_string(),
            span: self.previous.span.clone(),
        });
    }

    fn parse_precedence(&mut self, precedence: Precedence) {
        self.advance();

        // Prefix expression
        let rule = ParseRule::from(self.previous.token_type);
        match rule.prefix {
            Some(prefix_fn) => prefix_fn.call(self),
            None => {
                self.error_at_previous("Cadê a expressão, jão?");
                return;
            }
        }

        // Infix expressions
        while precedence <= ParseRule::from(self.current.token_type).precedence {
            self.advance();
            let infix_rule = ParseRule::from(self.previous.token_type);
            if let Some(infix_fn) = infix_rule.infix {
                infix_fn.call(self);
            }
        }
    }

    fn expression(&mut self) {
        self.parse_precedence(Precedence::Assignment);
    }

    fn grouping(&mut self) {
        self.expression();
        self.consume(TokenType::RightParen, "Cadê o ')', mano?");
    }

    fn unary(&mut self) {
        // Parse operand at unary precedence (binds tighter than binary ops)
        self.parse_precedence(Precedence::Unary);
        // Then emit the operator
        self.emit_byte(crate::OpCode::Negate as u8);
    }

    fn binary(&mut self) {
        let operator_type = self.previous.token_type;
        let rule = ParseRule::from(operator_type);

        // Parse right operand at one higher precedence (left-associative)
        self.parse_precedence(rule.precedence.next());

        // Emit operator instruction
        match operator_type {
            TokenType::Plus => self.emit_byte(crate::OpCode::Add as u8),
            TokenType::Minus => self.emit_byte(crate::OpCode::Subtract as u8),
            TokenType::Star => self.emit_byte(crate::OpCode::Multiply as u8),
            TokenType::Slash => self.emit_byte(crate::OpCode::Divide as u8),
            TokenType::Percent => self.emit_byte(crate::OpCode::Modulo as u8),
            _ => unreachable!("binary() called with non-binary operator"),
        }
    }

    fn number(&mut self) {
        if let Some(mano::Literal::Number(value)) = &self.previous.literal {
            self.emit_constant(*value);
        }
    }

    fn ternary(&mut self) {
        // Parse then branch
        self.expression();

        // Consume ':'
        self.consume(TokenType::Colon, "Cadê o ':' do ternário, chapa?");

        // Parse else branch (right-associative)
        self.parse_precedence(Precedence::Assignment);

        // For now, just error - we need jump instructions to implement this
        self.error_at_previous("Ternário ainda não suportado na VM, mano!");
    }
}

/// Compile source code into bytecode.
///
/// Returns `Ok(chunk)` on success, `Err(errors)` on failure.
pub fn compile(source: &str) -> CompileResult {
    let mut compiler = Compiler::new(source);

    compiler.advance();
    compiler.expression();
    compiler.consume(TokenType::Eof, "Expect end of expression.");
    compiler.emit_return();

    if compiler.errors.is_empty() {
        Ok(compiler.chunk)
    } else {
        Err(compiler.errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mano::TokenType;

    // 17.2.0 - Compiler struct
    #[test]
    fn compiler_new_has_empty_errors() {
        let compiler = Compiler::new("42");
        assert!(compiler.errors.is_empty());
    }

    // 17.2.1 - advance()
    #[test]
    fn advance_sets_current_to_first_token() {
        let mut compiler = Compiler::new("42");
        compiler.advance();
        assert_eq!(compiler.current.token_type, TokenType::Number);
    }

    #[test]
    fn advance_moves_current_to_previous() {
        let mut compiler = Compiler::new("42 + 3");
        compiler.advance(); // current = 42
        compiler.advance(); // previous = 42, current = +
        assert_eq!(compiler.previous.token_type, TokenType::Number);
        assert_eq!(compiler.current.token_type, TokenType::Plus);
    }

    #[test]
    fn advance_reaches_eof_on_empty_source() {
        let mut compiler = Compiler::new("");
        compiler.advance();
        assert_eq!(compiler.current.token_type, TokenType::Eof);
    }

    #[test]
    fn advance_reaches_eof_after_last_token() {
        let mut compiler = Compiler::new("42");
        compiler.advance(); // current = 42
        compiler.advance(); // current = EOF
        assert_eq!(compiler.current.token_type, TokenType::Eof);
    }

    #[test]
    fn advance_collects_scanner_errors() {
        let mut compiler = Compiler::new("@");
        compiler.advance();
        assert!(!compiler.errors.is_empty());
    }

    #[test]
    fn advance_skips_errors_and_continues() {
        let mut compiler = Compiler::new("42 @ 3");
        compiler.advance(); // current = 42
        compiler.advance(); // skips @, current = 3
        assert_eq!(compiler.current.token_type, TokenType::Number);
        assert_eq!(compiler.current.lexeme, "3");
    }

    // 17.2.1 - error reporting (error_at_current used by consume in 17.2.2)
    #[test]
    fn error_at_current_adds_to_errors_list() {
        let mut compiler = Compiler::new("42");
        compiler.advance();
        compiler.error_at_current("test error");
        assert_eq!(compiler.errors.len(), 1);
    }

    #[test]
    fn error_at_current_uses_current_token_span() {
        let mut compiler = Compiler::new("42 +");
        compiler.advance(); // current = 42
        compiler.advance(); // current = +
        let expected_span = compiler.current.span.clone();
        compiler.error_at_current("test error");
        match &compiler.errors[0] {
            ManoError::Parse { span, .. } => assert_eq!(*span, expected_span),
            _ => panic!("Expected Parse error"),
        }
    }

    // consume()
    #[test]
    fn consume_advances_on_match() {
        let mut compiler = Compiler::new("42 + 3");
        compiler.advance(); // current = 42
        compiler.consume(TokenType::Number, "Expected number");
        assert_eq!(compiler.previous.token_type, TokenType::Number);
        assert_eq!(compiler.current.token_type, TokenType::Plus);
    }

    #[test]
    fn consume_errors_on_mismatch() {
        let mut compiler = Compiler::new("42 + 3");
        compiler.advance(); // current = 42
        compiler.consume(TokenType::Plus, "Expected '+'");
        assert_eq!(compiler.errors.len(), 1);
    }

    #[test]
    fn consume_error_message_is_preserved() {
        let mut compiler = Compiler::new("42");
        compiler.advance();
        compiler.consume(TokenType::Plus, "Cadê o '+', mano?");
        match &compiler.errors[0] {
            ManoError::Parse { message, .. } => assert!(message.contains("Cadê o '+'")),
            _ => panic!("Expected Parse error"),
        }
    }

    // compile() integration
    #[test]
    fn compile_empty_source_errors() {
        match compile("") {
            Err(errors) => match &errors[0] {
                ManoError::Parse { message, span } => {
                    assert!(message.contains("expressão"));
                    assert_eq!(*span, 0..0); // EOF token has empty span
                }
                _ => panic!("expected Parse error"),
            },
            Ok(_) => panic!("empty source should error"),
        }
    }

    #[test]
    fn compile_emits_return_at_end() {
        let chunk = compile("42").unwrap();
        assert_eq!(chunk.code.last(), Some(&(crate::OpCode::Return as u8)));
    }

    #[test]
    fn compile_number_emits_constant() {
        let chunk = compile("42").unwrap();
        // OP_CONSTANT, index, OP_RETURN
        assert_eq!(chunk.code.len(), 3);
        assert_eq!(chunk.code[0], crate::OpCode::Constant as u8);
        assert_eq!(chunk.code[1], 0); // constant index
        assert_eq!(chunk.code[2], crate::OpCode::Return as u8);
        assert_eq!(chunk.constants[0], 42.0);
    }

    #[test]
    fn compile_scanner_error_returns_err() {
        let result = compile("@");
        assert!(result.is_err());
    }

    // 17.4.2 - Parentheses for grouping
    #[test]
    fn grouping_compiles_inner_expression() {
        let grouped = compile("(42)").unwrap();
        let bare = compile("42").unwrap();
        // Grouping is purely syntactic - same bytecode as bare number
        assert_eq!(grouped.code, bare.code);
        assert_eq!(grouped.constants, bare.constants);
    }

    #[test]
    fn grouping_nested() {
        let nested = compile("((42))").unwrap();
        let bare = compile("42").unwrap();
        assert_eq!(nested.code, bare.code);
    }

    #[test]
    fn grouping_missing_right_paren_errors() {
        match compile("(42") {
            Err(errors) => match &errors[0] {
                ManoError::Parse { message, span } => {
                    assert!(message.contains("')'"), "error should mention ')'");
                    assert_eq!(*span, 3..3); // EOF at position 3
                }
                _ => panic!("expected Parse error"),
            },
            Ok(_) => panic!("should fail on missing ')'"),
        }
    }

    #[test]
    fn grouping_empty_errors() {
        match compile("()") {
            Err(errors) => match &errors[0] {
                ManoError::Parse { message, span } => {
                    assert!(
                        message.contains("expressão"),
                        "error should mention 'expressão'"
                    );
                    assert_eq!(*span, 1..2); // ')' token at position 1
                }
                _ => panic!("expected Parse error"),
            },
            Ok(_) => panic!("should fail on empty grouping"),
        }
    }

    // 17.4.3 - Unary negation
    #[test]
    fn unary_negation_emits_negate() {
        let chunk = compile("-42").unwrap();
        // OP_CONSTANT, index, OP_NEGATE, OP_RETURN
        assert_eq!(chunk.code.len(), 4);
        assert_eq!(chunk.code[0], crate::OpCode::Constant as u8);
        assert_eq!(chunk.code[1], 0);
        assert_eq!(chunk.code[2], crate::OpCode::Negate as u8);
        assert_eq!(chunk.code[3], crate::OpCode::Return as u8);
    }

    #[test]
    fn unary_nested_double_negation() {
        let chunk = compile("--42").unwrap();
        // OP_CONSTANT, index, OP_NEGATE, OP_NEGATE, OP_RETURN
        assert_eq!(chunk.code.len(), 5);
        assert_eq!(chunk.code[2], crate::OpCode::Negate as u8);
        assert_eq!(chunk.code[3], crate::OpCode::Negate as u8);
    }

    #[test]
    fn unary_with_grouping() {
        let chunk = compile("-(42)").unwrap();
        // OP_CONSTANT, index, OP_NEGATE, OP_RETURN
        assert_eq!(chunk.code.len(), 4);
        assert_eq!(chunk.code[2], crate::OpCode::Negate as u8);
    }

    // 17.5/17.6 - Parsing Infix Expressions
    #[test]
    fn binary_addition() {
        let chunk = compile("1 + 2").unwrap();
        // OP_CONSTANT 0, OP_CONSTANT 1, OP_ADD, OP_RETURN
        assert_eq!(chunk.code.len(), 6);
        assert_eq!(chunk.code[0], crate::OpCode::Constant as u8);
        assert_eq!(chunk.code[2], crate::OpCode::Constant as u8);
        assert_eq!(chunk.code[4], crate::OpCode::Add as u8);
        assert_eq!(chunk.code[5], crate::OpCode::Return as u8);
    }

    #[test]
    fn binary_subtraction() {
        let chunk = compile("5 - 3").unwrap();
        assert_eq!(chunk.code[4], crate::OpCode::Subtract as u8);
    }

    #[test]
    fn binary_multiplication() {
        let chunk = compile("2 * 3").unwrap();
        assert_eq!(chunk.code[4], crate::OpCode::Multiply as u8);
    }

    #[test]
    fn binary_division() {
        let chunk = compile("6 / 2").unwrap();
        assert_eq!(chunk.code[4], crate::OpCode::Divide as u8);
    }

    #[test]
    fn binary_modulo() {
        let chunk = compile("10 % 3").unwrap();
        assert_eq!(chunk.code[4], crate::OpCode::Modulo as u8);
    }

    #[test]
    fn binary_precedence_mul_over_add() {
        // 2 + 3 * 4 = 2 + 12 = 14, not (2 + 3) * 4 = 20
        let chunk = compile("2 + 3 * 4").unwrap();
        // 2, 3, 4, *, +
        assert_eq!(chunk.code[6], crate::OpCode::Multiply as u8);
        assert_eq!(chunk.code[7], crate::OpCode::Add as u8);
    }

    #[test]
    fn binary_left_associativity() {
        // 1 - 2 - 3 = (1 - 2) - 3 = -4, not 1 - (2 - 3) = 2
        let chunk = compile("1 - 2 - 3").unwrap();
        // 1, 2, -, 3, -
        assert_eq!(chunk.code[4], crate::OpCode::Subtract as u8);
        assert_eq!(chunk.code[7], crate::OpCode::Subtract as u8);
    }

    // Ternary tests (parsing only, no codegen yet)
    #[test]
    fn ternary_missing_colon_errors() {
        let result = compile("1 ? 2");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        match &errors[0] {
            mano::ManoError::Parse { message, .. } => {
                assert!(
                    message.contains(":"),
                    "Expected error about missing ':', got: {}",
                    message
                );
            }
            _ => panic!("Expected Parse error"),
        }
    }

    #[test]
    fn ternary_not_yet_supported() {
        let result = compile("1 ? 2 : 3");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        match &errors[0] {
            mano::ManoError::Parse { message, .. } => {
                assert!(
                    message.to_lowercase().contains("ternário"),
                    "Expected error about ternário, got: {}",
                    message
                );
            }
            _ => panic!("Expected Parse error"),
        }
    }

    // ParseRule tests
    #[test]
    fn parse_rule_number() {
        let rule = ParseRule::from(TokenType::Number);
        assert_eq!(rule.prefix, Some(ParseFn::Number));
        assert_eq!(rule.infix, None);
        assert_eq!(rule.precedence, Precedence::None);
    }

    #[test]
    fn parse_rule_plus() {
        let rule = ParseRule::from(TokenType::Plus);
        assert_eq!(rule.prefix, None);
        assert_eq!(rule.infix, Some(ParseFn::Binary));
        assert_eq!(rule.precedence, Precedence::Term);
    }

    #[test]
    fn parse_rule_minus() {
        let rule = ParseRule::from(TokenType::Minus);
        assert_eq!(rule.prefix, Some(ParseFn::Unary));
        assert_eq!(rule.infix, Some(ParseFn::Binary));
        assert_eq!(rule.precedence, Precedence::Term);
    }

    #[test]
    fn parse_rule_star() {
        let rule = ParseRule::from(TokenType::Star);
        assert_eq!(rule.prefix, None);
        assert_eq!(rule.infix, Some(ParseFn::Binary));
        assert_eq!(rule.precedence, Precedence::Factor);
    }

    #[test]
    fn parse_rule_slash() {
        let rule = ParseRule::from(TokenType::Slash);
        assert_eq!(rule.prefix, None);
        assert_eq!(rule.infix, Some(ParseFn::Binary));
        assert_eq!(rule.precedence, Precedence::Factor);
    }

    #[test]
    fn parse_rule_left_paren() {
        let rule = ParseRule::from(TokenType::LeftParen);
        assert_eq!(rule.prefix, Some(ParseFn::Grouping));
        assert_eq!(rule.infix, None);
        assert_eq!(rule.precedence, Precedence::None);
    }

    #[test]
    fn parse_rule_eof() {
        let rule = ParseRule::from(TokenType::Eof);
        assert_eq!(rule.prefix, None);
        assert_eq!(rule.infix, None);
        assert_eq!(rule.precedence, Precedence::None);
    }

    // ParseFn::call tests
    #[test]
    fn parse_fn_call_number() {
        let mut compiler = Compiler::new("42");
        compiler.advance();
        compiler.advance(); // previous = 42
        ParseFn::Number.call(&mut compiler);
        assert_eq!(compiler.chunk.constants[0], 42.0);
    }

    #[test]
    fn parse_fn_call_unary() {
        let mut compiler = Compiler::new("-42");
        compiler.advance();
        compiler.advance(); // previous = -
        ParseFn::Unary.call(&mut compiler);
        assert_eq!(
            compiler.chunk.code.last(),
            Some(&(crate::OpCode::Negate as u8))
        );
    }

    #[test]
    fn parse_fn_call_grouping() {
        let mut compiler = Compiler::new("(42)");
        compiler.advance();
        compiler.advance(); // previous = (
        ParseFn::Grouping.call(&mut compiler);
        assert_eq!(compiler.chunk.constants[0], 42.0);
    }

    #[test]
    fn parse_fn_call_binary() {
        let mut compiler = Compiler::new("1 + 2");
        compiler.advance();
        compiler.advance(); // previous = 1
        compiler.number(); // compile left operand
        compiler.advance(); // previous = +
        ParseFn::Binary.call(&mut compiler);
        assert_eq!(
            compiler.chunk.code.last(),
            Some(&(crate::OpCode::Add as u8))
        );
    }

    // Precedence::next tests
    #[test]
    fn precedence_next_none_is_assignment() {
        assert_eq!(Precedence::None.next(), Precedence::Assignment);
    }

    #[test]
    fn precedence_next_assignment_is_or() {
        assert_eq!(Precedence::Assignment.next(), Precedence::Or);
    }

    #[test]
    fn precedence_next_or_is_and() {
        assert_eq!(Precedence::Or.next(), Precedence::And);
    }

    #[test]
    fn precedence_next_and_is_equality() {
        assert_eq!(Precedence::And.next(), Precedence::Equality);
    }

    #[test]
    fn precedence_next_equality_is_comparison() {
        assert_eq!(Precedence::Equality.next(), Precedence::Comparison);
    }

    #[test]
    fn precedence_next_comparison_is_term() {
        assert_eq!(Precedence::Comparison.next(), Precedence::Term);
    }

    #[test]
    fn precedence_next_term_is_factor() {
        assert_eq!(Precedence::Term.next(), Precedence::Factor);
    }

    #[test]
    fn precedence_next_factor_is_unary() {
        assert_eq!(Precedence::Factor.next(), Precedence::Unary);
    }

    #[test]
    fn precedence_next_unary_is_call() {
        assert_eq!(Precedence::Unary.next(), Precedence::Call);
    }

    #[test]
    fn precedence_next_call_is_primary() {
        assert_eq!(Precedence::Call.next(), Precedence::Primary);
    }

    #[test]
    fn precedence_next_primary_is_primary() {
        // Max level stays at max
        assert_eq!(Precedence::Primary.next(), Precedence::Primary);
    }
}
