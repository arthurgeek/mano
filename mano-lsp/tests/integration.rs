use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

struct LspClient {
    child: Child,
    stdin: std::process::ChildStdin,
    receiver: mpsc::Receiver<String>,
}

impl LspClient {
    fn spawn() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_mano-lsp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("Failed to start mano-lsp");

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        // Spawn a thread to read messages and send them through a channel
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut content_length = 0;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap() == 0 {
                        return; // EOF
                    }
                    if line == "\r\n" {
                        break;
                    }
                    if line.starts_with("Content-Length:") {
                        content_length = line
                            .trim_start_matches("Content-Length:")
                            .trim()
                            .parse()
                            .unwrap();
                    }
                }
                let mut buf = vec![0u8; content_length];
                reader.read_exact(&mut buf).unwrap();
                let msg = String::from_utf8(buf).unwrap();
                if sender.send(msg).is_err() {
                    return; // Receiver dropped
                }
            }
        });

        Self {
            child,
            stdin,
            receiver,
        }
    }

    fn send(&mut self, content: &str) {
        let msg = format!("Content-Length: {}\r\n\r\n{}", content.len(), content);
        self.stdin.write_all(msg.as_bytes()).unwrap();
        self.stdin.flush().unwrap();
    }

    fn recv(&mut self) -> String {
        self.recv_timeout(Duration::from_secs(1))
    }

    fn recv_timeout(&mut self, timeout: Duration) -> String {
        self.receiver
            .recv_timeout(timeout)
            .unwrap_or_else(|_| panic!("Timeout waiting for LSP response after {:?}", timeout))
    }

    fn initialize(&mut self) {
        self.send(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#);
        let response = self.recv();
        assert!(
            response.contains(r#""id":1"#),
            "Expected initialize response"
        );

        self.send(r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#);
    }

    fn shutdown(mut self) {
        self.send(r#"{"jsonrpc":"2.0","id":99,"method":"shutdown","params":null}"#);
        let _ = self.recv();
        self.send(r#"{"jsonrpc":"2.0","method":"exit","params":null}"#);

        let status = self.child.wait().expect("wait failed");
        assert!(status.success(), "LSP exited with error: {:?}", status);
    }
}

#[test]
fn lsp_publishes_diagnostics_on_did_open() {
    let mut lsp = LspClient::spawn();
    lsp.initialize();

    // Open document with scan error
    lsp.send(r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///test.mano","languageId":"mano","version":1,"text":"@"}}}"#);

    let diagnostics = lsp.recv();
    assert!(diagnostics.contains("textDocument/publishDiagnostics"));
    assert!(diagnostics.contains("@")); // Error message mentions invalid char

    lsp.shutdown();
}

#[test]
fn lsp_clears_diagnostics_when_errors_fixed() {
    let mut lsp = LspClient::spawn();
    lsp.initialize();

    // Open with error
    lsp.send(r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///test.mano","languageId":"mano","version":1,"text":"@"}}}"#);
    let diag1 = lsp.recv();
    assert!(diag1.contains("diagnostics"));

    // Fix the error
    lsp.send(r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///test.mano","version":2},"contentChanges":[{"text":"salve 42;"}]}}"#);
    let diag2 = lsp.recv();
    assert!(diag2.contains(r#""diagnostics":[]"#)); // No errors

    lsp.shutdown();
}

#[test]
fn lsp_returns_completions() {
    let mut lsp = LspClient::spawn();
    lsp.initialize();

    // Open document with a variable
    lsp.send(r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///test.mano","languageId":"mano","version":1,"text":"seLiga meuNome = 42;\nsal"}}}"#);
    let _ = lsp.recv(); // diagnostics

    // Request completion at position after "sal"
    lsp.send(r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/completion","params":{"textDocument":{"uri":"file:///test.mano"},"position":{"line":1,"character":3}}}"#);
    let response = lsp.recv();

    assert!(
        response.contains(r#""id":2"#),
        "Expected completion response"
    );
    assert!(
        response.contains("salve"),
        "Expected 'salve' keyword completion"
    );

    lsp.shutdown();
}

#[test]
fn lsp_go_to_definition() {
    let mut lsp = LspClient::spawn();
    lsp.initialize();

    // Open document: variable declared on line 0, used on line 1
    // "seLiga foo = 42;\nsalve foo;"
    //        ^foo declaration at col 7
    //              ^foo usage at line 1, col 6
    lsp.send(r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///test.mano","languageId":"mano","version":1,"text":"seLiga foo = 42;\nsalve foo;"}}}"#);
    let _ = lsp.recv(); // diagnostics

    // Request go to definition at "foo" on line 1
    lsp.send(r#"{"jsonrpc":"2.0","id":3,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///test.mano"},"position":{"line":1,"character":6}}}"#);
    let response = lsp.recv();

    assert!(
        response.contains(r#""id":3"#),
        "Expected definition response"
    );
    // Should point to line 0 where foo is declared
    assert!(
        response.contains(r#""line":0"#),
        "Expected definition on line 0"
    );

    lsp.shutdown();
}

#[test]
fn lsp_hover() {
    let mut lsp = LspClient::spawn();
    lsp.initialize();

    lsp.send(r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///test.mano","languageId":"mano","version":1,"text":"salve 42;"}}}"#);
    let _ = lsp.recv(); // diagnostics

    // Request hover on "salve" keyword
    lsp.send(r#"{"jsonrpc":"2.0","id":4,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///test.mano"},"position":{"line":0,"character":0}}}"#);
    let response = lsp.recv();

    assert!(response.contains(r#""id":4"#), "Expected hover response");
    assert!(
        response.contains("salve"),
        "Expected hover to mention salve"
    );

    lsp.shutdown();
}

#[test]
fn lsp_document_symbols() {
    let mut lsp = LspClient::spawn();
    lsp.initialize();

    lsp.send(r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///test.mano","languageId":"mano","version":1,"text":"seLiga foo = 42;\nseLiga bar = 10;"}}}"#);
    let _ = lsp.recv(); // diagnostics

    // Request document symbols
    lsp.send(r#"{"jsonrpc":"2.0","id":5,"method":"textDocument/documentSymbol","params":{"textDocument":{"uri":"file:///test.mano"}}}"#);
    let response = lsp.recv();

    assert!(
        response.contains(r#""id":5"#),
        "Expected document symbols response"
    );
    assert!(response.contains("foo"), "Expected 'foo' symbol");
    assert!(response.contains("bar"), "Expected 'bar' symbol");

    lsp.shutdown();
}

#[test]
fn lsp_find_references() {
    let mut lsp = LspClient::spawn();
    lsp.initialize();

    // foo is declared on line 0, used on lines 1 and 2
    lsp.send(r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///test.mano","languageId":"mano","version":1,"text":"seLiga foo = 42;\nsalve foo;\nsalve foo + 1;"}}}"#);
    let _ = lsp.recv(); // diagnostics

    // Request references at "foo" on line 1
    lsp.send(r#"{"jsonrpc":"2.0","id":6,"method":"textDocument/references","params":{"textDocument":{"uri":"file:///test.mano"},"position":{"line":1,"character":6},"context":{"includeDeclaration":true}}}"#);
    let response = lsp.recv();

    assert!(
        response.contains(r#""id":6"#),
        "Expected references response"
    );
    // Should find 3 locations: declaration + 2 usages
    assert!(
        response.contains(r#""line":0"#),
        "Expected reference on line 0"
    );
    assert!(
        response.contains(r#""line":1"#),
        "Expected reference on line 1"
    );
    assert!(
        response.contains(r#""line":2"#),
        "Expected reference on line 2"
    );

    lsp.shutdown();
}

#[test]
fn lsp_hover_handles_multibyte_utf8_positions() {
    let mut lsp = LspClient::spawn();
    lsp.initialize();

    // "sePá" has 'á' which is 2 bytes in UTF-8 but 1 UTF-16 code unit
    // Variable "foo" starts at UTF-16 position 5, but byte position 6
    // This test ensures LSP positions (UTF-16) are correctly converted to byte offsets
    lsp.send(r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///test.mano","languageId":"mano","version":1,"text":"sePá (foo) {}"}}}"#);
    let _ = lsp.recv(); // diagnostics

    // Request hover on "foo" - UTF-16 position 6 (after "sePá (")
    // Without proper UTF-16 to byte conversion, this would panic with:
    // "byte index 6 is not a char boundary; it is inside 'á'"
    lsp.send(r#"{"jsonrpc":"2.0","id":8,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///test.mano"},"position":{"line":0,"character":6}}}"#);
    let response = lsp.recv();

    assert!(response.contains(r#""id":8"#), "Expected hover response");

    lsp.shutdown();
}

#[test]
fn lsp_emoji_variable_hover() {
    let mut lsp = LspClient::spawn();
    lsp.initialize();

    lsp.send(r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///test.mano","languageId":"mano","version":1,"text":"seLiga 🔥 = 100;\nsalve 🔥;"}}}"#);
    let _ = lsp.recv(); // diagnostics

    lsp.send(r#"{"jsonrpc":"2.0","id":10,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///test.mano"},"position":{"line":1,"character":6}}}"#);
    let response = lsp.recv();

    assert!(response.contains(r#""id":10"#), "Expected hover response");
    assert!(
        response.contains("🔥"),
        "Expected hover to show emoji variable"
    );

    lsp.shutdown();
}

#[test]
fn lsp_emoji_variable_go_to_definition() {
    let mut lsp = LspClient::spawn();
    lsp.initialize();

    // 🔥 declared on line 0, used on line 1
    lsp.send(r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///test.mano","languageId":"mano","version":1,"text":"seLiga 🔥 = 100;\nsalve 🔥;"}}}"#);
    let _ = lsp.recv(); // diagnostics

    // Go to definition from usage on line 1
    lsp.send(r#"{"jsonrpc":"2.0","id":11,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///test.mano"},"position":{"line":1,"character":6}}}"#);
    let response = lsp.recv();

    assert!(
        response.contains(r#""id":11"#),
        "Expected definition response"
    );
    assert!(
        response.contains(r#""line":0"#),
        "Expected definition on line 0"
    );

    lsp.shutdown();
}

#[test]
fn lsp_emoji_variable_find_references() {
    let mut lsp = LspClient::spawn();
    lsp.initialize();

    // 🔥 declared on line 0, used on lines 1 and 2
    lsp.send(r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///test.mano","languageId":"mano","version":1,"text":"seLiga 🔥 = 100;\nsalve 🔥;\nsalve 🔥 + 1;"}}}"#);
    let _ = lsp.recv(); // diagnostics

    lsp.send(r#"{"jsonrpc":"2.0","id":12,"method":"textDocument/references","params":{"textDocument":{"uri":"file:///test.mano"},"position":{"line":1,"character":6},"context":{"includeDeclaration":true}}}"#);
    let response = lsp.recv();

    assert!(
        response.contains(r#""id":12"#),
        "Expected references response"
    );
    assert!(
        response.contains(r#""line":0"#),
        "Expected reference on line 0"
    );
    assert!(
        response.contains(r#""line":1"#),
        "Expected reference on line 1"
    );
    assert!(
        response.contains(r#""line":2"#),
        "Expected reference on line 2"
    );

    lsp.shutdown();
}

#[test]
fn lsp_rename() {
    let mut lsp = LspClient::spawn();
    lsp.initialize();

    // foo declared on line 0, used on line 1
    lsp.send(r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///test.mano","languageId":"mano","version":1,"text":"seLiga foo = 42;\nsalve foo;"}}}"#);
    let _ = lsp.recv(); // diagnostics

    // Rename "foo" to "bar"
    lsp.send(r#"{"jsonrpc":"2.0","id":13,"method":"textDocument/rename","params":{"textDocument":{"uri":"file:///test.mano"},"position":{"line":1,"character":6},"newName":"bar"}}"#);
    let response = lsp.recv();

    assert!(response.contains(r#""id":13"#), "Expected rename response");
    assert!(response.contains("bar"), "Expected new name in edits");
    assert!(response.contains(r#""line":0"#), "Expected edit on line 0");
    assert!(response.contains(r#""line":1"#), "Expected edit on line 1");

    lsp.shutdown();
}

#[test]
fn lsp_folding_ranges() {
    let mut lsp = LspClient::spawn();
    lsp.initialize();

    // Block from line 0-2
    lsp.send(r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///test.mano","languageId":"mano","version":1,"text":"sePá (firmeza) {\n    salve 42;\n}"}}}"#);
    let _ = lsp.recv(); // diagnostics

    lsp.send(r#"{"jsonrpc":"2.0","id":14,"method":"textDocument/foldingRange","params":{"textDocument":{"uri":"file:///test.mano"}}}"#);
    let response = lsp.recv();

    assert!(
        response.contains(r#""id":14"#),
        "Expected folding range response"
    );
    assert!(
        response.contains(r#""startLine":0"#),
        "Expected fold starting at line 0"
    );
    assert!(
        response.contains(r#""endLine":2"#),
        "Expected fold ending at line 2"
    );

    lsp.shutdown();
}

#[test]
fn lsp_prepare_rename() {
    let mut lsp = LspClient::spawn();
    lsp.initialize();

    // foo declared on line 0, used on line 1
    lsp.send(r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///test.mano","languageId":"mano","version":1,"text":"seLiga foo = 42;\nsalve foo;"}}}"#);
    let _ = lsp.recv(); // diagnostics

    // Request prepare rename on "foo"
    lsp.send(r#"{"jsonrpc":"2.0","id":16,"method":"textDocument/prepareRename","params":{"textDocument":{"uri":"file:///test.mano"},"position":{"line":1,"character":6}}}"#);
    let response = lsp.recv();

    assert!(
        response.contains(r#""id":16"#),
        "Expected prepare rename response"
    );
    // Should return the range of "foo" at line 1, character 6-9
    assert!(
        response.contains(r#""start""#),
        "Expected range start in response"
    );
    assert!(
        response.contains(r#""end""#),
        "Expected range end in response"
    );

    lsp.shutdown();
}

#[test]
fn lsp_folding_ranges_without_braces() {
    let mut lsp = LspClient::spawn();
    lsp.initialize();

    // If/else without braces spans lines 0-3
    lsp.send(r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///test.mano","languageId":"mano","version":1,"text":"sePá (firmeza)\n    salve 1;\nvacilou\n    salve 2;"}}}"#);
    let _ = lsp.recv(); // diagnostics

    lsp.send(r#"{"jsonrpc":"2.0","id":15,"method":"textDocument/foldingRange","params":{"textDocument":{"uri":"file:///test.mano"}}}"#);
    let response = lsp.recv();

    assert!(
        response.contains(r#""id":15"#),
        "Expected folding range response"
    );
    assert!(
        response.contains(r#""startLine":0"#),
        "Expected fold starting at line 0"
    );
    assert!(
        response.contains(r#""endLine":3"#),
        "Expected fold ending at line 3"
    );

    lsp.shutdown();
}

#[test]
fn lsp_folding_else_branch_without_braces() {
    let mut lsp = LspClient::spawn();
    lsp.initialize();

    // The else branch (vacilou) spanning lines 2-3 should be foldable separately
    // Line 0: sePá (firmeza)
    // Line 1:     salve 1;
    // Line 2: vacilou
    // Line 3:     salve 2;
    lsp.send(r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///test.mano","languageId":"mano","version":1,"text":"sePá (firmeza)\n    salve 1;\nvacilou\n    salve 2;"}}}"#);
    let _ = lsp.recv(); // diagnostics

    lsp.send(r#"{"jsonrpc":"2.0","id":17,"method":"textDocument/foldingRange","params":{"textDocument":{"uri":"file:///test.mano"}}}"#);
    let response = lsp.recv();

    assert!(
        response.contains(r#""id":17"#),
        "Expected folding range response"
    );
    // Should have a fold for the else branch starting at line 2
    assert!(
        response.contains(r#""startLine":2"#),
        "Expected fold for vacilou starting at line 2. Response: {}",
        response
    );

    lsp.shutdown();
}
