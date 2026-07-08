use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};

use clap::Parser;
use pkl_lsp_core::{
    CompletionOptions, DocumentStore, FeatureEngine, Location, TextPosition, TextRange,
};
use serde_json::{Value, json};

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Args {
    /// Run as a stdio LSP server.
    #[arg(long)]
    stdio: bool,
}

struct Server {
    engine: FeatureEngine,
    shutdown_requested: bool,
}

impl Server {
    fn new() -> Self {
        Self {
            engine: FeatureEngine::new(DocumentStore::default()),
            shutdown_requested: false,
        }
    }

    fn handle(&mut self, message: Value, out: &mut impl Write) -> anyhow::Result<bool> {
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let id = message.get("id").cloned();
        let params = message.get("params").cloned().unwrap_or(Value::Null);

        match method {
            "initialize" => self.respond(out, id, initialize_result()),
            "initialized" => Ok(()),
            "shutdown" => {
                self.shutdown_requested = true;
                self.respond(out, id, Value::Null)
            }
            "exit" => Ok(()),
            "textDocument/didOpen" => {
                let uri = required_text_document_uri(&params)?;
                let text = params
                    .pointer("/textDocument/text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let version = params
                    .pointer("/textDocument/version")
                    .and_then(Value::as_i64)
                    .unwrap_or(0) as i32;
                self.engine.documents_mut().open(uri.clone(), text, version);
                self.engine.rebuild();
                self.publish_diagnostics(out, &uri)?;
                Ok(())
            }
            "textDocument/didChange" => {
                let uri = required_text_document_uri(&params)?;
                let version = params
                    .pointer("/textDocument/version")
                    .and_then(Value::as_i64)
                    .unwrap_or(0) as i32;
                if let Some(text) = params
                    .get("contentChanges")
                    .and_then(Value::as_array)
                    .and_then(|changes| changes.last())
                    .and_then(|change| change.get("text"))
                    .and_then(Value::as_str)
                {
                    self.engine
                        .documents_mut()
                        .change(uri.clone(), text.to_string(), version);
                    self.engine.rebuild();
                    self.publish_diagnostics(out, &uri)?;
                }
                Ok(())
            }
            "textDocument/didClose" => {
                let uri = required_text_document_uri(&params)?;
                self.engine.documents_mut().close(&uri);
                self.engine.rebuild();
                self.write_notification(
                    out,
                    "textDocument/publishDiagnostics",
                    json!({ "uri": uri, "diagnostics": [] }),
                )
            }
            "textDocument/documentSymbol" => {
                let uri = required_text_document_uri(&params)?;
                self.respond(
                    out,
                    id,
                    serde_json::to_value(self.engine.document_symbols(&uri))?,
                )
            }
            "workspace/symbol" => {
                let query = params
                    .get("query")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                self.respond(
                    out,
                    id,
                    serde_json::to_value(self.engine.workspace_symbols(query))?,
                )
            }
            "textDocument/hover" => {
                let (uri, position) = required_text_document_position(&params)?;
                self.respond(
                    out,
                    id,
                    serde_json::to_value(self.engine.hover(&uri, position))?,
                )
            }
            "textDocument/completion" => {
                let (uri, position) = required_text_document_position(&params)?;
                let completion = self.engine.completion(
                    &uri,
                    position,
                    CompletionOptions {
                        include_keywords: true,
                    },
                );
                self.respond(out, id, serde_json::to_value(completion.items)?)
            }
            "textDocument/definition" => {
                let (uri, position) = required_text_document_position(&params)?;
                self.respond_location(out, id, self.engine.definition(&uri, position))
            }
            "textDocument/typeDefinition" => {
                let (uri, position) = required_text_document_position(&params)?;
                self.respond_location(out, id, self.engine.type_definition(&uri, position))
            }
            "textDocument/implementation" => {
                let (uri, position) = required_text_document_position(&params)?;
                self.respond(
                    out,
                    id,
                    serde_json::to_value(self.engine.implementation(&uri, position))?,
                )
            }
            "textDocument/references" => {
                let (uri, position) = required_text_document_position(&params)?;
                self.respond(
                    out,
                    id,
                    serde_json::to_value(self.engine.references(&uri, position))?,
                )
            }
            "textDocument/documentHighlight" => {
                let (uri, position) = required_text_document_position(&params)?;
                self.respond(
                    out,
                    id,
                    serde_json::to_value(self.engine.document_highlights(&uri, position))?,
                )
            }
            "textDocument/semanticTokens/full" => {
                let uri = required_text_document_uri(&params)?;
                self.respond(
                    out,
                    id,
                    json!({ "data": encode_semantic_tokens(self.engine.semantic_tokens(&uri, None).data) }),
                )
            }
            "textDocument/semanticTokens/range" => {
                let uri = required_text_document_uri(&params)?;
                let range = required_range(params.get("range").unwrap_or(&Value::Null))?;
                self.respond(
                    out,
                    id,
                    json!({ "data": encode_semantic_tokens(self.engine.semantic_tokens(&uri, Some(range)).data) }),
                )
            }
            "textDocument/foldingRange" => {
                let uri = required_text_document_uri(&params)?;
                self.respond(
                    out,
                    id,
                    serde_json::to_value(self.engine.folding_ranges(&uri))?,
                )
            }
            "textDocument/selectionRange" => {
                let uri = required_text_document_uri(&params)?;
                let positions = params
                    .get("positions")
                    .and_then(Value::as_array)
                    .map(|values| values.iter().filter_map(parse_position).collect::<Vec<_>>())
                    .unwrap_or_default();
                self.respond(
                    out,
                    id,
                    serde_json::to_value(self.engine.selection_ranges(&uri, &positions))?,
                )
            }
            "textDocument/prepareRename" => {
                let (uri, position) = required_text_document_position(&params)?;
                self.respond(
                    out,
                    id,
                    serde_json::to_value(self.engine.prepare_rename(&uri, position))?,
                )
            }
            "textDocument/rename" => {
                let (uri, position) = required_text_document_position(&params)?;
                let new_name = params
                    .get("newName")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let edit = self
                    .engine
                    .rename(&uri, position, new_name)
                    .map(workspace_edit_to_lsp);
                self.respond(out, id, serde_json::to_value(edit)?)
            }
            "textDocument/codeAction" => {
                let uri = required_text_document_uri(&params)?;
                let range = required_range(params.pointer("/range").unwrap_or(&Value::Null))?;
                self.respond(
                    out,
                    id,
                    serde_json::to_value(self.engine.code_actions(&uri, range))?,
                )
            }
            "textDocument/signatureHelp" => {
                let (uri, position) = required_text_document_position(&params)?;
                self.respond(
                    out,
                    id,
                    serde_json::to_value(self.engine.signature_help(&uri, position))?,
                )
            }
            "textDocument/inlayHint" => {
                let uri = required_text_document_uri(&params)?;
                let range = required_range(params.get("range").unwrap_or(&Value::Null))?;
                self.respond(
                    out,
                    id,
                    serde_json::to_value(self.engine.inlay_hints(&uri, range))?,
                )
            }
            "textDocument/diagnostic" => {
                let uri = required_text_document_uri(&params)?;
                self.respond(
                    out,
                    id,
                    json!({ "kind": "full", "items": self.engine.publish_diagnostics(&uri) }),
                )
            }
            "workspace/diagnostic" => {
                let items = self
                    .engine
                    .documents()
                    .iter()
                    .map(|document| {
                        json!({
                            "uri": document.uri,
                            "kind": "full",
                            "items": self.engine.publish_diagnostics(&document.uri),
                        })
                    })
                    .collect::<Vec<_>>();
                self.respond(out, id, json!({ "items": items }))
            }
            _ if id.is_some() => self.respond_error(out, id, -32601, "method not found"),
            _ => Ok(()),
        }?;

        Ok(method != "exit" || !self.shutdown_requested)
    }

    fn publish_diagnostics(&self, out: &mut impl Write, uri: &str) -> anyhow::Result<()> {
        self.write_notification(
            out,
            "textDocument/publishDiagnostics",
            json!({ "uri": uri, "diagnostics": self.engine.publish_diagnostics(uri) }),
        )
    }

    fn respond_location(
        &self,
        out: &mut impl Write,
        id: Option<Value>,
        location: Option<Location>,
    ) -> anyhow::Result<()> {
        self.respond(out, id, serde_json::to_value(location)?)
    }

    fn respond(
        &self,
        out: &mut impl Write,
        id: Option<Value>,
        result: Value,
    ) -> anyhow::Result<()> {
        if let Some(id) = id {
            write_message(
                out,
                &json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            )?;
        }
        Ok(())
    }

    fn respond_error(
        &self,
        out: &mut impl Write,
        id: Option<Value>,
        code: i32,
        message: &str,
    ) -> anyhow::Result<()> {
        if let Some(id) = id {
            write_message(
                out,
                &json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
            )?;
        }
        Ok(())
    }

    fn write_notification(
        &self,
        out: &mut impl Write,
        method: &str,
        params: Value,
    ) -> anyhow::Result<()> {
        write_message(
            out,
            &json!({ "jsonrpc": "2.0", "method": method, "params": params }),
        )
    }
}

fn initialize_result() -> Value {
    json!({
        "capabilities": {
            "textDocumentSync": 1,
            "hoverProvider": true,
            "completionProvider": {
                "resolveProvider": false,
                "triggerCharacters": [".", "\"", "@", ":"]
            },
            "definitionProvider": true,
            "typeDefinitionProvider": true,
            "implementationProvider": true,
            "referencesProvider": true,
            "documentHighlightProvider": true,
            "documentSymbolProvider": true,
            "workspaceSymbolProvider": true,
            "foldingRangeProvider": true,
            "selectionRangeProvider": true,
            "renameProvider": { "prepareProvider": true },
            "codeActionProvider": true,
            "signatureHelpProvider": { "triggerCharacters": ["(", ","] },
            "inlayHintProvider": true,
            "diagnosticProvider": {
                "identifier": "pkl-lsp",
                "interFileDependencies": true,
                "workspaceDiagnostics": true
            },
            "semanticTokensProvider": {
                "legend": {
                    "tokenTypes": [
                        "namespace", "type", "class", "parameter", "variable", "property",
                        "function", "keyword", "comment", "string", "number", "operator", "macro"
                    ],
                    "tokenModifiers": ["declaration", "definition", "readonly", "deprecated"]
                },
                "full": true,
                "range": true
            }
        },
        "serverInfo": {
            "name": "pkl-lsp",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

fn read_message(input: &mut impl BufRead) -> anyhow::Result<Option<Value>> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            content_length = Some(value.trim().parse::<usize>()?);
        }
    }
    let Some(content_length) = content_length else {
        anyhow::bail!("missing Content-Length header");
    };
    let mut body = vec![0; content_length];
    input.read_exact(&mut body)?;
    Ok(Some(serde_json::from_slice(&body)?))
}

fn write_message(out: &mut impl Write, value: &Value) -> anyhow::Result<()> {
    let body = serde_json::to_vec(value)?;
    write!(out, "Content-Length: {}\r\n\r\n", body.len())?;
    out.write_all(&body)?;
    out.flush()?;
    Ok(())
}

fn required_text_document_uri(params: &Value) -> anyhow::Result<String> {
    params
        .pointer("/textDocument/uri")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow::anyhow!("missing textDocument.uri"))
}

fn required_text_document_position(params: &Value) -> anyhow::Result<(String, TextPosition)> {
    let uri = required_text_document_uri(params)?;
    let position = params
        .get("position")
        .or_else(|| params.pointer("/textDocumentPosition/position"))
        .and_then(parse_position)
        .ok_or_else(|| anyhow::anyhow!("missing position"))?;
    Ok((uri, position))
}

fn parse_position(value: &Value) -> Option<TextPosition> {
    Some(TextPosition {
        line: value.get("line")?.as_u64()? as u32,
        character: value.get("character")?.as_u64()? as u32,
    })
}

fn required_range(value: &Value) -> anyhow::Result<TextRange> {
    let start = value
        .get("start")
        .and_then(parse_position)
        .ok_or_else(|| anyhow::anyhow!("missing range.start"))?;
    let end = value
        .get("end")
        .and_then(parse_position)
        .ok_or_else(|| anyhow::anyhow!("missing range.end"))?;
    Ok(TextRange { start, end })
}

fn encode_semantic_tokens(tokens: Vec<pkl_lsp_core::SemanticToken>) -> Vec<u32> {
    let mut previous_line = 0;
    let mut previous_start = 0;
    tokens
        .into_iter()
        .flat_map(|token| {
            let delta_line = token.line.saturating_sub(previous_line);
            let delta_start = if delta_line == 0 {
                token.start.saturating_sub(previous_start)
            } else {
                token.start
            };
            previous_line = token.line;
            previous_start = token.start;
            [
                delta_line,
                delta_start,
                token.length,
                token.token_type,
                token.modifiers,
            ]
        })
        .collect()
}

fn workspace_edit_to_lsp(edit: pkl_lsp_core::WorkspaceEdit) -> Value {
    let mut changes = BTreeMap::new();
    for document in edit.changes {
        changes.insert(document.uri, document.edits);
    }
    json!({ "changes": changes })
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if !args.stdio {
        eprintln!("pkl-lsp currently supports only --stdio");
        std::process::exit(2);
    }

    let stdin = std::io::stdin();
    let mut input = BufReader::new(stdin.lock());
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    let mut server = Server::new();

    while let Some(message) = read_message(&mut input)? {
        if !server.handle(message, &mut output)? {
            break;
        }
    }
    Ok(())
}
