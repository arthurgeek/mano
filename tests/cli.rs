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
    writeln!(file, "salve (1 + 2);").unwrap();

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
        .write_stdin("salve 1 + 2;\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("3"));
}

#[test]
fn repl_recovers_after_error() {
    // Error on first line, second line should still work
    mano()
        .write_stdin("@\nsalve 1 + 2;\n")
        .assert()
        .success()
        .stderr(predicates::str::contains("Tá na nóia"))
        .stdout(predicates::str::contains("3"));
}

#[test]
fn repl_accepts_multiline_block() {
    mano()
        .write_stdin("{\nsalve 42;\n}\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("42"))
        .stderr(predicates::str::is_empty());
}

#[test]
fn repl_accepts_nested_blocks() {
    mano()
        .write_stdin("{\n{\nsalve 1;\n}\nsalve 2;\n}\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("1"))
        .stdout(predicates::str::contains("2"))
        .stderr(predicates::str::is_empty());
}

#[test]
fn repl_block_scoping_works() {
    // Outer var, inner shadows, outer still accessible after block
    mano()
        .write_stdin("seLiga x = 1;\n{\nseLiga x = 99;\nsalve x;\n}\nsalve x;\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("99"))
        .stdout(predicates::str::contains("1"))
        .stderr(predicates::str::is_empty());
}

#[test]
fn repl_auto_prints_expressions() {
    // Expression without semicolon should auto-print result
    mano()
        .write_stdin("seLiga a = 1;\na\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("1"))
        .stderr(predicates::str::is_empty());
}

#[test]
fn repl_auto_prints_string_expression() {
    mano()
        .write_stdin("\"mano\"\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("mano"))
        .stderr(predicates::str::is_empty());
}
