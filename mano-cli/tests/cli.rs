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
        .stderr(predicates::str::contains("Tá moscando, Brown?"))
        .stdout(predicates::str::contains("3"));
}

#[test]
fn repl_errors_use_ariadne_format() {
    let output = mano().write_stdin("@\n").output().unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Strip ANSI codes for snapshot
    let stderr_clean = strip_ansi(&stderr);
    insta::assert_snapshot!(stderr_clean);
}

#[test]
fn file_errors_show_filename() {
    let mut file = tempfile::NamedTempFile::with_suffix(".mano").unwrap();
    std::io::Write::write_all(&mut file, b"@\n").unwrap();

    let output = mano().arg(file.path()).output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Replace temp path with placeholder for snapshot stability
    let stderr_clean = strip_ansi(&stderr).replace(
        &file.path().to_string_lossy().to_string(),
        "<tempfile>.mano",
    );
    insta::assert_snapshot!(stderr_clean);
}

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
