use std::io::Write;
use std::path::Path;

use mano::{Mano, ManoError};

#[test]
fn scans_simple_expression() {
    let mut mano = Mano::new();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let result = mano.run_with_output("(1 + 2)", &mut stdout, &mut stderr);

    assert!(result.is_ok());
    let output = String::from_utf8(stdout).unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert!(lines[0].starts_with("LeftParen"));
    assert!(lines[1].starts_with("Number") && lines[1].contains("1"));
    assert!(lines[2].starts_with("Plus"));
    assert!(lines[3].starts_with("Number") && lines[3].contains("2"));
    assert!(lines[4].starts_with("RightParen"));
    assert!(lines[5].starts_with("Eof"));
}

#[test]
fn scans_mano_keywords() {
    let mut mano = Mano::new();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let result = mano.run_with_output("seLiga x = firmeza;", &mut stdout, &mut stderr);

    assert!(result.is_ok());
    let output = String::from_utf8(stdout).unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert!(lines[0].starts_with("Var") && lines[0].contains("seLiga"));
    assert!(lines[1].starts_with("Identifier") && lines[1].contains("x"));
    assert!(lines[2].starts_with("Equal"));
    assert!(lines[3].starts_with("True") && lines[3].contains("firmeza"));
    assert!(lines[4].starts_with("Semicolon"));
}

#[test]
fn scans_string_with_unicode() {
    let mut mano = Mano::new();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let result = mano.run_with_output("salve \"e aí mano, beleza?\";", &mut stdout, &mut stderr);

    assert!(result.is_ok());
    let output = String::from_utf8(stdout).unwrap();
    assert!(output.contains("e aí mano, beleza?"));
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
    assert!(errors.contains("Tá inventando"));
}

#[test]
fn continues_scanning_after_error() {
    let mut mano = Mano::new();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    // Both @ and $ are invalid - should scan all, not stop at first
    let result = mano.run_with_output("(@$)", &mut stdout, &mut stderr);

    assert!(result.is_ok());
    let errors = String::from_utf8(stderr).unwrap();
    assert!(errors.contains("@"));
    assert!(errors.contains("$"));

    // And we still got the valid tokens
    let output = String::from_utf8(stdout).unwrap();
    assert!(output.contains("LeftParen"));
    assert!(output.contains("RightParen"));
}

#[test]
fn runs_file_with_mano_code() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    writeln!(file, "seLiga nome = \"mano\";").unwrap();
    writeln!(file, "salve nome;").unwrap();

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
fn scans_all_operators() {
    let mut mano = Mano::new();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let result = mano.run_with_output("! != = == < <= > >=", &mut stdout, &mut stderr);

    assert!(result.is_ok());
    let output = String::from_utf8(stdout).unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert!(lines[0].starts_with("Bang "));
    assert!(lines[1].starts_with("BangEqual"));
    assert!(lines[2].starts_with("Equal "));
    assert!(lines[3].starts_with("EqualEqual"));
    assert!(lines[4].starts_with("Less "));
    assert!(lines[5].starts_with("LessEqual"));
    assert!(lines[6].starts_with("Greater "));
    assert!(lines[7].starts_with("GreaterEqual"));
}

#[test]
fn scans_number_literals() {
    let mut mano = Mano::new();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let result = mano.run_with_output("123 45.67", &mut stdout, &mut stderr);

    assert!(result.is_ok());
    let output = String::from_utf8(stdout).unwrap();
    assert!(output.contains("Number") && output.contains("123"));
    assert!(output.contains("45.67"));
}

#[test]
fn handles_comments() {
    let mut mano = Mano::new();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let result = mano.run_with_output("( // isso é um comentário\n)", &mut stdout, &mut stderr);

    assert!(result.is_ok());
    let output = String::from_utf8(stdout).unwrap();
    // Comment should NOT appear in output
    assert!(!output.contains("comentário"));
    // But the parentheses should
    assert!(output.contains("LeftParen"));
    assert!(output.contains("RightParen"));
}

#[test]
fn handles_multiline_string() {
    let mut mano = Mano::new();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let result = mano.run_with_output("\"linha 1\nlinha 2\"", &mut stdout, &mut stderr);

    assert!(result.is_ok());
    let output = String::from_utf8(stdout).unwrap();
    assert!(output.contains("linha 1"));
    assert!(output.contains("linha 2"));
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
