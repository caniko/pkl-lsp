use crate::document::DocumentStore;
use crate::index::{
    PklSymbol, SymbolKind, TextPosition, TextRange, WorkspaceIndex, offset_at_position,
    position_at_offset, word_at_position,
};
use crate::protocol::{
    self, CodeAction, CompletionItem, CompletionList, DocumentHighlight, FoldingRange, Hover,
    InlayHint, Location, PrepareRename, SelectionRange, SemanticToken, SemanticTokens,
    SignatureHelp, SignatureInformation, TextEdit, WorkspaceEdit, WorkspaceSymbol,
};
use pklr::lexer::{self, Token, TokenKind};

const KEYWORDS: &[&str] = &[
    "abstract",
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

const BUILTIN_TYPES: &[&str] = &[
    "Any",
    "Null",
    "Boolean",
    "String",
    "Number",
    "Int",
    "Float",
    "Duration",
    "DataSize",
    "Dynamic",
    "Typed",
    "Listing",
    "Mapping",
    "Collection",
    "List",
    "Set",
    "Map",
    "Pair",
    "Regex",
    "Bytes",
    "IntSeq",
    "Class",
    "Function0",
    "Function1",
    "Function2",
    "Function3",
    "Function4",
    "Function5",
    "unknown",
    "nothing",
];

const BUILTIN_CONSTANTS: &[&str] = &["NaN", "Infinity", "true", "false", "null"];

const BUILTIN_FUNCTIONS: &[(&str, &str)] = &[
    ("read", "read(uri: String): Any"),
    ("read?", "read?(uri: String): Any?"),
    ("trace", "trace(value: Any): Any"),
    ("throw", "throw(message: String): nothing"),
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

    pub fn workspace_symbols(&self, query: &str) -> Vec<WorkspaceSymbol> {
        let query = query.trim().to_ascii_lowercase();
        self.index
            .symbols
            .iter()
            .filter(|symbol| {
                query.is_empty()
                    || symbol.name.to_ascii_lowercase().contains(&query)
                    || symbol
                        .container_name
                        .as_ref()
                        .is_some_and(|container| container.to_ascii_lowercase().contains(&query))
            })
            .map(|symbol| WorkspaceSymbol {
                name: symbol.name.clone(),
                kind: symbol.kind.as_protocol(),
                location: Location {
                    uri: symbol.uri.clone(),
                    range: symbol.selection_range,
                },
                container_name: symbol.container_name.clone(),
            })
            .collect()
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
        position: TextPosition,
        options: CompletionOptions,
    ) -> CompletionList {
        let mut items = Vec::new();
        let context = self.completion_context(uri, position);
        if options.include_keywords {
            items.extend(KEYWORDS.iter().map(|keyword| {
                completion_item(
                    keyword,
                    protocol::completion_kind::KEYWORD,
                    Some("Pkl keyword"),
                    None,
                )
            }));
        }
        items.extend(BUILTIN_TYPES.iter().map(|name| {
            completion_item(
                name,
                protocol::completion_kind::CLASS,
                Some("Pkl standard library type"),
                Some("Built-in type from the Pkl standard library"),
            )
        }));
        items.extend(BUILTIN_CONSTANTS.iter().map(|name| {
            completion_item(
                name,
                protocol::completion_kind::VALUE,
                Some("Pkl constant"),
                None,
            )
        }));
        items.extend(BUILTIN_FUNCTIONS.iter().map(|(name, detail)| {
            completion_item(
                name,
                protocol::completion_kind::FUNCTION,
                Some(detail),
                Some("Built-in Pkl expression"),
            )
        }));

        match context {
            CompletionContext::Member { receiver } => {
                items.extend(self.member_completion_items(uri, receiver.as_deref()));
            }
            CompletionContext::Annotation => {
                items.extend(
                    self.index
                        .symbols
                        .iter()
                        .filter(|symbol| symbol.kind == SymbolKind::Annotation)
                        .map(symbol_completion_item),
                );
            }
            CompletionContext::ImportString => {
                items.extend(self.index.imports.iter().map(|edge| {
                    completion_item(
                        &edge.target,
                        protocol::completion_kind::MODULE,
                        Some("Imported module"),
                        None,
                    )
                }));
            }
            CompletionContext::Default => {
                items.extend(self.index.symbols.iter().map(symbol_completion_item));
            }
        }

        dedupe_completion_items(&mut items);
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

    pub fn document_highlights(&self, uri: &str, position: TextPosition) -> Vec<DocumentHighlight> {
        self.references(uri, position)
            .into_iter()
            .filter(|location| location.uri == uri)
            .map(|location| DocumentHighlight {
                range: location.range,
                kind: Some(protocol::document_highlight_kind::TEXT),
            })
            .collect()
    }

    pub fn semantic_tokens(&self, uri: &str, range: Option<TextRange>) -> SemanticTokens {
        let Some(document) = self.documents.get(uri) else {
            return SemanticTokens { data: Vec::new() };
        };
        let Ok(tokens) = lexer::lex_named(&document.text, uri) else {
            return SemanticTokens { data: Vec::new() };
        };
        let mut data = Vec::new();
        for token in tokens {
            if matches!(token.kind, TokenKind::Eof) {
                continue;
            }
            let Some((token_type, modifiers)) = semantic_class_for_token(&token, &self.index)
            else {
                continue;
            };
            let token_range = token_text_range(&document.text, &token);
            if range.is_some_and(|range| !ranges_intersect(range, token_range)) {
                continue;
            }
            data.push(SemanticToken {
                line: token_range.start.line,
                start: token_range.start.character,
                length: token_range
                    .end
                    .character
                    .saturating_sub(token_range.start.character)
                    .max(1),
                token_type,
                modifiers,
            });
        }
        SemanticTokens { data }
    }

    pub fn folding_ranges(&self, uri: &str) -> Vec<FoldingRange> {
        let Some(document) = self.documents.get(uri) else {
            return Vec::new();
        };
        let mut stack: Vec<(char, TextPosition)> = Vec::new();
        let mut ranges = Vec::new();
        for (offset, ch) in document.text.char_indices() {
            match ch {
                '{' | '[' | '(' => stack.push((ch, position_at_offset(&document.text, offset))),
                '}' | ']' | ')' => {
                    let close = position_at_offset(&document.text, offset);
                    if let Some((open, start)) = stack.pop()
                        && matching_delimiter(open, ch)
                        && close.line > start.line
                    {
                        ranges.push(FoldingRange {
                            start_line: start.line,
                            start_character: Some(start.character),
                            end_line: close.line,
                            end_character: Some(close.character),
                            kind: Some("region".to_string()),
                        });
                    }
                }
                _ => {}
            }
        }
        ranges
    }

    pub fn selection_ranges(&self, uri: &str, positions: &[TextPosition]) -> Vec<SelectionRange> {
        let Some(document) = self.documents.get(uri) else {
            return Vec::new();
        };
        positions
            .iter()
            .map(|position| {
                let word = word_range_at_position(&document.text, *position).unwrap_or(TextRange {
                    start: *position,
                    end: *position,
                });
                SelectionRange {
                    range: word,
                    parent: Some(Box::new(SelectionRange {
                        range: entire_document_range_public(&document.text),
                        parent: None,
                    })),
                }
            })
            .collect()
    }

    pub fn prepare_rename(&self, uri: &str, position: TextPosition) -> Option<PrepareRename> {
        let document = self.documents.get(uri)?;
        let range = word_range_at_position(&document.text, position)?;
        let placeholder = word_at_position(&document.text, position)?;
        Some(PrepareRename { range, placeholder })
    }

    pub fn rename(
        &self,
        uri: &str,
        position: TextPosition,
        new_name: &str,
    ) -> Option<WorkspaceEdit> {
        if !is_valid_identifier(new_name) {
            return None;
        }
        let document = self.documents.get(uri)?;
        let word = word_at_position(&document.text, position)?;
        let mut changes = Vec::new();
        for document in self.documents.iter() {
            let edits = find_word_ranges(&document.uri, &document.text, &word)
                .into_iter()
                .map(|location| TextEdit {
                    range: location.range,
                    new_text: new_name.to_string(),
                })
                .collect::<Vec<_>>();
            if !edits.is_empty() {
                changes.push(protocol::DocumentEdit {
                    uri: document.uri.clone(),
                    edits,
                });
            }
        }
        Some(WorkspaceEdit { changes })
    }

    pub fn code_actions(&self, _uri: &str, _range: TextRange) -> Vec<CodeAction> {
        Vec::new()
    }

    pub fn signature_help(&self, uri: &str, position: TextPosition) -> Option<SignatureHelp> {
        let document = self.documents.get(uri)?;
        let offset = offset_at_position(&document.text, position);
        let prefix = &document.text[..offset.min(document.text.len())];
        let function = BUILTIN_FUNCTIONS
            .iter()
            .find(|(name, _)| prefix.ends_with(&format!("{name}(")))?;
        Some(SignatureHelp {
            signatures: vec![SignatureInformation {
                label: function.1.to_string(),
                documentation: Some("Pkl built-in function".to_string()),
                parameters: Vec::new(),
            }],
            active_signature: Some(0),
            active_parameter: Some(0),
        })
    }

    pub fn inlay_hints(&self, _uri: &str, _range: TextRange) -> Vec<InlayHint> {
        Vec::new()
    }

    pub fn type_definition(&self, uri: &str, position: TextPosition) -> Option<Location> {
        let document = self.documents.get(uri)?;
        let word = word_at_position(&document.text, position)?;
        self.index
            .symbols
            .iter()
            .find(|symbol| {
                symbol.name == word
                    && matches!(symbol.kind, SymbolKind::Class | SymbolKind::TypeAlias)
            })
            .map(|symbol| Location {
                uri: symbol.uri.clone(),
                range: symbol.selection_range,
            })
    }

    pub fn implementation(&self, uri: &str, position: TextPosition) -> Vec<Location> {
        self.type_definition(uri, position).into_iter().collect()
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

    fn member_completion_items(&self, uri: &str, receiver: Option<&str>) -> Vec<CompletionItem> {
        self.index
            .symbols
            .iter()
            .filter(move |symbol| {
                let include = match receiver {
                    Some(receiver) => {
                        symbol.container_name.as_deref() == Some(receiver)
                            || symbol.uri.ends_with(&format!("/{receiver}.pkl"))
                            || symbol.uri == uri && symbol.name != receiver
                    }
                    None => symbol.uri == uri,
                };
                include
                    && matches!(
                        symbol.kind,
                        SymbolKind::Property | SymbolKind::Class | SymbolKind::TypeAlias
                    )
            })
            .map(symbol_completion_item)
            .collect()
    }

    fn completion_context(&self, uri: &str, position: TextPosition) -> CompletionContext {
        let Some(document) = self.documents.get(uri) else {
            return CompletionContext::Default;
        };
        let offset = offset_at_position(&document.text, position);
        let prefix = &document.text[..offset.min(document.text.len())];
        let trimmed = prefix.trim_end();
        if trimmed.ends_with('@') {
            return CompletionContext::Annotation;
        }
        if is_inside_import_string(prefix) {
            return CompletionContext::ImportString;
        }
        if trimmed.ends_with('.') || trimmed.ends_with("?.") {
            let receiver = receiver_before_dot(trimmed);
            return CompletionContext::Member { receiver };
        }
        CompletionContext::Default
    }
}

enum CompletionContext {
    Default,
    Member { receiver: Option<String> },
    Annotation,
    ImportString,
}

fn range_contains_offset(text: &str, range: TextRange, offset: usize) -> bool {
    let start = offset_at_position(text, range.start);
    let end = offset_at_position(text, range.end);
    start <= offset && offset <= end
}

fn completion_item(
    label: &str,
    kind: u32,
    detail: Option<&str>,
    documentation: Option<&str>,
) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(kind),
        detail: detail.map(ToOwned::to_owned),
        documentation: documentation.map(ToOwned::to_owned),
        sort_text: Some(format!("{kind:02}_{label}")),
        filter_text: Some(label.to_string()),
        insert_text: None,
        insert_text_format: None,
        text_edit: None,
    }
}

fn symbol_completion_item(symbol: &PklSymbol) -> CompletionItem {
    CompletionItem {
        label: symbol.name.clone(),
        kind: Some(match symbol.kind {
            SymbolKind::Module => protocol::completion_kind::MODULE,
            SymbolKind::Import => protocol::completion_kind::MODULE,
            SymbolKind::Property => protocol::completion_kind::PROPERTY,
            SymbolKind::Class => protocol::completion_kind::CLASS,
            SymbolKind::TypeAlias => protocol::completion_kind::TYPE_PARAMETER,
            SymbolKind::Annotation => protocol::completion_kind::REFERENCE,
        }),
        detail: symbol.detail.clone(),
        documentation: symbol
            .container_name
            .as_ref()
            .map(|container| format!("Container: {container}")),
        sort_text: Some(format!("20_{}", symbol.name)),
        filter_text: Some(symbol.name.clone()),
        insert_text: None,
        insert_text_format: None,
        text_edit: None,
    }
}

fn dedupe_completion_items(items: &mut Vec<CompletionItem>) {
    items.sort_by(|a, b| {
        a.sort_text
            .cmp(&b.sort_text)
            .then_with(|| a.label.cmp(&b.label))
    });
    items.dedup_by(|left, right| left.label == right.label);
}

fn semantic_class_for_token(token: &Token, index: &WorkspaceIndex) -> Option<(u32, u32)> {
    match &token.kind {
        TokenKind::StringLit(_) | TokenKind::InterpolatedString(_) => {
            Some((protocol::semantic_token_type::STRING, 0))
        }
        TokenKind::IntLit(_) | TokenKind::FloatLit(_) => {
            Some((protocol::semantic_token_type::NUMBER, 0))
        }
        TokenKind::BoolLit(_) | TokenKind::Null => Some((
            protocol::semantic_token_type::KEYWORD,
            protocol::semantic_token_modifier::READONLY,
        )),
        TokenKind::Ident(name) if BUILTIN_TYPES.contains(&name.as_str()) => {
            Some((protocol::semantic_token_type::TYPE, 0))
        }
        TokenKind::Ident(name) if BUILTIN_CONSTANTS.contains(&name.as_str()) => Some((
            protocol::semantic_token_type::VARIABLE,
            protocol::semantic_token_modifier::READONLY,
        )),
        TokenKind::Ident(name) => index
            .symbols
            .iter()
            .find(|symbol| symbol.name.trim_start_matches('@') == name)
            .map(|symbol| {
                let token_type = match symbol.kind {
                    SymbolKind::Module | SymbolKind::Import => {
                        protocol::semantic_token_type::NAMESPACE
                    }
                    SymbolKind::Property => protocol::semantic_token_type::PROPERTY,
                    SymbolKind::Class => protocol::semantic_token_type::CLASS,
                    SymbolKind::TypeAlias => protocol::semantic_token_type::TYPE,
                    SymbolKind::Annotation => protocol::semantic_token_type::MACRO,
                };
                (token_type, protocol::semantic_token_modifier::DEFINITION)
            })
            .or(Some((protocol::semantic_token_type::VARIABLE, 0))),
        kind if is_keyword_token(kind) => Some((protocol::semantic_token_type::KEYWORD, 0)),
        kind if is_operator_token(kind) => Some((protocol::semantic_token_type::OPERATOR, 0)),
        _ => None,
    }
}

fn is_keyword_token(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::KwAmends
            | TokenKind::KwImport
            | TokenKind::KwAs
            | TokenKind::KwLocal
            | TokenKind::KwConst
            | TokenKind::KwFixed
            | TokenKind::KwHidden
            | TokenKind::KwNew
            | TokenKind::KwExtends
            | TokenKind::KwAbstract
            | TokenKind::KwOpen
            | TokenKind::KwExternal
            | TokenKind::KwClass
            | TokenKind::KwTypeAlias
            | TokenKind::KwFunction
            | TokenKind::KwThis
            | TokenKind::KwSuper
            | TokenKind::KwModule
            | TokenKind::KwImportStar
            | TokenKind::KwIf
            | TokenKind::KwElse
            | TokenKind::KwWhen
            | TokenKind::KwIs
            | TokenKind::KwLet
            | TokenKind::KwThrow
            | TokenKind::KwTrace
            | TokenKind::KwRead
            | TokenKind::KwReadOrNull
            | TokenKind::KwFor
            | TokenKind::KwIn
    )
}

