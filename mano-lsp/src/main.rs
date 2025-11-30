use std::collections::HashMap;
use std::error::Error;

use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams, Diagnostic,
    DiagnosticSeverity, DocumentSymbolParams, DocumentSymbolResponse, FoldingRange,
    FoldingRangeParams, GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverContents,
    HoverParams, HoverProviderCapability, InitializeParams, Location, MarkupContent, MarkupKind,
    OneOf, Position, PublishDiagnosticsParams, Range, ReferenceParams, RenameParams,
    ServerCapabilities, SymbolInformation, SymbolKind, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextEdit, Uri, WorkspaceEdit,
    notification::{DidChangeTextDocument, DidOpenTextDocument, Notification as _},
    request::{
        Completion, DocumentSymbolRequest, FoldingRangeRequest, GotoDefinition, HoverRequest,
        PrepareRenameRequest, References, Rename, Request as _,
    },
};
use mano::{KEYWORDS, ManoError, Parser, Scanner, Stmt, TokenType, is_identifier_char};

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    eprintln!("mano-lsp starting...");

    let (connection, io_threads) = Connection::stdio();

    let server_capabilities = serde_json::to_value(ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        completion_provider: Some(CompletionOptions::default()),
        definition_provider: Some(OneOf::Left(true)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        rename_provider: Some(OneOf::Right(lsp_types::RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: Default::default(),
        })),
        folding_range_provider: Some(lsp_types::FoldingRangeProviderCapability::Simple(true)),
        ..Default::default()
    })?;

    let initialization_params = match connection.initialize(server_capabilities) {
        Ok(it) => it,
        Err(e) => {
            if e.channel_is_disconnected() {
                io_threads.join()?;
            }
            return Err(e.into());
        }
    };

    let _: InitializeParams = serde_json::from_value(initialization_params)?;
    eprintln!("mano-lsp initialized!");

    main_loop(connection)?;
    io_threads.join()?;

    eprintln!("mano-lsp shutting down.");
    Ok(())
}

fn main_loop(connection: Connection) -> Result<(), Box<dyn Error + Sync + Send>> {
    let mut documents: HashMap<String, String> = HashMap::new();

    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                handle_request(&connection, req, &documents)?;
            }
            Message::Response(Response { .. }) => {}
            Message::Notification(not) => {
                handle_notification(&connection, not, &mut documents)?;
            }
        }
    }
    Ok(())
}

