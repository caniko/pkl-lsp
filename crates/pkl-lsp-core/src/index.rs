use lsp_types::{
    Diagnostic, DiagnosticSeverity, DocumentSymbol, Position, Range, SymbolKind as LspSymbolKind,
};
use pklr::error::Error as PklError;
use pklr::lexer::{self, Token, TokenKind};
use pklr::parser::{self, Annotation, Entry, Expr, Import, Module, Property, TypeExpr};
use rkyv::{Archive, Deserialize, Serialize};

use crate::Document;

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub enum DiagnosticSource {
    Lex,
    Parse,
    Eval,
    Io,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct PklDiagnostic {
    pub uri: String,
    pub range: TextRange,
    pub message: String,
    pub source: DiagnosticSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct TextPosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct TextRange {
    pub start: TextPosition,
    pub end: TextPosition,
}

impl From<TextPosition> for Position {
    fn from(value: TextPosition) -> Self {
        Position::new(value.line, value.character)
    }
}

impl From<Position> for TextPosition {
    fn from(value: Position) -> Self {
        Self {
            line: value.line,
            character: value.character,
        }
    }
}

impl From<TextRange> for Range {
    fn from(value: TextRange) -> Self {
        Range::new(value.start.into(), value.end.into())
    }
}

impl From<Range> for TextRange {
    fn from(value: Range) -> Self {
        Self {
            start: value.start.into(),
            end: value.end.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub enum SymbolKind {
    Module,
    Import,
    Property,
    Class,
    TypeAlias,
    Annotation,
}

impl SymbolKind {
    pub fn as_lsp(self) -> LspSymbolKind {
        match self {
            Self::Module => LspSymbolKind::MODULE,
            Self::Import => LspSymbolKind::NAMESPACE,
            Self::Property => LspSymbolKind::PROPERTY,
            Self::Class => LspSymbolKind::CLASS,
            Self::TypeAlias => LspSymbolKind::TYPE_PARAMETER,
            Self::Annotation => LspSymbolKind::EVENT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct PklSymbol {
    pub uri: String,
    pub name: String,
    pub kind: SymbolKind,
    pub range: TextRange,
    pub selection_range: TextRange,
    pub detail: Option<String>,
    pub container_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub enum ImportKind {
    Import,
    GlobImport,
    Amends,
    Extends,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct ImportEdge {
    pub uri: String,
    pub target: String,
    pub alias: Option<String>,
    pub kind: ImportKind,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct WorkspaceIndex {
    pub diagnostics: Vec<PklDiagnostic>,
    pub imports: Vec<ImportEdge>,
    pub symbols: Vec<PklSymbol>,
}

impl WorkspaceIndex {
    pub fn build<'a>(documents: impl IntoIterator<Item = &'a Document>) -> Self {
        let mut index = Self {
            diagnostics: Vec::new(),
            imports: Vec::new(),
            symbols: Vec::new(),
        };

        for document in documents {
            index.index_document(document);
        }

        index
    }

    pub fn diagnostics_for_lsp(&self, uri: &str) -> Vec<Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.uri == uri)
            .map(|diagnostic| Diagnostic {
                range: diagnostic.range.into(),
                severity: Some(match diagnostic.source {
                    DiagnosticSource::Lex
                    | DiagnosticSource::Parse
                    | DiagnosticSource::Eval
                    | DiagnosticSource::Io => DiagnosticSeverity::ERROR,
                }),
                source: Some("pkl-lsp".to_string()),
                message: diagnostic.message.clone(),
                ..Diagnostic::default()
            })
            .collect()
    }

    #[allow(deprecated)]
    pub fn document_symbols(&self, uri: &str) -> Vec<DocumentSymbol> {
        self.symbols
            .iter()
            .filter(|symbol| symbol.uri == uri)
            .map(|symbol| DocumentSymbol {
                name: symbol.name.clone(),
                detail: symbol.detail.clone(),
                kind: symbol.kind.clone().as_lsp(),
                tags: None,
                deprecated: None,
                range: symbol.range.into(),
                selection_range: symbol.selection_range.into(),
                children: None,
            })
            .collect()
    }

    pub fn symbols_in_uri(&self, uri: &str) -> impl Iterator<Item = &PklSymbol> {
        self.symbols.iter().filter(move |symbol| symbol.uri == uri)
    }

    fn index_document(&mut self, document: &Document) {
        let module_symbol = PklSymbol {
            uri: document.uri.clone(),
            name: module_name(&document.uri),
            kind: SymbolKind::Module,
            range: entire_document_range(&document.text),
            selection_range: position_range(0, 0, 0, 0),
            detail: Some("Pkl module".to_string()),
            container_name: None,
        };
        self.symbols.push(module_symbol);

        let tokens = match lexer::lex_named(&document.text, &document.uri) {
            Ok(tokens) => tokens,
            Err(error) => {
                self.diagnostics
                    .push(error_to_diagnostic(&document.uri, &document.text, error));
                return;
            }
        };

        let module = match parser::parse_named(&tokens, &document.text, &document.uri) {
            Ok(module) => module,
            Err(error) => {
                self.diagnostics
                    .push(error_to_diagnostic(&document.uri, &document.text, error));
                self.collect_fast_imports(document, &tokens);
                return;
            }
        };

        self.collect_module(document, &tokens, &module);
    }

    fn collect_fast_imports(&mut self, document: &Document, tokens: &[Token]) {
        for uri in parser::collect_imports(tokens) {
            if let Some(token) = find_string_token(tokens, &uri) {
                self.imports.push(ImportEdge {
                    uri: document.uri.clone(),
                    target: uri,
                    alias: None,
                    kind: ImportKind::Import,
                    range: token_range(token),
                });
            }
        }
    }

    fn collect_module(&mut self, document: &Document, tokens: &[Token], module: &Module) {
        if let Some(amends) = &module.amends {
            self.push_module_edge(document, tokens, amends, ImportKind::Amends, None);
        }
        if let Some(extends) = &module.extends {
            self.push_module_edge(document, tokens, extends, ImportKind::Extends, None);
        }
        for import in &module.imports {
            self.push_import(document, tokens, import);
        }
        let mut cursor = TokenCursor::new(tokens);
        self.collect_annotations(document, &module.annotations, &mut cursor, None);
        self.collect_entries(document, &module.body, &mut cursor, None);
    }

    fn push_import(&mut self, document: &Document, tokens: &[Token], import: &Import) {
        let kind = if import.is_glob {
            ImportKind::GlobImport
        } else {
            ImportKind::Import
        };
        self.push_module_edge(document, tokens, &import.uri, kind, import.alias.clone());
        let name = import
            .alias
            .clone()
            .unwrap_or_else(|| import_name_from_uri(&import.uri));
        if let Some(token) = find_ident_or_string_token(tokens, &name, &import.uri) {
            self.symbols.push(PklSymbol {
                uri: document.uri.clone(),
                name,
                kind: SymbolKind::Import,
                range: token_range(token),
                selection_range: token_range(token),
                detail: Some(format!("import {}", import.uri)),
                container_name: None,
            });
        }
    }

    fn push_module_edge(
        &mut self,
        document: &Document,
        tokens: &[Token],
        target: &str,
        kind: ImportKind,
        alias: Option<String>,
    ) {
        let range = find_string_token(tokens, target)
            .map_or_else(|| entire_document_range(&document.text), token_range);
        self.imports.push(ImportEdge {
            uri: document.uri.clone(),
            target: target.to_string(),
            alias,
            kind,
            range,
        });
    }

    fn collect_annotations(
        &mut self,
        document: &Document,
        annotations: &[Annotation],
        cursor: &mut TokenCursor<'_>,
        container_name: Option<&str>,
    ) {
        for annotation in annotations {
            if let Some(token) = cursor.next_ident(&annotation.name) {
                self.symbols.push(PklSymbol {
                    uri: document.uri.clone(),
                    name: format!("@{}", annotation.name),
                    kind: SymbolKind::Annotation,
                    range: token_range(token),
                    selection_range: token_range(token),
                    detail: Some("annotation".to_string()),
                    container_name: container_name.map(ToOwned::to_owned),
                });
            }
            self.collect_entries(document, &annotation.body, cursor, Some(&annotation.name));
        }
    }

    fn collect_entries(
        &mut self,
        document: &Document,
        entries: &[Entry],
        cursor: &mut TokenCursor<'_>,
        container_name: Option<&str>,
    ) {
        for entry in entries {
            match entry {
                Entry::Property(property) => {
                    self.collect_property(document, property, cursor, container_name)
                }
                Entry::ClassDef(name, _, parent, body) => {
                    if let Some(token) = cursor.next_ident(name) {
                        self.symbols.push(PklSymbol {
                            uri: document.uri.clone(),
                            name: name.clone(),
                            kind: SymbolKind::Class,
                            range: token_range(token),
                            selection_range: token_range(token),
                            detail: parent.as_ref().map(|parent| format!("extends {parent}")),
                            container_name: container_name.map(ToOwned::to_owned),
                        });
                    }
                    self.collect_entries(document, body, cursor, Some(name));
                }
                Entry::TypeAlias(name, ty) => {
                    if let Some(token) = cursor.next_ident(name) {
                        self.symbols.push(PklSymbol {
                            uri: document.uri.clone(),
                            name: name.clone(),
                            kind: SymbolKind::TypeAlias,
                            range: token_range(token),
                            selection_range: token_range(token),
                            detail: Some(type_expr_label(ty)),
                            container_name: container_name.map(ToOwned::to_owned),
                        });
                    }
                }
                Entry::DynProperty(key, value) => {
                    collect_expr(cursor, key);
                    collect_expr(cursor, value);
                }
                Entry::ForGenerator(generator) => {
                    self.collect_entries(document, &generator.body, cursor, container_name)
                }
                Entry::WhenGenerator(generator) => {
                    self.collect_entries(document, &generator.body, cursor, container_name);
                    if let Some(else_body) = &generator.else_body {
                        self.collect_entries(document, else_body, cursor, container_name);
                    }
                }
                Entry::Spread(expr) | Entry::Elem(expr) => collect_expr(cursor, expr),
            }
        }
    }

    fn collect_property(
        &mut self,
        document: &Document,
        property: &Property,
        cursor: &mut TokenCursor<'_>,
        container_name: Option<&str>,
    ) {
        self.collect_annotations(
            document,
            &property.annotations,
            cursor,
            Some(&property.name),
        );
        if let Some(token) = cursor.next_ident(&property.name) {
            let detail = property.type_ann.as_ref().map(type_expr_label);
            self.symbols.push(PklSymbol {
                uri: document.uri.clone(),
                name: property.name.clone(),
                kind: SymbolKind::Property,
                range: token_range(token),
                selection_range: token_range(token),
                detail,
                container_name: container_name.map(ToOwned::to_owned),
            });
        }
        if let Some(value) = &property.value {
            collect_expr(cursor, value);
        }
        if let Some(body) = &property.body {
            self.collect_entries(document, body, cursor, Some(&property.name));
        }
    }
}

fn collect_expr(_cursor: &mut TokenCursor<'_>, _expr: &Expr) {}

fn error_to_diagnostic(uri: &str, text: &str, error: PklError) -> PklDiagnostic {
    match &error {
        PklError::Lex { message, .. } => PklDiagnostic {
            uri: uri.to_string(),
            range: offset_range(text, error.source_offset().unwrap_or_default(), 1),
            message: message.clone(),
            source: DiagnosticSource::Lex,
        },
        PklError::Parse { message, .. } => PklDiagnostic {
            uri: uri.to_string(),
            range: offset_range(text, error.source_offset().unwrap_or_default(), 1),
            message: message.clone(),
            source: DiagnosticSource::Parse,
        },
        PklError::Io(path, error) => PklDiagnostic {
            uri: uri.to_string(),
            range: position_range(0, 0, 0, 1),
            message: format!("IO error reading {}: {error}", path.display()),
            source: DiagnosticSource::Io,
        },
        PklError::Eval(message)
        | PklError::ImportNotFound(message)
        | PklError::Unsupported(message) => PklDiagnostic {
            uri: uri.to_string(),
            range: position_range(0, 0, 0, 1),
            message: message.clone(),
            source: DiagnosticSource::Eval,
        },
    }
}

pub fn position_at_offset(text: &str, offset: usize) -> TextPosition {
    let mut line = 0u32;
    let mut line_start = 0usize;
    for (idx, ch) in text.char_indices() {
        if idx >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            line_start = idx + ch.len_utf8();
        }
    }
    TextPosition {
        line,
        character: text[line_start..offset.min(text.len())].chars().count() as u32,
    }
}

pub fn offset_at_position(text: &str, position: Position) -> usize {
    let mut line = 0u32;
    let mut character = 0u32;
    for (idx, ch) in text.char_indices() {
        if line == position.line && character == position.character {
            return idx;
        }
        if ch == '\n' {
            line += 1;
            character = 0;
            if line > position.line {
                return idx;
            }
        } else {
            character += 1;
        }
    }
    text.len()
}

pub fn word_at_position(text: &str, position: Position) -> Option<String> {
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
    (start < end).then(|| text[start..end].to_string())
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

fn offset_range(text: &str, offset: usize, len: usize) -> TextRange {
    TextRange {
        start: position_at_offset(text, offset),
        end: position_at_offset(text, offset.saturating_add(len).min(text.len())),
    }
}

fn token_range(token: &Token) -> TextRange {
    let start_line = token.line.saturating_sub(1) as u32;
    let start_col = token.col.saturating_sub(1) as u32;
    let len = token_len(token) as u32;
    position_range(
        start_line,
        start_col,
        start_line,
        start_col.saturating_add(len.max(1)),
    )
}

fn token_len(token: &Token) -> usize {
    match &token.kind {
        TokenKind::Ident(value) | TokenKind::StringLit(value) => value.chars().count(),
        TokenKind::IntLit(value) => value.to_string().len(),
        TokenKind::FloatLit(value) => value.to_string().len(),
        TokenKind::BoolLit(value) => value.to_string().len(),
        TokenKind::Null => 4,
        _ => 1,
    }
}

pub fn position_range(start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> TextRange {
    TextRange {
        start: TextPosition {
            line: start_line,
            character: start_col,
        },
        end: TextPosition {
            line: end_line,
            character: end_col,
        },
    }
}

fn entire_document_range(text: &str) -> TextRange {
    let end = position_at_offset(text, text.len());
    TextRange {
        start: TextPosition {
            line: 0,
            character: 0,
        },
        end,
    }
}

fn find_string_token<'a>(tokens: &'a [Token], value: &str) -> Option<&'a Token> {
    tokens
        .iter()
        .find(|token| matches!(&token.kind, TokenKind::StringLit(text) if text == value))
}

fn find_ident_or_string_token<'a>(
    tokens: &'a [Token],
    ident: &str,
    string: &str,
) -> Option<&'a Token> {
    tokens.iter().find(|token| match &token.kind {
        TokenKind::Ident(text) => text == ident,
        TokenKind::StringLit(text) => text == string,
        _ => false,
    })
}

fn module_name(uri: &str) -> String {
    uri.rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(uri)
        .trim_end_matches(".pkl")
        .to_string()
}

fn import_name_from_uri(uri: &str) -> String {
    uri.rsplit('/')
        .next()
        .unwrap_or(uri)
        .trim_end_matches(".pkl")
        .replace('-', "_")
}

fn type_expr_label(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Named(name) => name.clone(),
        TypeExpr::Nullable(inner) => format!("{}?", type_expr_label(inner)),
        TypeExpr::Union(items) => items
            .iter()
            .map(type_expr_label)
            .collect::<Vec<_>>()
            .join(" | "),
        TypeExpr::Generic(name, args) => {
            format!(
                "{name}<{}>",
                args.iter()
                    .map(type_expr_label)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        TypeExpr::Constrained(name, _) => format!("{name}(...)"),
    }
}

struct TokenCursor<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> TokenCursor<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn next_ident(&mut self, name: &str) -> Option<&'a Token> {
        let found = self.tokens[self.pos..]
            .iter()
            .position(|token| matches!(&token.kind, TokenKind::Ident(text) if text == name))?;
        self.pos += found + 1;
        self.tokens.get(self.pos - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::{ImportKind, SymbolKind, WorkspaceIndex};
    use crate::Document;

    #[test]
    fn extracts_symbols_and_imports() {
        let document = Document {
            uri: "file:///demo.pkl".to_string(),
            version: 1,
            text: r#"
import "base.pkl" as base
amends "parent.pkl"

@Deprecated { message = "old" }
class AppConfig {
  name: String = "demo"
}

typealias Name = String
"#
            .to_string(),
        };

        let index = WorkspaceIndex::build([&document]);

        assert!(index.diagnostics.is_empty(), "{:?}", index.diagnostics);
        assert!(
            index
                .symbols
                .iter()
                .any(|symbol| symbol.name == "AppConfig" && symbol.kind == SymbolKind::Class)
        );
        assert!(
            index
                .symbols
                .iter()
                .any(|symbol| symbol.name == "Name" && symbol.kind == SymbolKind::TypeAlias)
        );
        assert!(
            index
                .imports
                .iter()
                .any(|edge| edge.target == "parent.pkl" && edge.kind == ImportKind::Amends)
        );
        assert!(
            index
                .imports
                .iter()
                .any(|edge| edge.alias.as_deref() == Some("base"))
        );
    }

    #[test]
    fn reports_parse_diagnostics_with_ranges() {
        let document = Document {
            uri: "file:///bad.pkl".to_string(),
            version: 1,
            text: "name = ".to_string(),
        };

        let index = WorkspaceIndex::build([&document]);
        assert_eq!(index.diagnostics.len(), 1);
        assert_eq!(index.diagnostics[0].range.start.line, 0);
    }
}