fn is_operator_token(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
            | TokenKind::Percent
            | TokenKind::EqEq
            | TokenKind::BangEq
            | TokenKind::Lt
            | TokenKind::Gt
            | TokenKind::LtEq
            | TokenKind::GtEq
            | TokenKind::TildeSlash
            | TokenKind::StarStar
            | TokenKind::AmpAmp
            | TokenKind::PipePipe
            | TokenKind::Arrow
            | TokenKind::ThinArrow
            | TokenKind::QuestionQuestion
            | TokenKind::PipeGt
            | TokenKind::Bang
            | TokenKind::BangBang
    )
}

fn token_text_range(text: &str, token: &Token) -> TextRange {
    let start = TextPosition {
        line: token.line.saturating_sub(1) as u32,
        character: token.col.saturating_sub(1) as u32,
    };
    let end = position_at_offset(text, token.offset.saturating_add(token_display_len(token)));
    TextRange { start, end }
}

fn token_display_len(token: &Token) -> usize {
    match &token.kind {
        TokenKind::Ident(value) | TokenKind::StringLit(value) => value.len(),
        TokenKind::IntLit(value) => value.to_string().len(),
        TokenKind::FloatLit(value) if value.is_nan() => 3,
        TokenKind::FloatLit(value) if value.is_infinite() => "Infinity".len(),
        TokenKind::FloatLit(value) => value.to_string().len(),
        TokenKind::BoolLit(true) => 4,
        TokenKind::BoolLit(false) => 5,
        TokenKind::Null => 4,
        TokenKind::KwImportStar => "import*".len(),
        TokenKind::KwReadOrNull => "read?".len(),
        TokenKind::DotDotDot => 3,
        TokenKind::QuestionQuestion
        | TokenKind::QuestionDot
        | TokenKind::BangBang
        | TokenKind::PipeGt
        | TokenKind::EqEq
        | TokenKind::BangEq
        | TokenKind::LtEq
        | TokenKind::GtEq
        | TokenKind::TildeSlash
        | TokenKind::StarStar
        | TokenKind::AmpAmp
        | TokenKind::PipePipe
        | TokenKind::Arrow
        | TokenKind::ThinArrow => 2,
        _ => 1,
    }
}

