use assert_cmd::Command;
use std::io::Write;

fn mano() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("mano"))
}

#[test]
fn runs_file_successfully() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    writeln!(file, "salve \"oi mano\";").unwrap();

    mano().arg(file.path()).assert().success();
}

#[test]
fn evaluates_expression_from_file() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    writeln!(file, "(1 + 2)").unwrap();

    mano()
        .arg(file.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("3"));
}

#[test]
fn prints_usage_with_too_many_args() {
    mano()
        .args(["file1.mano", "file2.mano"])
        .assert()
        .code(64)
        .stderr(predicates::str::contains("Uso: mano"));
}

#[test]
fn exits_with_error_for_missing_file() {
    mano()
        .arg("arquivo_que_nao_existe.mano")
        .assert()
        .code(65)
        .stderr(predicates::str::contains("Cadê o arquivo"));
}

#[test]
fn repl_exits_on_eof() {
    // When stdin is piped and empty, rustyline returns EOF immediately
    // without printing the prompt (non-tty behavior)
    mano().write_stdin("").assert().success();
}

#[test]
fn repl_evaluates_expression() {
    mano()
        .write_stdin("1 + 2\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("3"));
}

#[test]
fn repl_recovers_after_error() {
    // Error on first line, second line should still work
    mano()
        .write_stdin("@\n1 + 2\n")
        .assert()
        .success()
        .stderr(predicates::str::contains("Tá na nóia"))
        .stdout(predicates::str::contains("3"));
}