fn handle_request(
    connection: &Connection,
    req: Request,
    documents: &HashMap<String, String>,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    if req.method == Completion::METHOD {
        let params: CompletionParams = serde_json::from_value(req.params)?;
        let uri = params.text_document_position.text_document.uri.to_string();
        let position = params.text_document_position.position;

        let completions = if let Some(source) = documents.get(&uri) {
            let prefix = get_prefix_at_position(source, position);
            get_completions(source, &prefix)
        } else {
            vec![]
        };

        let response = Response::new_ok(req.id, completions);
        connection.sender.send(Message::Response(response))?;
    } else if req.method == GotoDefinition::METHOD {
        let params: GotoDefinitionParams = serde_json::from_value(req.params)?;
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let result = documents.get(&uri.to_string()).and_then(|source| {
            find_definition(source, position).map(|range| {
                GotoDefinitionResponse::Scalar(Location {
                    uri: uri.clone(),
                    range,
                })
            })
        });

        let response = Response::new_ok(req.id, result);
        connection.sender.send(Message::Response(response))?;
    } else if req.method == HoverRequest::METHOD {
        let params: HoverParams = serde_json::from_value(req.params)?;
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let result = documents
            .get(&uri.to_string())
            .and_then(|source| create_hover_response(source, position));

        let response = Response::new_ok(req.id, result);
        connection.sender.send(Message::Response(response))?;
    } else if req.method == DocumentSymbolRequest::METHOD {
        let params: DocumentSymbolParams = serde_json::from_value(req.params)?;
        let uri = params.text_document.uri;

        let result = documents
            .get(&uri.to_string())
            .map(|source| DocumentSymbolResponse::Flat(get_document_symbols(source, uri.clone())));

        let response = Response::new_ok(req.id, result);
        connection.sender.send(Message::Response(response))?;
    } else if req.method == References::METHOD {
        let params: ReferenceParams = serde_json::from_value(req.params)?;
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let result = documents
            .get(&uri.to_string())
            .map(|source| find_references(source, position, uri.clone()));

        let response = Response::new_ok(req.id, result);
        connection.sender.send(Message::Response(response))?;
    } else if req.method == Rename::METHOD {
        let params: RenameParams = serde_json::from_value(req.params)?;
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let new_name = params.new_name;

        let result = documents.get(&uri.to_string()).map(|source| {
            let edits = get_rename_edits(source, position, &new_name, uri.clone());
            WorkspaceEdit {
                changes: Some(HashMap::from([(uri, edits)])),
                ..Default::default()
            }
        });

        let response = Response::new_ok(req.id, result);
        connection.sender.send(Message::Response(response))?;
    } else if req.method == PrepareRenameRequest::METHOD {
        let params: lsp_types::TextDocumentPositionParams = serde_json::from_value(req.params)?;
        let uri = params.text_document.uri;
        let position = params.position;

        let result = documents
            .get(&uri.to_string())
            .and_then(|source| prepare_rename(source, position));

        let response = Response::new_ok(req.id, result);
        connection.sender.send(Message::Response(response))?;
    } else if req.method == FoldingRangeRequest::METHOD {
        let params: FoldingRangeParams = serde_json::from_value(req.params)?;
        let uri = params.text_document.uri;

        let result = documents
            .get(&uri.to_string())
            .map(|source| get_folding_ranges(source));

        let response = Response::new_ok(req.id, result);
        connection.sender.send(Message::Response(response))?;
    }
    Ok(())
}

