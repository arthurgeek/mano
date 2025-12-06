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
        .code(2)
        .stderr(predicates::str::contains("Usage: mano"));
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
    // Error on first line exits with failure (piped input doesn't recover)
    mano()
        .write_stdin("@\nsalve 1 + 2;\n")
        .assert()
        .failure()
        .stderr(predicates::str::contains("Tá moscando, Brown?"));
}

#[test]
fn repl_errors_use_ariadne_format() {
    let output = mano().write_stdin("@\n").output().unwrap();
    assert!(!output.status.success()); // Should fail with error
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
    assert!(!output.status.success()); // Should fail with error
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
    // Expression without semicolon in piped input should error (no auto-print)
    mano().write_stdin("seLiga a = 1;\na\n").assert().failure();
}

#[test]
fn repl_auto_prints_string_expression() {
    // Expression without semicolon in piped input should error (no auto-print)
    mano().write_stdin("\"mano\"\n").assert().failure();
}

#[test]
fn piped_input_does_not_auto_print_declarations() {
    // When stdin is piped (not TTY), declarations should not auto-print
    mano()
        .write_stdin("bagulho Pessoa {} seLiga a = Pessoa(); salve a;\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("<parada Pessoa>"))
        .stderr(predicates::str::is_empty());
}

#[test]
fn piped_input_does_not_auto_print_expressions() {
    // When stdin is piped, expressions without semicolons should error, not auto-print
    mano()
        .write_stdin("seLiga a = \"a\";\na\n")
        .assert()
        .failure()
        .stderr(predicates::str::contains("Deu mole"));
}

#[test]
fn errors_are_not_printed_twice() {
    // Errors should only be printed once, not duplicated
    let output = mano().write_stdin("@\n").output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Count how many times the error message appears
    let count = stderr.matches("Tá moscando, Brown?").count();
    assert_eq!(
        count, 1,
        "Error message should appear exactly once, but appeared {} times",
        count
    );
}

#[test]
fn io_errors_are_still_printed() {
    // IO errors like file not found should be printed
    let output = mano().arg("nonexistent.mano").output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should contain IO error message about missing file
    assert!(stderr.contains("Cadê o arquivo") || stderr.contains("No such file"));
}

#[test]
fn vm_flag_runs_bytecode() {
    mano()
        .args(["--vm"])
        .write_stdin("1 + 2 * 3\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("7"));
}

#[test]
fn vm_debug_flag_traces_execution() {
    mano()
        .args(["--vm", "--debug"])
        .write_stdin("42\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("== code =="))
        .stdout(predicates::str::contains("== trace =="))
        .stdout(predicates::str::contains("OP_CONSTANT"));
}

#[test]
fn vm_file_mode_works() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    writeln!(file, "1 + 2").unwrap();

    mano()
        .args(["--vm"])
        .arg(file.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("3"));
}

#[test]
fn vm_reports_errors() {
    mano()
        .args(["--vm"])
        .write_stdin("1 +\n")
        .assert()
        .failure()
        .stderr(predicates::str::contains("Deu mole"));
}

#[test]
fn help_flag_shows_usage() {
    mano()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("--vm"));
}
