use crate::document::DocumentStore;
use crate::index::{
    PklSymbol, TextPosition, TextRange, WorkspaceIndex, offset_at_position, word_at_position,
};
use crate::protocol::{self, CompletionItem, CompletionList, Hover, Location};

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

    pub fn publish_diagnostics(&self, uri: &str) -> Vec<protocol::Diagnostic> {
        self.index.diagnostics_for_protocol(uri)
    }

    pub fn document_symbols(&self, uri: &str) -> Vec<protocol::DocumentSymbol> {
        self.index.document_symbols(uri)
    }

    pub fn hover(&self, uri: &str, position: TextPosition) -> Option<Hover> {
        let symbol = self.symbol_at(uri, position)?;
        let mut value = format!("**{}**", symbol.name);
        if let Some(detail) = &symbol.detail {
            value.push_str(&format!("\n\n`{detail}`"));
        }
        if let Some(container) = &symbol.container_name {
            value.push_str(&format!("\n\nContainer: `{container}`"));
        }
        Some(Hover {
            contents: value,
            range: Some(symbol.selection_range),
        })
    }

    pub fn completion(
        &self,
        uri: &str,
        _position: TextPosition,
        options: CompletionOptions,
    ) -> CompletionList {
        let mut items = Vec::new();
        if options.include_keywords {
            items.extend(KEYWORDS.iter().map(|keyword| CompletionItem {
                label: (*keyword).to_string(),
                kind: Some(protocol::completion_kind::KEYWORD),
                detail: None,
                documentation: None,
            }));
        }
        items.extend(self.index.symbols_in_uri(uri).map(|symbol| {
            CompletionItem {
                label: symbol.name.clone(),
                kind: Some(match symbol.kind {
                    crate::SymbolKind::Module => protocol::completion_kind::MODULE,
                    crate::SymbolKind::Import => protocol::completion_kind::MODULE,
                    crate::SymbolKind::Property => protocol::completion_kind::PROPERTY,
                    crate::SymbolKind::Class => protocol::completion_kind::CLASS,
                    crate::SymbolKind::TypeAlias => protocol::completion_kind::TYPE_PARAMETER,
                    crate::SymbolKind::Annotation => protocol::completion_kind::REFERENCE,
                }),
                detail: symbol.detail.clone(),
                documentation: symbol
                    .container_name
                    .as_ref()
                    .map(|container| format!("Container: {container}")),
            }
        }));
        CompletionList { items }
    }

    pub fn definition(&self, uri: &str, position: TextPosition) -> Option<Location> {
        let document = self.documents.get(uri)?;
        let word = word_at_position(&document.text, position)?;
        self.index
            .symbols
            .iter()
            .find(|symbol| symbol.name.trim_start_matches('@') == word)
            .map(|symbol| Location {
                uri: symbol.uri.clone(),
                range: symbol.selection_range,
            })
    }

    pub fn references(&self, uri: &str, position: TextPosition) -> Vec<Location> {
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

    fn symbol_at(&self, uri: &str, position: TextPosition) -> Option<&PklSymbol> {
        let offset = self
            .documents
            .get(uri)
            .map(|document| offset_at_position(&document.text, position))?;
        self.index
            .symbols_in_uri(uri)
            .filter(|symbol| {
                range_contains_offset(
                    self.documents.get(uri).unwrap().text.as_str(),
                    symbol.selection_range,
                    offset,
                )
            })
            .min_by_key(|symbol| {
                let range = symbol.selection_range;
                (
                    range.end.line - range.start.line,
                    range.end.character - range.start.character,
                )
            })
    }
}

fn range_contains_offset(text: &str, range: TextRange, offset: usize) -> bool {
    let start = offset_at_position(text, range.start);
    let end = offset_at_position(text, range.end);
    start <= offset && offset <= end
}

fn find_word_ranges(uri: &str, text: &str, word: &str) -> Vec<Location> {
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
            result.push(Location {
                uri: uri.to_string(),
                range: TextRange {
                    start: start_pos,
                    end: end_pos,
                },
            });
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
    use crate::{CompletionOptions, DocumentStore, FeatureEngine, TextPosition};

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
                .hover(
                    "file:///demo.pkl",
                    TextPosition {
                        line: 0,
                        character: 1,
                    },
                )
                .is_some()
        );
        let completion = engine.completion(
            "file:///demo.pkl",
            TextPosition {
                line: 0,
                character: 0,
            },
            CompletionOptions {
                include_keywords: true,
            },
        );
        assert!(!completion.items.is_empty());
        assert!(
            engine
                .definition(
                    "file:///demo.pkl",
                    TextPosition {
                        line: 1,
                        character: 9,
                    },
                )
                .is_some()
        );
        assert_eq!(
            engine
                .references(
                    "file:///demo.pkl",
                    TextPosition {
                        line: 1,
                        character: 9,
                    },
                )
                .len(),
            2
        );
    }
}