fn ranges_intersect(left: TextRange, right: TextRange) -> bool {
    position_le(left.start, right.end) && position_le(right.start, left.end)
}

fn position_le(left: TextPosition, right: TextPosition) -> bool {
    left.line < right.line || (left.line == right.line && left.character <= right.character)
}

fn word_range_at_position(text: &str, position: TextPosition) -> Option<TextRange> {
    let offset = offset_at_position(text, position);
    let bytes = text.as_bytes();
    let mut start = offset.min(bytes.len());
    while start > 0 && is_ident_byte(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = offset.min(bytes.len());
    while end < bytes.len() && is_ident_byte(bytes[end]) {
        end += 1;
    }
    (start < end).then(|| TextRange {
        start: position_at_offset(text, start),
        end: position_at_offset(text, end),
    })
}

fn entire_document_range_public(text: &str) -> TextRange {
    TextRange {
        start: TextPosition {
            line: 0,
            character: 0,
        },
        end: position_at_offset(text, text.len()),
    }
}

fn matching_delimiter(open: char, close: char) -> bool {
    matches!((open, close), ('{', '}') | ('[', ']') | ('(', ')'))
}

fn is_valid_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch.is_alphabetic() || ch == '_')
        && chars.all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '-')
}

fn is_inside_import_string(prefix: &str) -> bool {
    let trimmed = prefix.trim_end();
    let Some(last_quote) = trimmed.rfind('"') else {
        return false;
    };
    let before = trimmed[..last_quote].trim_end();
    before.ends_with("import") || before.ends_with("import*") || before.ends_with("amends")
}

