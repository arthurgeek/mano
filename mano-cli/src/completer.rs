use rustyline::Context;
use rustyline::Helper;
use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use std::cell::RefCell;

/// Rustyline helper that provides auto-completion for mano REPL
pub struct ManoHelper {
    variables: RefCell<Vec<String>>,
}

impl ManoHelper {
    pub fn new() -> Self {
        Self {
            variables: RefCell::new(Vec::new()),
        }
    }

    pub fn set_variables(&self, vars: Vec<String>) {
        *self.variables.borrow_mut() = vars;
    }

    /// Find the start position of the current word being typed
    fn find_word_start(line: &str, pos: usize) -> usize {
        let before_cursor = &line[..pos];
        // Search backwards for a non-identifier character
        for (i, c) in before_cursor.char_indices().rev() {
            if !c.is_alphanumeric() && c != '_' {
                return i + c.len_utf8();
            }
        }
        0
    }

    /// Get completion candidates for the given prefix
    fn get_completions(prefix: &str, variables: &[String]) -> Vec<String> {
        if prefix.is_empty() {
            return Vec::new();
        }

        let mut completions = Vec::new();

        // Add matching keywords
        for (keyword, _) in mano::KEYWORDS.entries() {
            if keyword.starts_with(prefix) {
                completions.push((*keyword).to_string());
            }
        }

        // Add matching variables
        for var in variables {
            if var.starts_with(prefix) {
                completions.push(var.clone());
            }
        }

        completions
    }

    /// Highlight a line of mano code with ANSI colors using the scanner
    pub fn highlight_line(line: &str, variables: &[String]) -> String {
        if line.is_empty() {
            return String::new();
        }

        // ANSI color codes
        const KEYWORD: &str = "\x1b[35m"; // Magenta
        const STRING: &str = "\x1b[32m"; // Green
        const NUMBER: &str = "\x1b[33m"; // Yellow
        const COMMENT: &str = "\x1b[90m"; // Gray
        const VARIABLE: &str = "\x1b[36m"; // Cyan
        const RESET: &str = "\x1b[0m";

        let scanner = mano::Scanner::with_comments(line);
        let mut result = String::new();
        let mut pos = 0usize; // byte position in line

        for token_result in scanner {
            match token_result {
                Ok(token) => {
                    if token.token_type == mano::TokenType::Eof {
                        break;
                    }

                    // Append any whitespace/characters before this token using span
                    if token.span.start > pos {
                        result.push_str(&line[pos..token.span.start]);
                    }

                    // Determine color based on token type
                    let color = match token.token_type {
                        mano::TokenType::Comment => Some(COMMENT),
                        mano::TokenType::String
                        | mano::TokenType::StringStart
                        | mano::TokenType::StringMiddle
                        | mano::TokenType::StringEnd => Some(STRING),
                        mano::TokenType::Number => Some(NUMBER),
                        mano::TokenType::Identifier => {
                            if variables.contains(&token.lexeme) {
                                Some(VARIABLE)
                            } else {
                                None
                            }
                        }
                        // Keywords
                        mano::TokenType::And
                        | mano::TokenType::Class
                        | mano::TokenType::Else
                        | mano::TokenType::False
                        | mano::TokenType::Fun
                        | mano::TokenType::For
                        | mano::TokenType::If
                        | mano::TokenType::Nil
                        | mano::TokenType::Or
                        | mano::TokenType::Print
                        | mano::TokenType::Return
                        | mano::TokenType::Super
                        | mano::TokenType::This
                        | mano::TokenType::True
                        | mano::TokenType::Var
                        | mano::TokenType::While
                        | mano::TokenType::Break => Some(KEYWORD),
                        // Operators and punctuation - no highlighting
                        _ => None,
                    };

                    // Append token with color
                    if let Some(c) = color {
                        result.push_str(c);
                        result.push_str(&line[token.span.clone()]);
                        result.push_str(RESET);
                    } else {
                        result.push_str(&line[token.span.clone()]);
                    }
                    pos = token.span.end;
                }
                Err(_) => {
                    // On scanner error, the scanner has advanced past the problematic character.
                    // The next token's span.start will include any skipped characters as "whitespace".
                }
            }
        }

        // Append any remaining characters (trailing whitespace or chars after errors)
        if pos < line.len() {
            result.push_str(&line[pos..]);
        }

        result
    }
}

impl Helper for ManoHelper {}

