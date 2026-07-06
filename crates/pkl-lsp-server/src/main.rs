use std::str::FromStr;
use std::sync::Arc;

use clap::Parser;
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams, CompletionResponse,
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverContents, HoverParams,
    InitializeParams, InitializeResult, Location, MarkedString, OneOf, Position, Range,
    ReferenceParams, ServerCapabilities, SymbolKind, TextDocumentSyncCapability,
    TextDocumentSyncKind, Url, WorkDoneProgressOptions,
};
use pkl_lsp_core::{
    CompletionList, CompletionOptions as CoreCompletionOptions, DocumentStore, FeatureEngine,
    TextPosition, TextRange,
};
use tokio::sync::Mutex;
use tower_lsp::{Client, LanguageServer, LspService, Server, jsonrpc::Result as RpcResult};

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Args {
    /// Run as a stdio LSP server.
    #[arg(long)]
    stdio: bool,
}

#[derive(Debug)]
struct Backend {
    client: Client,
    engine: Arc<Mutex<FeatureEngine>>,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            engine: Arc::new(Mutex::new(FeatureEngine::new(DocumentStore::default()))),
        }
    }

    async fn publish(&self, uri: &Url) {
        let uri_string = uri.to_string();
        let diagnostics = self
            .engine
            .lock()
            .await
            .publish_diagnostics(&uri_string)
            .into_iter()
            .map(core_diagnostic_to_lsp)
            .collect();
        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> RpcResult<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                document_symbol_provider: Some(OneOf::Left(true)),
                hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![
                        ".".to_string(),
                        "\"".to_string(),
                        "@".to_string(),
                    ]),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                    all_commit_characters: None,
                    completion_item: None,
                }),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
            server_info: Some(lsp_types::ServerInfo {
                name: "pkl-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: lsp_types::InitializedParams) {
        self.client
            .log_message(lsp_types::MessageType::INFO, "pkl-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> RpcResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        {
            let mut engine = self.engine.lock().await;
            engine.documents_mut().open(
                uri.to_string(),
                params.text_document.text,
                params.text_document.version,
            );
            engine.rebuild();
        }
        self.publish(&uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().last() {
            {
                let mut engine = self.engine.lock().await;
                engine.documents_mut().change(
                    uri.to_string(),
                    change.text,
                    params.text_document.version,
                );
                engine.rebuild();
            }
            self.publish(&uri).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        {
            let mut engine = self.engine.lock().await;
            engine.documents_mut().close(uri.as_ref());
            engine.rebuild();
        }
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> RpcResult<Option<DocumentSymbolResponse>> {
        let symbols = self
            .engine
            .lock()
            .await
            .document_symbols(params.text_document.uri.as_ref())
            .into_iter()
            .map(core_document_symbol_to_lsp)
            .collect();
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn hover(&self, params: HoverParams) -> RpcResult<Option<Hover>> {
        Ok(self
            .engine
            .lock()
            .await
            .hover(
                params
                    .text_document_position_params
                    .text_document
                    .uri
                    .as_ref(),
                text_position_from_lsp(params.text_document_position_params.position),
            )
            .map(core_hover_to_lsp))
    }

    async fn completion(&self, params: CompletionParams) -> RpcResult<Option<CompletionResponse>> {
        let completion = self.engine.lock().await.completion(
            params.text_document_position.text_document.uri.as_ref(),
            text_position_from_lsp(params.text_document_position.position),
            CoreCompletionOptions {
                include_keywords: true,
            },
        );
        Ok(Some(core_completion_to_lsp(completion)))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> RpcResult<Option<GotoDefinitionResponse>> {
        let Some(location) = self.engine.lock().await.definition(
            params
                .text_document_position_params
                .text_document
                .uri
                .as_ref(),
            text_position_from_lsp(params.text_document_position_params.position),
        ) else {
            return Ok(None);
        };
        let Ok(uri) = Url::from_str(&location.uri) else {
            return Ok(None);
        };
        Ok(Some(GotoDefinitionResponse::Scalar(Location {
            uri,
            range: text_range_to_lsp(location.range),
        })))
    }

    async fn references(&self, params: ReferenceParams) -> RpcResult<Option<Vec<Location>>> {
        let references = self
            .engine
            .lock()
            .await
            .references(
                params.text_document_position.text_document.uri.as_ref(),
                text_position_from_lsp(params.text_document_position.position),
            )
            .into_iter()
            .filter_map(|location| {
                Url::from_str(&location.uri).ok().map(|uri| Location {
                    uri,
                    range: text_range_to_lsp(location.range),
                })
            })
            .collect();
        Ok(Some(references))
    }
}

fn text_position_from_lsp(position: Position) -> TextPosition {
    TextPosition {
        line: position.line,
        character: position.character,
    }
}

fn text_position_to_lsp(position: TextPosition) -> Position {
    Position::new(position.line, position.character)
}

fn text_range_to_lsp(range: TextRange) -> Range {
    Range::new(
        text_position_to_lsp(range.start),
        text_position_to_lsp(range.end),
    )
}

fn core_diagnostic_to_lsp(diagnostic: pkl_lsp_core::Diagnostic) -> Diagnostic {
    Diagnostic {
        range: text_range_to_lsp(diagnostic.range),
        severity: diagnostic.severity.and_then(|severity| match severity {
            1 => Some(DiagnosticSeverity::ERROR),
            _ => None,
        }),
        source: diagnostic.source,
        message: diagnostic.message,
        ..Diagnostic::default()
    }
}

#[allow(deprecated)]
fn core_document_symbol_to_lsp(symbol: pkl_lsp_core::DocumentSymbol) -> DocumentSymbol {
    DocumentSymbol {
        name: symbol.name,
        detail: symbol.detail,
        kind: match symbol.kind {
            2 => SymbolKind::MODULE,
            3 => SymbolKind::NAMESPACE,
            5 => SymbolKind::CLASS,
            7 => SymbolKind::PROPERTY,
            24 => SymbolKind::EVENT,
            26 => SymbolKind::TYPE_PARAMETER,
            _ => SymbolKind::VARIABLE,
        },
        tags: None,
        deprecated: None,
        range: text_range_to_lsp(symbol.range),
        selection_range: text_range_to_lsp(symbol.selection_range),
        children: symbol.children.map(|children| {
            children
                .into_iter()
                .map(core_document_symbol_to_lsp)
                .collect()
        }),
    }
}

fn core_hover_to_lsp(hover: pkl_lsp_core::Hover) -> Hover {
    Hover {
        contents: HoverContents::Scalar(MarkedString::String(hover.contents)),
        range: hover.range.map(text_range_to_lsp),
    }
}

fn core_completion_to_lsp(completion: CompletionList) -> CompletionResponse {
    CompletionResponse::Array(
        completion
            .items
            .into_iter()
            .map(|item| CompletionItem {
                label: item.label,
                kind: item.kind.and_then(|kind| match kind {
                    7 => Some(CompletionItemKind::CLASS),
                    9 => Some(CompletionItemKind::MODULE),
                    10 => Some(CompletionItemKind::PROPERTY),
                    14 => Some(CompletionItemKind::KEYWORD),
                    18 => Some(CompletionItemKind::REFERENCE),
                    25 => Some(CompletionItemKind::TYPE_PARAMETER),
                    _ => None,
                }),
                detail: item.detail,
                documentation: item.documentation.map(lsp_types::Documentation::String),
                ..CompletionItem::default()
            })
            .collect(),
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if !args.stdio {
        eprintln!("pkl-lsp currently supports only --stdio");
        std::process::exit(2);
    }

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
    Ok(())
}
