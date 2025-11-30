use std::path::PathBuf;

use mano_mcp::cli::{parse_binary_path, validate_binary};
use mano_mcp::server::{ManoMcp, RunParams, TranslateParams};
use rmcp::{ServerHandler, handler::server::wrapper::Parameters};

const MANO_BIN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../target/debug/mano");

fn server() -> ManoMcp {
    ManoMcp::new(PathBuf::from(MANO_BIN))
}

// run_mano tests

#[tokio::test]
async fn run_mano_executes_hello_world() {
    let result = server()
        .run_mano(Parameters(RunParams {
            code: r#"salve "E aí, mano!";"#.into(),
        }))
        .await;
    assert_eq!(result.trim(), "E aí, mano!");
}

#[tokio::test]
async fn run_mano_executes_arithmetic() {
    let result = server()
        .run_mano(Parameters(RunParams {
            code: "salve 1 + 2 * 3;".into(),
        }))
        .await;
    assert_eq!(result.trim(), "7");
}

#[tokio::test]
async fn run_mano_executes_variable() {
    let result = server()
        .run_mano(Parameters(RunParams {
            code: "seLiga x = 42; salve x;".into(),
        }))
        .await;
    assert_eq!(result.trim(), "42");
}

#[tokio::test]
async fn run_mano_executes_conditional() {
    let result = server()
        .run_mano(Parameters(RunParams {
            code: r#"sePá (firmeza) { salve "yes"; } vacilou { salve "no"; }"#.into(),
        }))
        .await;
    assert_eq!(result.trim(), "yes");
}

#[tokio::test]
async fn run_mano_returns_error_for_invalid_code() {
    let result = server()
        .run_mano(Parameters(RunParams {
            code: "@invalid".into(),
        }))
        .await;
    assert!(result.contains("@"));
}

#[tokio::test]
async fn run_mano_returns_error_for_invalid_binary() {
    let server = ManoMcp::new(PathBuf::from("/nonexistent/mano"));
    let result = server
        .run_mano(Parameters(RunParams {
            code: "salve 42;".into(),
        }))
        .await;
    assert!(result.contains("Error executing mano"));
}

// translate_to_mano tests

#[test]
fn translate_to_mano_includes_keyword_reference() {
    let result = server().translate_to_mano(Parameters(TranslateParams {
        code: "print('hello')".into(),
    }));

    assert!(result.contains("seLiga"));
    assert!(result.contains("salve"));
    assert!(result.contains("sePá"));
    assert!(result.contains("vacilou"));
    assert!(result.contains("print('hello')"));
}

#[test]
fn translate_to_mano_includes_examples() {
    let result = server().translate_to_mano(Parameters(TranslateParams {
        code: "x = 1".into(),
    }));

    assert!(result.contains("seLiga x = 42;"));
    assert!(result.contains("segueOFluxo"));
    assert!(result.contains("seVira"));
}

// get_info tests

#[test]
fn get_info_returns_instructions() {
    let info = server().get_info();
    let instructions = info.instructions.unwrap();

    assert!(instructions.contains("mano"));
    assert!(instructions.contains("interpreter"));
}

#[test]
fn get_info_enables_tools_capability() {
    let info = server().get_info();
    let tools = info.capabilities.tools.unwrap();

    // Tools capability should be enabled (empty object = enabled)
    assert!(tools.list_changed.is_none()); // default
}

// cli tests

#[test]
fn parse_binary_path_returns_default_when_no_args() {
    let args = vec!["mano-mcp".to_string()].into_iter();
    let path = parse_binary_path(args);
    assert_eq!(path, PathBuf::from("mano"));
}

#[test]
fn parse_binary_path_returns_provided_path() {
    let args = vec!["mano-mcp".to_string(), "/custom/path".to_string()].into_iter();
    let path = parse_binary_path(args);
    assert_eq!(path, PathBuf::from("/custom/path"));
}

#[test]
fn validate_binary_returns_ok_for_existing_path() {
    let path = PathBuf::from(MANO_BIN);
    assert!(validate_binary(&path).is_ok());
}

#[test]
fn validate_binary_returns_error_for_nonexistent_path() {
    let path = PathBuf::from("/nonexistent/mano");
    let result = validate_binary(&path);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("mano binary not found"));
}