fn receiver_before_dot(trimmed: &str) -> Option<String> {
    let without_dot = trimmed.trim_end_matches('.').trim_end_matches('?');
    let end = without_dot.len();
    let bytes = without_dot.as_bytes();
    let mut start = end;
    while start > 0 && is_ident_byte(bytes[start - 1]) {
        start -= 1;
    }
    (start < end).then(|| without_dot[start..end].to_string())
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
            "class App {\n  name: String\n}\nname: String = \"demo\"\nother = name\n",
            1,
        );
        let engine = FeatureEngine::new(docs);

        assert!(
            engine
                .hover(
                    "file:///demo.pkl",
                    TextPosition {
                        line: 0,
                        character: 7,
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
        assert!(completion.items.iter().any(|item| item.label == "String"));
        assert!(completion.items.iter().any(|item| item.label == "abstract"));
        assert!(
            engine
                .definition(
                    "file:///demo.pkl",
                    TextPosition {
                        line: 4,
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
                        line: 4,
                        character: 9,
                    },
                )
                .len(),
            3
        );
        assert!(
            !engine
                .semantic_tokens("file:///demo.pkl", None)
                .data
                .is_empty()
        );
        assert!(!engine.folding_ranges("file:///demo.pkl").is_empty());
        assert!(
            engine
                .prepare_rename(
                    "file:///demo.pkl",
                    TextPosition {
                        line: 4,
                        character: 9,
                    },
                )
                .is_some()
        );
        let edit = engine
            .rename(
                "file:///demo.pkl",
                TextPosition {
                    line: 4,
                    character: 9,
                },
                "newName",
            )
            .unwrap();
        assert_eq!(edit.changes[0].edits.len(), 3);
        assert!(
            engine
                .workspace_symbols("App")
                .iter()
                .any(|symbol| symbol.name == "App")
        );
    }
}
