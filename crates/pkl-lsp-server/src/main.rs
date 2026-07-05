use std::str::FromStr;
use std::sync::Arc;

use clap::Parser;
use lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DocumentSymbolParams,
    DocumentSymbolResponse, GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams,
    InitializeParams, InitializeResult, Location, OneOf, ReferenceParams, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind, Url, WorkDoneProgressOptions,
};
use pkl_lsp_core::{CompletionOptions as CoreCompletionOptions, DocumentStore, FeatureEngine};
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
        let diagnostics = self.engine.lock().await.publish_diagnostics(&uri_string);
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
            .document_symbols(params.text_document.uri.as_ref());
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn hover(&self, params: HoverParams) -> RpcResult<Option<Hover>> {
        Ok(self.engine.lock().await.hover(
            params
                .text_document_position_params
                .text_document
                .uri
                .as_ref(),
            params.text_document_position_params.position,
        ))
    }

    async fn completion(&self, params: CompletionParams) -> RpcResult<Option<CompletionResponse>> {
        Ok(Some(self.engine.lock().await.completion(
            params.text_document_position.text_document.uri.as_ref(),
            params.text_document_position.position,
            CoreCompletionOptions {
                include_keywords: true,
            },
        )))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> RpcResult<Option<GotoDefinitionResponse>> {
        let Some((uri, range)) = self.engine.lock().await.definition(
            params
                .text_document_position_params
                .text_document
                .uri
                .as_ref(),
            params.text_document_position_params.position,
        ) else {
            return Ok(None);
        };
        let Ok(uri) = Url::from_str(&uri) else {
            return Ok(None);
        };
        Ok(Some(GotoDefinitionResponse::Scalar(Location {
            uri,
            range,
        })))
    }

    async fn references(&self, params: ReferenceParams) -> RpcResult<Option<Vec<Location>>> {
        let references = self
            .engine
            .lock()
            .await
            .references(
                params.text_document_position.text_document.uri.as_ref(),
                params.text_document_position.position,
            )
            .into_iter()
            .filter_map(|(uri, range)| Url::from_str(&uri).ok().map(|uri| Location { uri, range }))
            .collect();
        Ok(Some(references))
    }
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
