use serde::{Deserialize, Serialize};

use crate::index::TextRange;

pub mod completion_kind {
    pub const CLASS: u32 = 7;
    pub const MODULE: u32 = 9;
    pub const PROPERTY: u32 = 10;
    pub const KEYWORD: u32 = 14;
    pub const REFERENCE: u32 = 18;
    pub const TYPE_PARAMETER: u32 = 25;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItem {
    pub label: String,
    pub kind: Option<u32>,
    pub detail: Option<String>,
    pub documentation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionList {
    pub items: Vec<CompletionItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hover {
    pub contents: String,
    pub range: Option<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSymbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: u32,
    pub range: TextRange,
    pub selection_range: TextRange,
    pub children: Option<Vec<DocumentSymbol>>,
}

pub mod symbol_kind {
    pub const MODULE: u32 = 2;
    pub const NAMESPACE: u32 = 3;
    pub const CLASS: u32 = 5;
    pub const PROPERTY: u32 = 7;
    pub const EVENT: u32 = 24;
    pub const TYPE_PARAMETER: u32 = 26;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub range: TextRange,
    pub severity: Option<u32>,
    pub source: Option<String>,
    pub message: String,
}

pub mod diagnostic_severity {
    pub const ERROR: u32 = 1;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    pub uri: String,
    pub range: TextRange,
}
