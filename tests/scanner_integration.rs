use std::io::Write;
use std::path::Path;

use mano::{Mano, ManoError};

#[test]
fn evaluates_simple_expression() {
    let mut mano = Mano::new();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let result = mano.run_with_output("salve (1 + 2);", &mut stdout, &mut stderr);

    assert!(result.is_ok());
    let output = String::from_utf8(stdout).unwrap();
    assert_eq!(output.trim(), "3");
}

#[test]
fn evaluates_comparison_operators() {
    let mut mano = Mano::new();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let result = mano.run_with_output("salve 1 < 2;", &mut stdout, &mut stderr);

    assert!(result.is_ok());
    let output = String::from_utf8(stdout).unwrap();
    assert_eq!(output.trim(), "firmeza");
}

#[test]
fn evaluates_equality_operators() {
    let mut mano = Mano::new();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let result = mano.run_with_output("salve 1 == 2;", &mut stdout, &mut stderr);

    assert!(result.is_ok());
    let output = String::from_utf8(stdout).unwrap();
    assert_eq!(output.trim(), "treta");
}

#[test]
fn evaluates_unary_operators() {
    let mut mano = Mano::new();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let result = mano.run_with_output("salve -42;", &mut stdout, &mut stderr);

    assert!(result.is_ok());
    let output = String::from_utf8(stdout).unwrap();
    assert_eq!(output.trim(), "-42");
}

#[test]
fn parses_boolean_literals() {
    let mut mano = Mano::new();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let result = mano.run_with_output("salve firmeza;", &mut stdout, &mut stderr);

    assert!(result.is_ok());
    let output = String::from_utf8(stdout).unwrap();
    assert_eq!(output.trim(), "firmeza");
}

#[test]
fn reports_error_for_invalid_character() {
    let mut mano = Mano::new();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let result = mano.run_with_output("@", &mut stdout, &mut stderr);

    assert!(result.is_ok());
    let errors = String::from_utf8(stderr).unwrap();
    assert!(errors.contains("@"));
    assert!(errors.contains("Tá na nóia"));
}

#[test]
fn continues_scanning_after_error() {
    let mut mano = Mano::new();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    // Both @ and $ are invalid - should scan all, not stop at first
    let result = mano.run_with_output("@$", &mut stdout, &mut stderr);

    assert!(result.is_ok());
    let errors = String::from_utf8(stderr).unwrap();
    assert!(errors.contains("@"));
    assert!(errors.contains("$"));
}

#[test]
fn runs_file_with_mano_code() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    writeln!(file, "salve 1 + 2;").unwrap();

    let mut mano = Mano::new();
    let result = mano.run_file(file.path());
    assert!(result.is_ok());
}

#[test]
fn returns_io_error_for_missing_file() {
    let mut mano = Mano::new();
    let result = mano.run_file(Path::new("arquivo_que_nao_existe.mano"));
    assert!(matches!(result, Err(ManoError::Io(_))));
}

#[test]
fn evaluates_complex_expression() {
    let mut mano = Mano::new();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let result = mano.run_with_output("salve 1 + 2 * 3;", &mut stdout, &mut stderr);

    assert!(result.is_ok());
    let output = String::from_utf8(stdout).unwrap();
    // Should respect precedence: 1 + (2 * 3) = 7
    assert_eq!(output.trim(), "7");
}

#[test]
fn parses_string_literal() {
    let mut mano = Mano::new();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let result = mano.run_with_output("salve \"e aí mano\";", &mut stdout, &mut stderr);

    assert!(result.is_ok());
    let output = String::from_utf8(stdout).unwrap();
    assert_eq!(output.trim(), "e aí mano");
}

#[test]
fn reports_unterminated_string() {
    let mut mano = Mano::new();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let result = mano.run_with_output("\"esqueceu de fechar", &mut stdout, &mut stderr);

    assert!(result.is_ok());
    let errors = String::from_utf8(stderr).unwrap();
    assert!(errors.contains("Tá moscando, Brown?"));
    assert!(errors.contains("fecha aspas"));
}
