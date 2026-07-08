use serde::{Deserialize, Serialize};

use crate::index::TextRange;

#[allow(dead_code)]
pub mod completion_kind {
    pub const CLASS: u32 = 7;
    pub const FUNCTION: u32 = 3;
    pub const FIELD: u32 = 5;
    pub const VARIABLE: u32 = 6;
    pub const MODULE: u32 = 9;
    pub const PROPERTY: u32 = 10;
    pub const VALUE: u32 = 12;
    pub const KEYWORD: u32 = 14;
    pub const SNIPPET: u32 = 15;
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
    pub sort_text: Option<String>,
    pub filter_text: Option<String>,
    pub insert_text: Option<String>,
    pub insert_text_format: Option<u32>,
    pub text_edit: Option<TextEdit>,
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

#[allow(dead_code)]
pub mod symbol_kind {
    pub const MODULE: u32 = 2;
    pub const NAMESPACE: u32 = 3;
    pub const CLASS: u32 = 5;
    pub const PROPERTY: u32 = 7;
    pub const EVENT: u32 = 24;
    pub const TYPE_PARAMETER: u32 = 26;
    pub const FUNCTION: u32 = 12;
    pub const VARIABLE: u32 = 13;
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEdit {
    pub range: TextRange,
    pub new_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceEdit {
    pub changes: Vec<DocumentEdit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentEdit {
    pub uri: String,
    pub edits: Vec<TextEdit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSymbol {
    pub name: String,
    pub kind: u32,
    pub location: Location,
    pub container_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentHighlight {
    pub range: TextRange,
    pub kind: Option<u32>,
}

#[allow(dead_code)]
pub mod document_highlight_kind {
    pub const TEXT: u32 = 1;
    pub const READ: u32 = 2;
    pub const WRITE: u32 = 3;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FoldingRange {
    pub start_line: u32,
    pub start_character: Option<u32>,
    pub end_line: u32,
    pub end_character: Option<u32>,
    pub kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionRange {
    pub range: TextRange,
    pub parent: Option<Box<SelectionRange>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticToken {
    pub line: u32,
    pub start: u32,
    pub length: u32,
    pub token_type: u32,
    pub modifiers: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticTokens {
    pub data: Vec<SemanticToken>,
}

#[allow(dead_code)]
pub mod semantic_token_type {
    pub const NAMESPACE: u32 = 0;
    pub const TYPE: u32 = 1;
    pub const CLASS: u32 = 2;
    pub const PARAMETER: u32 = 3;
    pub const VARIABLE: u32 = 4;
    pub const PROPERTY: u32 = 5;
    pub const FUNCTION: u32 = 6;
    pub const KEYWORD: u32 = 7;
    pub const COMMENT: u32 = 8;
    pub const STRING: u32 = 9;
    pub const NUMBER: u32 = 10;
    pub const OPERATOR: u32 = 11;
    pub const MACRO: u32 = 12;
}

#[allow(dead_code)]
pub mod semantic_token_modifier {
    pub const DECLARATION: u32 = 1;
    pub const DEFINITION: u32 = 1 << 1;
    pub const READONLY: u32 = 1 << 2;
    pub const DEPRECATED: u32 = 1 << 3;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareRename {
    pub range: TextRange,
    pub placeholder: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeAction {
    pub title: String,
    pub kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureHelp {
    pub signatures: Vec<SignatureInformation>,
    pub active_signature: Option<u32>,
    pub active_parameter: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureInformation {
    pub label: String,
    pub documentation: Option<String>,
    pub parameters: Vec<ParameterInformation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterInformation {
    pub label: String,
    pub documentation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InlayHint {
    pub position: crate::index::TextPosition,
    pub label: String,
    pub kind: Option<u32>,
}
