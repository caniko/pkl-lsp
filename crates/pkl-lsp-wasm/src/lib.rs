use std::cell::RefCell;

use pkl_lsp_core::{CompletionOptions, DocumentStore, FeatureEngine, TextPosition};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct ServerHandle {
    engine: RefCell<FeatureEngine>,
}

#[wasm_bindgen]
pub fn create_server(_config: JsValue) -> ServerHandle {
    console_error_panic_hook::set_once();
    ServerHandle {
        engine: RefCell::new(FeatureEngine::new(DocumentStore::default())),
    }
}

#[wasm_bindgen]
impl ServerHandle {
    pub fn open_document(&self, uri: String, text: String, version: i32) {
        let mut engine = self.engine.borrow_mut();
        engine.documents_mut().open(uri, text, version);
        engine.rebuild();
    }

    pub fn change_document(&self, uri: String, text: String, version: i32) {
        let mut engine = self.engine.borrow_mut();
        engine.documents_mut().change(uri, text, version);
        engine.rebuild();
    }

    pub fn close_document(&self, uri: String) {
        let mut engine = self.engine.borrow_mut();
        engine.documents_mut().close(&uri);
        engine.rebuild();
    }

    pub fn request(&self, method: String, params_json: String) -> Result<String, JsValue> {
        let engine = self.engine.borrow();
        let params: serde_json::Value = serde_json::from_str(&params_json)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let response = match method.as_str() {
            "textDocument/diagnostic" | "pkl/diagnostics" => {
                let uri = required_string(&params, "uri")?;
                serde_json::to_value(engine.publish_diagnostics(uri)).unwrap()
            }
            "textDocument/documentSymbol" => {
                let uri = required_string(&params, "uri")?;
                serde_json::to_value(engine.document_symbols(uri)).unwrap()
            }
            "workspace/symbol" => {
                let query = params
                    .get("query")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                serde_json::to_value(engine.workspace_symbols(query)).unwrap()
            }
            "textDocument/hover" => {
                let uri = required_string(&params, "uri")?;
                let position = required_position(&params)?;
                serde_json::to_value(engine.hover(uri, position)).unwrap()
            }
            "textDocument/completion" => {
                let uri = required_string(&params, "uri")?;
                let position = required_position(&params)?;
                serde_json::to_value(engine.completion(
                    uri,
                    position,
                    CompletionOptions {
                        include_keywords: true,
                    },
                ))
                .unwrap()
            }
            "textDocument/definition" => {
                let uri = required_string(&params, "uri")?;
                let position = required_position(&params)?;
                serde_json::to_value(engine.definition(uri, position)).unwrap()
            }
            "textDocument/references" => {
                let uri = required_string(&params, "uri")?;
                let position = required_position(&params)?;
                serde_json::to_value(engine.references(uri, position)).unwrap()
            }
            "textDocument/typeDefinition" => {
                let uri = required_string(&params, "uri")?;
                let position = required_position(&params)?;
                serde_json::to_value(engine.type_definition(uri, position)).unwrap()
            }
            "textDocument/implementation" => {
                let uri = required_string(&params, "uri")?;
                let position = required_position(&params)?;
                serde_json::to_value(engine.implementation(uri, position)).unwrap()
            }
            "textDocument/documentHighlight" => {
                let uri = required_string(&params, "uri")?;
                let position = required_position(&params)?;
                serde_json::to_value(engine.document_highlights(uri, position)).unwrap()
            }
            "textDocument/semanticTokens/full" => {
                let uri = required_string(&params, "uri")?;
                serde_json::to_value(engine.semantic_tokens(uri, None)).unwrap()
            }
            "textDocument/semanticTokens/range" => {
                let uri = required_string(&params, "uri")?;
                let range = required_range(&params)?;
                serde_json::to_value(engine.semantic_tokens(uri, Some(range))).unwrap()
            }
            "textDocument/foldingRange" => {
                let uri = required_string(&params, "uri")?;
                serde_json::to_value(engine.folding_ranges(uri)).unwrap()
            }
            "textDocument/selectionRange" => {
                let uri = required_string(&params, "uri")?;
                let positions = params
                    .get("positions")
                    .and_then(serde_json::Value::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(|value| required_position_value(value).ok())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                serde_json::to_value(engine.selection_ranges(uri, &positions)).unwrap()
            }
            "textDocument/prepareRename" => {
                let uri = required_string(&params, "uri")?;
                let position = required_position(&params)?;
                serde_json::to_value(engine.prepare_rename(uri, position)).unwrap()
            }
            "textDocument/rename" => {
                let uri = required_string(&params, "uri")?;
                let position = required_position(&params)?;
                let new_name = required_string(&params, "newName")?;
                serde_json::to_value(engine.rename(uri, position, new_name)).unwrap()
            }
            "textDocument/codeAction" => {
                let uri = required_string(&params, "uri")?;
                let range = required_range(&params)?;
                serde_json::to_value(engine.code_actions(uri, range)).unwrap()
            }
            "textDocument/signatureHelp" => {
                let uri = required_string(&params, "uri")?;
                let position = required_position(&params)?;
                serde_json::to_value(engine.signature_help(uri, position)).unwrap()
            }
            "textDocument/inlayHint" => {
                let uri = required_string(&params, "uri")?;
                let range = required_range(&params)?;
                serde_json::to_value(engine.inlay_hints(uri, range)).unwrap()
            }
            _ => {
                return Err(JsValue::from_str(&format!(
                    "unsupported request method: {method}"
                )));
            }
        };
        serde_json::to_string(&response).map_err(|error| JsValue::from_str(&error.to_string()))
    }
}

fn required_string<'a>(params: &'a serde_json::Value, field: &str) -> Result<&'a str, JsValue> {
    params
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| JsValue::from_str(&format!("missing string field '{field}'")))
}

fn required_position(params: &serde_json::Value) -> Result<TextPosition, JsValue> {
    let position = params
        .get("position")
        .ok_or_else(|| JsValue::from_str("missing position"))?;
    required_position_value(position)
}

fn required_position_value(position: &serde_json::Value) -> Result<TextPosition, JsValue> {
    let line = position
        .get("line")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| JsValue::from_str("missing position.line"))?;
    let character = position
        .get("character")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| JsValue::from_str("missing position.character"))?;
    Ok(TextPosition {
        line: line as u32,
        character: character as u32,
    })
}

fn required_range(params: &serde_json::Value) -> Result<pkl_lsp_core::TextRange, JsValue> {
    let range = params
        .get("range")
        .ok_or_else(|| JsValue::from_str("missing range"))?;
    let start = range
        .get("start")
        .ok_or_else(|| JsValue::from_str("missing range.start"))
        .and_then(required_position_value)?;
    let end = range
        .get("end")
        .ok_or_else(|| JsValue::from_str("missing range.end"))
        .and_then(required_position_value)?;
    Ok(pkl_lsp_core::TextRange { start, end })
}
