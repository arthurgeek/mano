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
fn prints_tokens_from_file() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    writeln!(file, "(1 + 2)").unwrap();

    mano()
        .arg(file.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("LeftParen"))
        .stdout(predicates::str::contains("Number"))
        .stdout(predicates::str::contains("Plus"));
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
    mano()
        .write_stdin("")
        .assert()
        .success()
        .stdout(predicates::str::contains("> "));
}

#[test]
fn repl_processes_input() {
    mano()
        .write_stdin("()\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("LeftParen"))
        .stdout(predicates::str::contains("RightParen"));
}
