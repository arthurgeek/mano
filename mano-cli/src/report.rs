use ariadne::{Color, Label, Report, ReportKind, Source};
use mano::ManoError;
use std::io::Write;
use std::ops::Range;

/// Converts a byte span to a character span for ariadne
fn byte_to_char_span(source: &str, byte_span: &Range<usize>) -> Range<usize> {
    let start = source[..byte_span.start].chars().count();
    let end = source[..byte_span.end.min(source.len())].chars().count();
    start..end
}

/// Renders a ManoError using ariadne for beautiful error output
pub fn report_error<W: Write>(
    error: &ManoError,
    source: &str,
    filename: Option<&str>,
    mut writer: W,
) {
    // Use filename or empty string for source ID
    let name = filename.unwrap_or("");
    let src = (name, Source::from(source));

    match error {
        ManoError::Io(_) => {
            writeln!(writer, "{}", error).ok();
        }
        ManoError::Scan { span, message } => {
            let char_span = byte_to_char_span(source, span);
            Report::build(ReportKind::Error, (name, char_span.clone()))
                .with_message(error.to_string())
                .with_label(
                    Label::new((name, char_span))
                        .with_message(message)
                        .with_color(Color::Red),
                )
                .finish()
                .write(src, &mut writer)
                .ok();
        }
        ManoError::Parse { span, message } => {
            let char_span = byte_to_char_span(source, span);
            Report::build(ReportKind::Error, (name, char_span.clone()))
                .with_message(error.to_string())
                .with_label(
                    Label::new((name, char_span))
                        .with_message(message)
                        .with_color(Color::Red),
                )
                .finish()
                .write(src, &mut writer)
                .ok();
        }
        ManoError::Runtime { span, message } => {
            let char_span = byte_to_char_span(source, span);
            Report::build(ReportKind::Error, (name, char_span.clone()))
                .with_message(error.to_string())
                .with_label(
                    Label::new((name, char_span))
                        .with_message(message)
                        .with_color(Color::Red),
                )
                .finish()
                .write(src, &mut writer)
                .ok();
        }
        ManoError::Break | ManoError::Return(_) => {
            // Internal control flow, should never be reported to users
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_to_char_span_ascii_unchanged() {
        let source = "hello world";
        assert_eq!(byte_to_char_span(source, &(0..5)), 0..5);
        assert_eq!(byte_to_char_span(source, &(6..11)), 6..11);
    }

    #[test]
    fn byte_to_char_span_converts_utf8() {
        // "aí" - 'a' is 1 byte, 'í' is 2 bytes = 3 bytes total, 2 chars
        let source = "aí";
        assert_eq!(byte_to_char_span(source, &(0..1)), 0..1); // 'a'
        assert_eq!(byte_to_char_span(source, &(0..3)), 0..2); // "aí"
        assert_eq!(byte_to_char_span(source, &(1..3)), 1..2); // 'í'
    }

    #[test]
    fn byte_to_char_span_handles_mixed_content() {
        // "e aí mano" = 'e'(1) + ' '(1) + 'a'(1) + 'í'(2) + ' '(1) + 'm'(1) + 'a'(1) + 'n'(1) + 'o'(1) = 10 bytes, 9 chars
        let source = "e aí mano";
        assert_eq!(byte_to_char_span(source, &(0..10)), 0..9);
    }

    #[test]
    fn byte_to_char_span_clamps_to_source_length() {
        let source = "hi";
        assert_eq!(byte_to_char_span(source, &(0..100)), 0..2);
    }

    /// Helper to strip ANSI escape codes for snapshot testing
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

    #[test]
    fn report_scan_error_shows_span() {
        let error = ManoError::Scan {
            message: "E esse '@' aí, truta?".to_string(),
            span: 6..7,
        };
        let source = "salve @";
        let mut output = Vec::new();
        report_error(&error, source, None, &mut output);
        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("@"));
        assert!(result.contains("Tá moscando"));
    }

    #[test]
    fn report_parse_error_shows_span() {
        let error = ManoError::Parse {
            message: "Cadê o ponto e vírgula?".to_string(),
            span: 5..6,
        };
        let source = "salve 42";
        let mut output = Vec::new();
        report_error(&error, source, None, &mut output);
        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("Deu mole"));
    }

    #[test]
    fn report_runtime_error_shows_span() {
        let error = ManoError::Runtime {
            message: "Só dá pra negar número!".to_string(),
            span: 6..11,
        };
        let source = "salve -\"oi\"";
        let mut output = Vec::new();
        report_error(&error, source, None, &mut output);
        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("Só dá pra negar número!"));
    }

    #[test]
    fn report_io_error_just_prints_message() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let error: ManoError = io_err.into();
        let mut output = Vec::new();
        report_error(&error, "", None, &mut output);
        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("Cadê o arquivo"));
    }

    #[test]
    fn report_break_outputs_nothing() {
        let error = ManoError::Break;
        let mut output = Vec::new();
        report_error(&error, "", None, &mut output);
        assert!(output.is_empty());
    }

    // Snapshot tests for exact error formatting
    #[test]
    fn snapshot_scan_error() {
        let error = ManoError::Scan {
            message: "E esse '@' aí, truta?".to_string(),
            span: 6..7,
        };
        let source = "salve @";
        let mut output = Vec::new();
        report_error(&error, source, None, &mut output);
        let result = strip_ansi(&String::from_utf8(output).unwrap());
        insta::assert_snapshot!(result);
    }

    #[test]
    fn snapshot_parse_error() {
        let error = ManoError::Parse {
            message: "Cadê o ponto e vírgula?".to_string(),
            span: 8..8,
        };
        let source = "salve 42";
        let mut output = Vec::new();
        report_error(&error, source, None, &mut output);
        let result = strip_ansi(&String::from_utf8(output).unwrap());
        insta::assert_snapshot!(result);
    }

    #[test]
    fn snapshot_runtime_error() {
        let error = ManoError::Runtime {
            message: "Só dá pra negar número, chapa!".to_string(),
            span: 6..11,
        };
        let source = "salve -\"oi\"";
        let mut output = Vec::new();
        report_error(&error, source, None, &mut output);
        let result = strip_ansi(&String::from_utf8(output).unwrap());
        insta::assert_snapshot!(result);
    }

    #[test]
    fn report_error_renders_multibyte_utf8_spans() {
        // Byte 25 = ", byte 36 = EOF (after final \n)
        let error = ManoError::Scan {
            message: "Cadê o fecha aspas?".to_string(),
            span: 25..36,
        };
        let source = "// Erros do scanner\n@\n$\n\"e aí mano\n";
        let mut output = Vec::new();
        report_error(&error, source, None, &mut output);
        let result = strip_ansi(&String::from_utf8(output).unwrap());
        insta::assert_snapshot!(result);
    }
}