fn handle_notification(
    connection: &Connection,
    not: Notification,
    documents: &mut HashMap<String, String>,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    match not.method.as_str() {
        DidOpenTextDocument::METHOD => {
            let params: lsp_types::DidOpenTextDocumentParams = serde_json::from_value(not.params)?;
            let uri = params.text_document.uri.to_string();
            let text = params.text_document.text.clone();
            documents.insert(uri, text);
            publish_diagnostics(
                connection,
                params.text_document.uri,
                &params.text_document.text,
            )?;
        }
        DidChangeTextDocument::METHOD => {
            let params: lsp_types::DidChangeTextDocumentParams =
                serde_json::from_value(not.params)?;
            if let Some(change) = params.content_changes.into_iter().next() {
                documents.insert(params.text_document.uri.to_string(), change.text.clone());
                publish_diagnostics(connection, params.text_document.uri, &change.text)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn publish_diagnostics(
    connection: &Connection,
    uri: Uri,
    source: &str,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let diagnostics = get_diagnostics(source);

    let params = PublishDiagnosticsParams {
        uri,
        diagnostics,
        version: None,
    };

    let notification = Notification::new(
        lsp_types::notification::PublishDiagnostics::METHOD.to_string(),
        params,
    );

    connection
        .sender
        .send(Message::Notification(notification))?;
    Ok(())
}

fn byte_offset_to_position(source: &str, byte_offset: usize) -> Position {
    let prefix = &source[..byte_offset.min(source.len())];
    let line = prefix.matches('\n').count() as u32;
    let col = prefix
        .rfind('\n')
        .map_or(prefix.len(), |i| prefix.len() - i - 1) as u32;
    Position::new(line, col)
}

fn get_diagnostics(source: &str) -> Vec<Diagnostic> {
    let scanner = Scanner::new(source);
    let results: Vec<_> = scanner.collect();

    let mut diagnostics = Vec::new();

    for result in &results {
        if let Err(ManoError::Scan { message, span }) = result {
            diagnostics.push(to_lsp_diagnostic(message, span, source));
        }
    }

    let valid_tokens: Vec<_> = results.into_iter().filter_map(|r| r.ok()).collect();
    let mut parser = Parser::new(valid_tokens);
    let _ = parser.parse();

    for error in parser.take_errors() {
        if let ManoError::Parse { message, span } = error {
            diagnostics.push(to_lsp_diagnostic(&message, &span, source));
        }
    }

    diagnostics
}

fn to_lsp_diagnostic(message: &str, span: &std::ops::Range<usize>, source: &str) -> Diagnostic {
    let start = byte_offset_to_position(source, span.start);
    let end = byte_offset_to_position(source, span.end);

    Diagnostic {
        range: Range { start, end },
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("mano".to_string()),
        message: message.to_string(),
        ..Default::default()
    }
}

fn get_prefix_at_position(source: &str, position: Position) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let line_idx = position.line as usize;

    if line_idx >= lines.len() {
        return String::new();
    }

    let line = lines[line_idx];
    let col = position.character as usize;
    let col = col.min(line.len());

    // Walk backwards from cursor to find start of identifier
    let prefix_start = line[..col]
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);

    line[prefix_start..col].to_string()
}

fn get_completions(source: &str, prefix: &str) -> Vec<CompletionItem> {
    let mut completions = Vec::new();

    // Add keywords
    for (keyword, _) in KEYWORDS {
        if keyword.starts_with(prefix) {
            completions.push(CompletionItem {
                label: keyword.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                ..Default::default()
            });
        }
    }

    // Add variables from parsed AST
    for var in extract_variables(source) {
        if var.starts_with(prefix) {
            completions.push(CompletionItem {
                label: var,
                kind: Some(CompletionItemKind::VARIABLE),
                ..Default::default()
            });
        }
    }

    completions
}

fn extract_variables(source: &str) -> Vec<String> {
    extract_variable_declarations(source)
        .into_iter()
        .map(|(name, _)| name)
        .collect()
}

fn extract_variable_declarations(source: &str) -> Vec<(String, std::ops::Range<usize>)> {
    let scanner = Scanner::new(source);
    let tokens: Vec<_> = scanner.filter_map(|r| r.ok()).collect();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap_or_default();

    let mut declarations = Vec::new();
    collect_variable_declarations(&statements, &mut declarations);
    declarations
}

fn collect_variable_declarations(
    statements: &[Stmt],
    declarations: &mut Vec<(String, std::ops::Range<usize>)>,
) {
    for stmt in statements {
        if let Some((name, _)) = stmt.var_declaration() {
            declarations.push((name.lexeme.clone(), name.span.clone()));
        }
        for child in stmt.children() {
            collect_variable_declarations(std::slice::from_ref(child), declarations);
        }
    }
}

fn find_definition(source: &str, position: Position) -> Option<Range> {
    let word = get_word_at_position(source, position)?;

    for (name, span) in extract_variable_declarations(source) {
        if name == word {
            let start = byte_offset_to_position(source, span.start);
            let end = byte_offset_to_position(source, span.end);
            return Some(Range { start, end });
        }
    }

    None
}

fn get_word_at_position(source: &str, position: Position) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    let line_idx = position.line as usize;
    if line_idx >= lines.len() {
        return None;
    }

    let line = lines[line_idx];

    // Convert UTF-16 column to byte offset
    let mut byte_offset = 0;
    let mut utf16_offset = 0;
    for c in line.chars() {
        if utf16_offset >= position.character as usize {
            break;
        }
        utf16_offset += c.len_utf16();
        byte_offset += c.len_utf8();
    }
    byte_offset = byte_offset.min(line.len());

    // Find start of word (walk backwards)
    let mut start = byte_offset;
    for (i, c) in line[..byte_offset].char_indices().rev() {
        if !is_identifier_char(c) {
            start = i + c.len_utf8();
            break;
        }
        start = i;
    }

    // Find end of word (walk forwards)
    let mut end = byte_offset;
    for (i, c) in line[byte_offset..].char_indices() {
        if !is_identifier_char(c) {
            end = byte_offset + i;
            break;
        }
        end = byte_offset + i + c.len_utf8();
    }

    if start >= end {
        return None;
    }

    Some(line[start..end].to_string())
}

fn get_folding_ranges(source: &str) -> Vec<FoldingRange> {
    let scanner = Scanner::new(source);
    let tokens: Vec<_> = scanner.filter_map(|r| r.ok()).collect();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap_or_default();

    let mut ranges = Vec::new();
    collect_folding_ranges(&statements, source, &mut ranges);
    ranges
}

fn collect_folding_ranges(statements: &[Stmt], source: &str, ranges: &mut Vec<FoldingRange>) {
    for stmt in statements {
        let span = stmt.span();
        let start_pos = byte_offset_to_position(source, span.start);
        let end_pos = byte_offset_to_position(source, span.end);

        if end_pos.line > start_pos.line {
            ranges.push(FoldingRange {
                start_line: start_pos.line,
                start_character: Some(start_pos.character),
                end_line: end_pos.line,
                end_character: Some(end_pos.character),
                kind: None,
                collapsed_text: None,
            });
        }

        for child in stmt.children() {
            collect_folding_ranges(std::slice::from_ref(child), source, ranges);
        }
    }
}

fn prepare_rename(source: &str, position: Position) -> Option<Range> {
    let word = get_word_at_position(source, position)?;

    // Check if it's a declared variable (not a keyword)
    let declarations = extract_variable_declarations(source);
    if !declarations.iter().any(|(name, _)| name == &word) {
        return None;
    }

    // Find the reference at this position and return its range
    let refs = find_references(source, position, "file:///dummy".parse().unwrap());
    refs.into_iter()
        .find(|loc| {
            loc.range.start.line == position.line
                && loc.range.start.character <= position.character
                && loc.range.end.character >= position.character
        })
        .map(|loc| loc.range)
}

fn get_rename_edits(source: &str, position: Position, new_name: &str, uri: Uri) -> Vec<TextEdit> {
    find_references(source, position, uri)
        .into_iter()
        .map(|loc| TextEdit {
            range: loc.range,
            new_text: new_name.to_string(),
        })
        .collect()
}

fn find_references(source: &str, position: Position, uri: Uri) -> Vec<Location> {
    let word = match get_word_at_position(source, position) {
        Some(w) => w,
        None => return vec![],
    };

    // Check if this word is a declared variable
    let declarations = extract_variable_declarations(source);
    let is_variable = declarations.iter().any(|(name, _)| name == &word);
    if !is_variable {
        return vec![];
    }

    // Find all identifier tokens matching this name
    let scanner = Scanner::new(source);
    scanner
        .filter_map(|r| r.ok())
        .filter(|token| token.token_type == TokenType::Identifier && token.lexeme == word)
        .map(|token| {
            let start = byte_offset_to_position(source, token.span.start);
            let end = byte_offset_to_position(source, token.span.end);
            Location {
                uri: uri.clone(),
                range: Range { start, end },
            }
        })
        .collect()
}

#[allow(deprecated)] // SymbolInformation is deprecated but DocumentSymbol requires hierarchy
fn get_document_symbols(source: &str, uri: Uri) -> Vec<SymbolInformation> {
    extract_variable_declarations(source)
        .into_iter()
        .map(|(name, span)| {
            let start = byte_offset_to_position(source, span.start);
            let end = byte_offset_to_position(source, span.end);
            SymbolInformation {
                name,
                kind: SymbolKind::VARIABLE,
                location: Location {
                    uri: uri.clone(),
                    range: Range { start, end },
                },
                tags: None,
                deprecated: None,
                container_name: None,
            }
        })
        .collect()
}

fn create_hover_response(source: &str, position: Position) -> Option<Hover> {
    let text = get_hover(source, position)?;
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: text,
        }),
        range: None,
    })
}

