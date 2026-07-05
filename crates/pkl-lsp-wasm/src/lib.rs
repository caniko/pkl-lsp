use std::cell::RefCell;

use lsp_types::Position;
use pkl_lsp_core::{CompletionOptions, DocumentStore, FeatureEngine};
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

fn required_position(params: &serde_json::Value) -> Result<Position, JsValue> {
    let position = params
        .get("position")
        .ok_or_else(|| JsValue::from_str("missing position"))?;
    let line = position
        .get("line")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| JsValue::from_str("missing position.line"))?;
    let character = position
        .get("character")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| JsValue::from_str("missing position.character"))?;
    Ok(Position::new(line as u32, character as u32))
}
