use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionResponse, Documentation, Hover, HoverContents,
    MarkedString, Position, Range,
};

use crate::document::DocumentStore;
use crate::index::{PklSymbol, WorkspaceIndex, offset_at_position, word_at_position};

const KEYWORDS: &[&str] = &[
    "amends",
    "as",
    "class",
    "const",
    "else",
    "extends",
    "external",
    "false",
    "fixed",
    "for",
    "function",
    "hidden",
    "if",
    "import",
    "import*",
    "in",
    "is",
    "let",
    "local",
    "module",
    "new",
    "null",
    "open",
    "out",
    "read",
    "read?",
    "super",
    "this",
    "throw",
    "trace",
    "true",
    "typealias",
    "when",
];

#[derive(Debug, Clone, Copy, Default)]
pub struct CompletionOptions {
    pub include_keywords: bool,
}

#[derive(Debug, Clone)]
pub struct FeatureEngine {
    documents: DocumentStore,
    index: WorkspaceIndex,
}

impl FeatureEngine {
    pub fn new(documents: DocumentStore) -> Self {
        let index = WorkspaceIndex::build(documents.iter());
        Self { documents, index }
    }

    pub fn rebuild(&mut self) {
        self.index = WorkspaceIndex::build(self.documents.iter());
    }

    pub fn documents(&self) -> &DocumentStore {
        &self.documents
    }

    pub fn documents_mut(&mut self) -> &mut DocumentStore {
        &mut self.documents
    }

    pub fn index(&self) -> &WorkspaceIndex {
        &self.index
    }

    pub fn publish_diagnostics(&self, uri: &str) -> Vec<lsp_types::Diagnostic> {
        self.index.diagnostics_for_lsp(uri)
    }

    pub fn document_symbols(&self, uri: &str) -> Vec<lsp_types::DocumentSymbol> {
        self.index.document_symbols(uri)
    }

    pub fn hover(&self, uri: &str, position: Position) -> Option<Hover> {
        let symbol = self.symbol_at(uri, position)?;
        let mut value = format!("**{}**", symbol.name);
        if let Some(detail) = &symbol.detail {
            value.push_str(&format!("\n\n`{detail}`"));
        }
        if let Some(container) = &symbol.container_name {
            value.push_str(&format!("\n\nContainer: `{container}`"));
        }
        Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(value)),
            range: Some(symbol.selection_range.into()),
        })
    }

    pub fn completion(
        &self,
        uri: &str,
        _position: Position,
        options: CompletionOptions,
    ) -> CompletionResponse {
        let mut items = Vec::new();
        if options.include_keywords {
            items.extend(KEYWORDS.iter().map(|keyword| CompletionItem {
                label: (*keyword).to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                ..CompletionItem::default()
            }));
        }
        items.extend(self.index.symbols_in_uri(uri).map(|symbol| {
            CompletionItem {
                label: symbol.name.clone(),
                kind: Some(match symbol.kind {
                    crate::SymbolKind::Module => CompletionItemKind::MODULE,
                    crate::SymbolKind::Import => CompletionItemKind::MODULE,
                    crate::SymbolKind::Property => CompletionItemKind::PROPERTY,
                    crate::SymbolKind::Class => CompletionItemKind::CLASS,
                    crate::SymbolKind::TypeAlias => CompletionItemKind::TYPE_PARAMETER,
                    crate::SymbolKind::Annotation => CompletionItemKind::REFERENCE,
                }),
                detail: symbol.detail.clone(),
                documentation: symbol
                    .container_name
                    .as_ref()
                    .map(|container| Documentation::String(format!("Container: {container}"))),
                ..CompletionItem::default()
            }
        }));
        CompletionResponse::Array(items)
    }

    pub fn definition(&self, uri: &str, position: Position) -> Option<(String, Range)> {
        let document = self.documents.get(uri)?;
        let word = word_at_position(&document.text, position)?;
        self.index
            .symbols
            .iter()
            .find(|symbol| symbol.name.trim_start_matches('@') == word)
            .map(|symbol| (symbol.uri.clone(), symbol.selection_range.into()))
    }

    pub fn references(&self, uri: &str, position: Position) -> Vec<(String, Range)> {
        let Some(document) = self.documents.get(uri) else {
            return Vec::new();
        };
        let Some(word) = word_at_position(&document.text, position) else {
            return Vec::new();
        };
        self.documents
            .iter()
            .flat_map(|document| find_word_ranges(&document.uri, &document.text, &word))
            .collect()
    }

    fn symbol_at(&self, uri: &str, position: Position) -> Option<&PklSymbol> {
        let offset = self
            .documents
            .get(uri)
            .map(|document| offset_at_position(&document.text, position))?;
        self.index
            .symbols_in_uri(uri)
            .filter(|symbol| {
                range_contains_offset(
                    self.documents.get(uri).unwrap().text.as_str(),
                    symbol.selection_range.into(),
                    offset,
                )
            })
            .min_by_key(|symbol| {
                let range: Range = symbol.selection_range.into();
                (
                    range.end.line - range.start.line,
                    range.end.character - range.start.character,
                )
            })
    }
}

fn range_contains_offset(text: &str, range: Range, offset: usize) -> bool {
    let start = offset_at_position(text, range.start);
    let end = offset_at_position(text, range.end);
    start <= offset && offset <= end
}

fn find_word_ranges(uri: &str, text: &str, word: &str) -> Vec<(String, Range)> {
    let mut result = Vec::new();
    let mut start = 0;
    while let Some(idx) = text[start..].find(word) {
        let absolute = start + idx;
        let before = absolute
            .checked_sub(1)
            .and_then(|pos| text.as_bytes().get(pos))
            .copied();
        let after = text.as_bytes().get(absolute + word.len()).copied();
        if !before.is_some_and(is_ident_byte) && !after.is_some_and(is_ident_byte) {
            let start_pos = crate::index::position_at_offset(text, absolute);
            let end_pos = crate::index::position_at_offset(text, absolute + word.len());
            result.push((
                uri.to_string(),
                Range::new(start_pos.into(), end_pos.into()),
            ));
        }
        start = absolute + word.len();
    }
    result
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

#[cfg(test)]
mod tests {
    use lsp_types::Position;

    use crate::{CompletionOptions, DocumentStore, FeatureEngine};

    #[test]
    fn hover_completion_definition_and_references() {
        let mut docs = DocumentStore::default();
        docs.open(
            "file:///demo.pkl",
            "name: String = \"demo\"\nother = name\n",
            1,
        );
        let engine = FeatureEngine::new(docs);

        assert!(
            engine
                .hover("file:///demo.pkl", Position::new(0, 1))
                .is_some()
        );
        let completion = engine.completion(
            "file:///demo.pkl",
            Position::new(0, 0),
            CompletionOptions {
                include_keywords: true,
            },
        );
        let lsp_types::CompletionResponse::Array(items) = completion else {
            panic!("expected completion array");
        };
        assert!(!items.is_empty());
        assert!(
            engine
                .definition("file:///demo.pkl", Position::new(1, 9))
                .is_some()
        );
        assert_eq!(
            engine
                .references("file:///demo.pkl", Position::new(1, 9))
                .len(),
            2
        );
    }
}