fn get_hover(source: &str, position: Position) -> Option<String> {
    let word = get_word_at_position(source, position)?;

    // Check if it's a keyword
    for (keyword, _token_type) in KEYWORDS {
        if *keyword == word {
            return Some(format!("`{}` (keyword)", keyword));
        }
    }

    // Check if it's a variable
    let variables = extract_variables(source);
    if variables.contains(&word) {
        return Some(format!("`{}` (variable)", word));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_offset_at_start_is_line_0_col_0() {
        let pos = byte_offset_to_position("hello", 0);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.character, 0);
    }

    #[test]
    fn byte_offset_in_first_line() {
        let pos = byte_offset_to_position("hello world", 6);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.character, 6);
    }

    #[test]
    fn byte_offset_after_newline_is_next_line() {
        let pos = byte_offset_to_position("hello\nworld", 6);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.character, 0);
    }

    #[test]
    fn byte_offset_middle_of_second_line() {
        let pos = byte_offset_to_position("hello\nworld", 8);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.character, 2);
    }

    #[test]
    fn valid_code_produces_no_diagnostics() {
        let diagnostics = get_diagnostics("salve 42;");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn scan_error_produces_diagnostic() {
        let diagnostics = get_diagnostics("@");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains('@'));
    }

    #[test]
    fn parse_error_produces_diagnostic() {
        let diagnostics = get_diagnostics("salve");
        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn multiple_scan_errors_produce_multiple_diagnostics() {
        let diagnostics = get_diagnostics("@$");
        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn to_lsp_diagnostic_sets_error_severity() {
        let diag = to_lsp_diagnostic("test", &(0..1), "x");
        assert_eq!(diag.severity, Some(DiagnosticSeverity::ERROR));
    }

    #[test]
    fn to_lsp_diagnostic_sets_source_to_mano() {
        let diag = to_lsp_diagnostic("test", &(0..1), "x");
        assert_eq!(diag.source, Some("mano".to_string()));
    }

    #[test]
    fn to_lsp_diagnostic_preserves_message() {
        let diag = to_lsp_diagnostic("E esse '@' aí?", &(0..1), "x");
        assert_eq!(diag.message, "E esse '@' aí?");
    }

    #[test]
    fn to_lsp_diagnostic_converts_span_to_range() {
        let source = "hello\nworld";
        let diag = to_lsp_diagnostic("err", &(6..11), source);
        assert_eq!(diag.range.start, Position::new(1, 0));
        assert_eq!(diag.range.end, Position::new(1, 5));
    }

    #[test]
    fn get_completions_returns_all_keywords_for_empty_prefix() {
        let completions = get_completions("", "");
        assert!(completions.len() >= 18); // All keywords
    }

    #[test]
    fn get_completions_filters_by_prefix() {
        let completions = get_completions("", "sal");
        assert!(completions.iter().any(|c| c.label == "salve"));
        assert!(!completions.iter().any(|c| c.label == "seLiga"));
    }

    #[test]
    fn get_completions_prefix_se_matches_multiple() {
        let completions = get_completions("", "se");
        assert!(completions.iter().any(|c| c.label == "seLiga"));
        assert!(completions.iter().any(|c| c.label == "sePá"));
        assert!(completions.iter().any(|c| c.label == "seVira"));
        assert!(completions.iter().any(|c| c.label == "segueOFluxo"));
    }

    #[test]
    fn completion_item_has_keyword_kind() {
        let completions = get_completions("", "");
        let salve = completions.iter().find(|c| c.label == "salve").unwrap();
        assert_eq!(salve.kind, Some(lsp_types::CompletionItemKind::KEYWORD));
    }

    #[test]
    fn get_completions_includes_variables_from_source() {
        let completions = get_completions("seLiga meuNome = 42;", "meu");
        assert!(completions.iter().any(|c| c.label == "meuNome"));
    }

    #[test]
    fn get_completions_variable_has_variable_kind() {
        let completions = get_completions("seLiga x = 1;", "x");
        let x = completions.iter().find(|c| c.label == "x").unwrap();
        assert_eq!(x.kind, Some(lsp_types::CompletionItemKind::VARIABLE));
    }

    #[test]
    fn get_completions_excludes_variables_not_matching_prefix() {
        let completions = get_completions("seLiga foo = 1; seLiga bar = 2;", "fo");
        assert!(completions.iter().any(|c| c.label == "foo"));
        assert!(!completions.iter().any(|c| c.label == "bar"));
    }

    #[test]
    fn get_completions_finds_variables_in_blocks() {
        let completions = get_completions("{ seLiga inner = 1; }", "inn");
        assert!(completions.iter().any(|c| c.label == "inner"));
    }

    #[test]
    fn get_completions_finds_variables_in_if_then_branch() {
        let completions = get_completions("sePá (firmeza) { seLiga thenVar = 1; }", "then");
        assert!(completions.iter().any(|c| c.label == "thenVar"));
    }

    #[test]
    fn get_completions_finds_variables_in_if_else_branch() {
        let completions =
            get_completions("sePá (firmeza) { } vacilou { seLiga elseVar = 2; }", "else");
        assert!(completions.iter().any(|c| c.label == "elseVar"));
    }

    #[test]
    fn get_completions_finds_variables_in_while_body() {
        let completions = get_completions("segueOFluxo (firmeza) { seLiga loopVar = 1; }", "loop");
        assert!(completions.iter().any(|c| c.label == "loopVar"));
    }

    #[test]
    fn find_definition_returns_none_for_empty_source() {
        let result = find_definition("", Position::new(0, 0));
        assert!(result.is_none());
    }

    #[test]
    fn find_definition_returns_none_when_not_on_variable() {
        let result = find_definition("salve 42;", Position::new(0, 0));
        assert!(result.is_none());
    }

    #[test]
    fn find_definition_finds_variable_declaration() {
        // "seLiga foo = 42;\nsalve foo;"
        //        ^foo at col 7      ^foo at line 1, col 6
        let source = "seLiga foo = 42;\nsalve foo;";
        let result = find_definition(source, Position::new(1, 6));
        assert!(result.is_some());
        let range = result.unwrap();
        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.character, 7); // "foo" starts at col 7
    }

    #[test]
    fn find_definition_on_declaration_returns_itself() {
        let source = "seLiga foo = 42;";
        let result = find_definition(source, Position::new(0, 7));
        assert!(result.is_some());
        let range = result.unwrap();
        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.character, 7);
    }

    #[test]
    fn get_hover_returns_none_for_empty_source() {
        let result = get_hover("", Position::new(0, 0));
        assert!(result.is_none());
    }

    #[test]
    fn get_hover_returns_keyword_info() {
        // "salve" at col 0
        let result = get_hover("salve 42;", Position::new(0, 0));
        assert!(result.is_some());
        assert!(result.unwrap().contains("salve"));
    }

    #[test]
    fn get_hover_returns_variable_info() {
        let source = "seLiga foo = 42;\nsalve foo;";
        let result = get_hover(source, Position::new(1, 6));
        assert!(result.is_some());
        assert!(result.unwrap().contains("foo"));
    }

    #[test]
    fn get_hover_returns_none_on_number() {
        let result = get_hover("salve 42;", Position::new(0, 6));
        assert!(result.is_none());
    }

    #[test]
    fn create_hover_response_returns_none_for_empty_source() {
        let result = create_hover_response("", Position::new(0, 0));
        assert!(result.is_none());
    }

    #[test]
    fn create_hover_response_returns_hover_for_keyword() {
        let result = create_hover_response("salve 42;", Position::new(0, 0));
        assert!(result.is_some());
        let hover = result.unwrap();
        match hover.contents {
            HoverContents::Markup(markup) => {
                assert_eq!(markup.kind, MarkupKind::Markdown);
                assert!(markup.value.contains("salve"));
            }
            _ => panic!("Expected Markup content"),
        }
    }

    #[test]
    fn create_hover_response_returns_hover_for_variable() {
        let source = "seLiga foo = 42;\nsalve foo;";
        let result = create_hover_response(source, Position::new(1, 6));
        assert!(result.is_some());
        let hover = result.unwrap();
        match hover.contents {
            HoverContents::Markup(markup) => {
                assert!(markup.value.contains("foo"));
            }
            _ => panic!("Expected Markup content"),
        }
    }

    fn test_uri() -> Uri {
        "file:///test.mano".parse().unwrap()
    }

    #[test]
    fn get_document_symbols_returns_empty_for_no_variables() {
        let result = get_document_symbols("", test_uri());
        assert!(result.is_empty());
    }

    #[test]
    fn get_document_symbols_returns_variable_symbol() {
        let result = get_document_symbols("seLiga foo = 42;", test_uri());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "foo");
        assert_eq!(result[0].kind, SymbolKind::VARIABLE);
    }

    #[test]
    fn get_document_symbols_returns_multiple_variables() {
        let result = get_document_symbols("seLiga a = 1;\nseLiga b = 2;", test_uri());
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|s| s.name == "a"));
        assert!(result.iter().any(|s| s.name == "b"));
    }

    #[test]
    fn get_document_symbols_includes_correct_range() {
        let result = get_document_symbols("seLiga foo = 42;", test_uri());
        assert_eq!(result[0].location.range.start.line, 0);
        assert_eq!(result[0].location.range.start.character, 7); // "foo" starts at col 7
    }

    #[test]
    fn find_references_returns_empty_for_no_match() {
        let result = find_references("salve 42;", Position::new(0, 0), test_uri());
        assert!(result.is_empty());
    }

    #[test]
    fn find_references_finds_declaration() {
        let result = find_references("seLiga foo = 42;", Position::new(0, 7), test_uri());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].range.start.line, 0);
    }

    #[test]
    fn find_references_finds_declaration_and_usage() {
        let source = "seLiga foo = 42;\nsalve foo;";
        let result = find_references(source, Position::new(1, 6), test_uri());
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn find_references_finds_multiple_usages() {
        let source = "seLiga foo = 42;\nsalve foo;\nsalve foo + 1;";
        let result = find_references(source, Position::new(0, 7), test_uri());
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn get_hover_works_with_emoji_variable() {
        let source = "seLiga 🔥 = 42;\nsalve 🔥;";
        let result = get_hover(source, Position::new(1, 6));
        assert!(result.is_some(), "Hover should work for emoji variable");
        assert!(result.unwrap().contains("🔥"));
    }

    #[test]
    fn find_definition_works_with_emoji_variable() {
        let source = "seLiga 🔥 = 42;\nsalve 🔥;";
        let result = find_definition(source, Position::new(1, 6));
        assert!(
            result.is_some(),
            "Definition should work for emoji variable"
        );
    }

    #[test]
    fn find_references_works_with_emoji_variable() {
        let source = "seLiga 🔥 = 42;\nsalve 🔥;";
        let result = find_references(source, Position::new(1, 6), test_uri());
        assert_eq!(
            result.len(),
            2,
            "Should find 2 references for emoji variable"
        );
    }

    #[test]
    fn prepare_rename_returns_none_for_non_variable() {
        let result = prepare_rename("salve 42;", Position::new(0, 0));
        assert!(result.is_none());
    }

    #[test]
    fn prepare_rename_returns_range_for_variable() {
        let source = "seLiga foo = 42;\nsalve foo;";
        let result = prepare_rename(source, Position::new(1, 6));
        assert!(result.is_some());
    }

    #[test]
    fn prepare_rename_works_with_emoji_variable() {
        let source = "seLiga 🔥 = 42;\nsalve 🔥;";
        let result = prepare_rename(source, Position::new(1, 6));
        assert!(result.is_some(), "Should be able to rename emoji variable");
    }

    #[test]
    fn get_rename_edits_returns_empty_for_non_variable() {
        let result = get_rename_edits("salve 42;", Position::new(0, 0), "bar", test_uri());
        assert!(result.is_empty());
    }

    #[test]
    fn get_rename_edits_returns_edits_for_all_references() {
        let source = "seLiga foo = 42;\nsalve foo;";
        let result = get_rename_edits(source, Position::new(1, 6), "bar", test_uri());
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|edit| edit.new_text == "bar"));
    }

    #[test]
    fn get_rename_edits_works_with_emoji_variable() {
        let source = "seLiga 🔥 = 42;\nsalve 🔥;";
        let result = get_rename_edits(source, Position::new(1, 6), "fire", test_uri());
        assert_eq!(result.len(), 2, "Should find 2 edits for emoji variable");
        assert!(result.iter().all(|edit| edit.new_text == "fire"));
    }

    #[test]
    fn get_folding_ranges_returns_empty_for_no_blocks() {
        let result = get_folding_ranges("salve 42;");
        assert!(result.is_empty());
    }

    #[test]
    fn get_folding_ranges_returns_range_for_if_block() {
        let source = "sePá (firmeza) {\n    salve 42;\n}";
        let result = get_folding_ranges(source);
        // 2 folds: If statement (0-2) and Block inside (0-2)
        assert_eq!(result.len(), 2);
        // Both span lines 0-2
        assert!(result.iter().all(|r| r.start_line == 0 && r.end_line == 2));
    }

    #[test]
    fn get_folding_ranges_returns_range_for_while_block() {
        let source = "segueOFluxo (firmeza) {\n    salve 1;\n}";
        let result = get_folding_ranges(source);
        // 2 folds: While statement (0-2) and Block inside (0-2)
        assert_eq!(result.len(), 2);
        // Both span lines 0-2
        assert!(result.iter().all(|r| r.start_line == 0 && r.end_line == 2));
    }

    #[test]
    fn get_folding_ranges_returns_nested_ranges() {
        let source = "sePá (firmeza) {\n    sePá (firmeza) {\n        salve 1;\n    }\n}";
        let result = get_folding_ranges(source);
        // 4 folds: outer If, outer Block, inner If, inner Block
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn get_folding_ranges_folds_if_without_braces() {
        let source = "sePá (firmeza)\n    salve 42;";
        let result = get_folding_ranges(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].start_line, 0);
        assert_eq!(result[0].end_line, 1);
    }

    #[test]
    fn get_folding_ranges_folds_if_else_without_braces() {
        // Line 0: sePá (firmeza)
        // Line 1:     salve 1;
        // Line 2: vacilou
        // Line 3:     salve 2;
        let source = "sePá (firmeza)\n    salve 1;\nvacilou\n    salve 2;";
        let result = get_folding_ranges(source);

        // Should have 2 folds: whole if-else (0-3) and else branch (2-3)
        assert_eq!(result.len(), 2, "Expected 2 folds, got {:?}", result);

        // Find the fold for the whole if-else
        let if_else_fold = result.iter().find(|r| r.start_line == 0);
        assert!(
            if_else_fold.is_some(),
            "Expected fold starting at line 0 for if-else"
        );
        assert_eq!(if_else_fold.unwrap().end_line, 3);

        // Find the fold for the else branch
        let else_fold = result.iter().find(|r| r.start_line == 2);
        assert!(
            else_fold.is_some(),
            "Expected fold starting at line 2 for vacilou"
        );
        assert_eq!(else_fold.unwrap().end_line, 3);
    }

    #[test]
    fn get_folding_ranges_folds_while_without_braces() {
        let source = "segueOFluxo (firmeza)\n    salve 1;";
        let result = get_folding_ranges(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].start_line, 0);
        assert_eq!(result[0].end_line, 1);
    }

    // === edge case tests for coverage ===

    #[test]
    fn get_prefix_at_position_returns_empty_for_line_beyond_source() {
        let source = "salve 1;";
        let prefix = get_prefix_at_position(source, Position::new(5, 0));
        assert_eq!(prefix, "");
    }

    #[test]
    fn get_word_at_position_returns_word_at_line_start() {
        let source = "foo = 42;";
        let word = get_word_at_position(source, Position::new(0, 1));
        assert_eq!(word, Some("foo".to_string()));
    }

    #[test]
    fn get_word_at_position_returns_none_for_empty_position() {
        let source = "   ";
        let word = get_word_at_position(source, Position::new(0, 1));
        assert_eq!(word, None);
    }

    #[test]
    fn find_references_returns_empty_when_not_on_word() {
        let source = "   ";
        let result = find_references(source, Position::new(0, 1), test_uri()); // in spaces
        assert!(result.is_empty());
    }
}