impl Highlighter for ManoHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> std::borrow::Cow<'l, str> {
        let variables = self.variables.borrow();
        std::borrow::Cow::Owned(Self::highlight_line(line, &variables))
    }

    fn highlight_char(
        &self,
        _line: &str,
        _pos: usize,
        _kind: rustyline::highlight::CmdKind,
    ) -> bool {
        true // Always re-highlight
    }
}

impl Hinter for ManoHelper {
    type Hint = String;
}
impl Validator for ManoHelper {}

impl Completer for ManoHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let start = Self::find_word_start(line, pos);
        let prefix = &line[start..pos];
        let variables = self.variables.borrow();
        let completions = Self::get_completions(prefix, &variables);

        let pairs: Vec<Pair> = completions
            .into_iter()
            .map(|s| Pair {
                display: s.clone(),
                replacement: s,
            })
            .collect();

        Ok((start, pairs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ManoHelper integration tests

    #[test]
    fn helper_completes_keyword_at_start() {
        let helper = ManoHelper::new();
        let (start, pairs) = helper
            .complete(
                "sal",
                3,
                &Context::new(&rustyline::history::DefaultHistory::new()),
            )
            .unwrap();
        assert_eq!(start, 0);
        assert!(pairs.iter().any(|p| p.replacement == "salve"));
    }

    #[test]
    fn helper_completes_after_space() {
        let helper = ManoHelper::new();
        let (start, pairs) = helper
            .complete(
                "salve x",
                7,
                &Context::new(&rustyline::history::DefaultHistory::new()),
            )
            .unwrap();
        assert_eq!(start, 6);
        // "x" doesn't match any keyword
        assert!(pairs.is_empty());
    }

    #[test]
    fn helper_completes_variables() {
        let helper = ManoHelper::new();
        helper.set_variables(vec!["contador".to_string()]);
        let (start, pairs) = helper
            .complete(
                "salve con",
                9,
                &Context::new(&rustyline::history::DefaultHistory::new()),
            )
            .unwrap();
        assert_eq!(start, 6);
        assert!(pairs.iter().any(|p| p.replacement == "contador"));
    }

    #[test]
    fn helper_updates_variables() {
        let helper = ManoHelper::new();
        helper.set_variables(vec!["x".to_string()]);
        helper.set_variables(vec!["y".to_string()]);
        let (_, pairs) = helper
            .complete(
                "y",
                1,
                &Context::new(&rustyline::history::DefaultHistory::new()),
            )
            .unwrap();
        assert!(pairs.iter().any(|p| p.replacement == "y"));
        let (_, pairs) = helper
            .complete(
                "x",
                1,
                &Context::new(&rustyline::history::DefaultHistory::new()),
            )
            .unwrap();
        assert!(pairs.is_empty());
    }

    // Unit tests for helper methods

    #[test]
    fn find_word_start_at_beginning() {
        assert_eq!(ManoHelper::find_word_start("sal", 3), 0);
    }

    #[test]
    fn find_word_start_after_space() {
        assert_eq!(ManoHelper::find_word_start("salve x", 7), 6);
    }

    #[test]
    fn find_word_start_after_operator() {
        assert_eq!(ManoHelper::find_word_start("1 + se", 6), 4);
    }

    #[test]
    fn find_word_start_empty() {
        assert_eq!(ManoHelper::find_word_start("", 0), 0);
    }

    #[test]
    fn get_completions_matches_keywords() {
        let completions = ManoHelper::get_completions("sal", &[]);
        assert!(completions.contains(&"salve".to_string()));
    }

    #[test]
    fn get_completions_matches_multiple_keywords() {
        let completions = ManoHelper::get_completions("se", &[]);
        assert!(completions.contains(&"seLiga".to_string()));
        assert!(completions.contains(&"sePá".to_string()));
        assert!(completions.contains(&"seVira".to_string()));
        assert!(completions.contains(&"segueOFluxo".to_string()));
    }

    #[test]
    fn get_completions_matches_variables() {
        let vars = vec!["contador".to_string(), "nome".to_string()];
        let completions = ManoHelper::get_completions("con", &vars);
        assert!(completions.contains(&"contador".to_string()));
        assert!(!completions.contains(&"nome".to_string()));
    }

    #[test]
    fn get_completions_matches_both_keywords_and_variables() {
        let vars = vec!["salário".to_string()];
        let completions = ManoHelper::get_completions("sal", &vars);
        assert!(completions.contains(&"salve".to_string()));
        assert!(completions.contains(&"salário".to_string()));
    }

    #[test]
    fn get_completions_empty_prefix_returns_empty() {
        let completions = ManoHelper::get_completions("", &["x".to_string()]);
        assert!(completions.is_empty());
    }

    #[test]
    fn get_completions_no_match_returns_empty() {
        let completions = ManoHelper::get_completions("xyz", &[]);
        assert!(completions.is_empty());
    }

    #[test]
    fn get_completions_unicode_prefix_matches() {
        let vars = vec!["salário".to_string(), "salame".to_string()];
        let completions = ManoHelper::get_completions("salá", &vars);
        assert!(completions.contains(&"salário".to_string()));
        assert!(!completions.contains(&"salame".to_string()));
    }

    // Highlighter tests

    #[test]
    fn highlight_keywords() {
        let result = ManoHelper::highlight_line("salve", &[]);
        assert!(result.contains("\x1b[")); // Contains ANSI escape
        assert!(result.contains("salve"));
    }

    #[test]
    fn highlight_multiple_keywords() {
        let result = ManoHelper::highlight_line("seLiga x = firmeza", &[]);
        assert!(result.contains("seLiga"));
        assert!(result.contains("firmeza"));
        // Non-keywords should be present too
        assert!(result.contains("x"));
    }

    #[test]
    fn highlight_string() {
        let result = ManoHelper::highlight_line("salve \"oi mano\"", &[]);
        assert!(result.contains("\"oi mano\""));
    }

    #[test]
    fn highlight_number() {
        let result = ManoHelper::highlight_line("salve 42", &[]);
        assert!(result.contains("42"));
    }

    #[test]
    fn highlight_line_comment() {
        let result = ManoHelper::highlight_line("// comentário", &[]);
        assert!(result.contains("\x1b[90m")); // Gray color
        assert!(result.contains("// comentário"));
    }

    #[test]
    fn highlight_block_comment() {
        let result = ManoHelper::highlight_line("/* bloco */", &[]);
        assert!(result.contains("\x1b[90m")); // Gray color
        assert!(result.contains("/* bloco */"));
    }

    #[test]
    fn highlight_inline_block_comment() {
        let result = ManoHelper::highlight_line("salve /* comentário */ 42", &[]);
        assert!(result.contains("\x1b[35m")); // Keyword color (salve)
        assert!(result.contains("\x1b[90m")); // Comment color
        assert!(result.contains("\x1b[33m")); // Number color (42)
    }

    #[test]
    fn highlight_empty_returns_empty() {
        let result = ManoHelper::highlight_line("", &[]);
        assert_eq!(result, "");
    }

    #[test]
    fn highlight_handles_scanner_errors() {
        // '@' is an invalid character that causes scanner error
        let result = ManoHelper::highlight_line("salve @", &[]);
        // Should still highlight valid tokens and preserve invalid characters
        assert!(result.contains("salve"));
        assert!(result.contains("@"));
    }

    #[test]
    fn highlight_preserves_structure() {
        // The highlighted output should have the same visible characters
        let input = "seLiga x = 1 + 2";
        let result = ManoHelper::highlight_line(input, &[]);
        // Strip ANSI codes and check structure is preserved
        let stripped = strip_ansi(&result);
        assert_eq!(stripped, input);
    }

    #[test]
    fn highlight_variables() {
        let vars = vec!["contador".to_string()];
        let result = ManoHelper::highlight_line("salve contador", &vars);
        // Should contain ANSI codes for both keyword and variable
        assert!(result.contains("\x1b[35m")); // Keyword color (magenta)
        assert!(result.contains("\x1b[36m")); // Variable color (cyan)
        assert!(result.contains("contador"));
    }

    #[test]
    fn highlighter_trait_uses_variables() {
        use rustyline::highlight::Highlighter;

        let helper = ManoHelper::new();
        helper.set_variables(vec!["meuVar".to_string()]);

        let result = helper.highlight("salve meuVar", 0);
        // Should highlight both keyword and variable
        assert!(result.contains("\x1b[35m")); // Keyword color
        assert!(result.contains("\x1b[36m")); // Variable color
    }

    /// Helper to strip ANSI escape codes for testing
    fn strip_ansi(s: &str) -> String {
        let mut result = String::new();
        let mut in_escape = false;
        for c in s.chars() {
            if c == '\x1b' {
                in_escape = true;
            } else if in_escape {
                if c == 'm' {
                    in_escape = false;
                }
            } else {
                result.push(c);
            }
        }
        result
    }
}
