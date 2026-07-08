import * as vscode from "vscode";

type WasmServer = {
  open_document(uri: string, text: string, version: number): void;
  change_document(uri: string, text: string, version: number): void;
  close_document(uri: string): void;
  request(method: string, paramsJson: string): string;
};

type WasmLocation = {
  uri: string;
  range: WasmRange;
};

type WasmPosition = {
  line: number;
  character: number;
};

type WasmRange = {
  start: WasmPosition;
  end: WasmPosition;
};

type WasmWorkspaceSymbol = {
  name: string;
  kind: number;
  location: WasmLocation;
  containerName?: string;
};

type WasmSemanticToken = {
  line: number;
  start: number;
  length: number;
  tokenType: number;
  modifiers: number;
};

let server: WasmServer | undefined;
const diagnostics = vscode.languages.createDiagnosticCollection("pkl-lsp");
const semanticLegend = new vscode.SemanticTokensLegend(
  ["namespace", "type", "class", "parameter", "variable", "property", "function", "keyword", "comment", "string", "number", "operator", "macro"],
  ["declaration", "definition", "readonly", "deprecated"]
);

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const wasm = await import("../media/pkl_lsp_wasm.js");
  await wasm.default(context.extensionUri.with({ path: `${context.extensionUri.path}/media/pkl_lsp_wasm_bg.wasm` }).toString());
  server = wasm.create_server({});

  context.subscriptions.push(diagnostics);
  context.subscriptions.push(vscode.workspace.onDidOpenTextDocument(syncOpen));
  context.subscriptions.push(vscode.workspace.onDidChangeTextDocument((event) => syncChange(event.document)));
  context.subscriptions.push(vscode.workspace.onDidCloseTextDocument(syncClose));
  context.subscriptions.push(vscode.languages.registerHoverProvider("pkl", {
    provideHover(document, position) {
      syncChange(document);
      return request("textDocument/hover", document, position) as vscode.ProviderResult<vscode.Hover>;
    }
  }));
  context.subscriptions.push(vscode.languages.registerCompletionItemProvider("pkl", {
    provideCompletionItems(document, position) {
      syncChange(document);
      const response = request("textDocument/completion", document, position) as { items?: vscode.CompletionItem[] } | vscode.CompletionItem[];
      return Array.isArray(response) ? response : response?.items ?? [];
    }
  }, ".", "\"", "@"));
  context.subscriptions.push(vscode.languages.registerDefinitionProvider("pkl", {
    provideDefinition(document, position) {
      syncChange(document);
      const result = request("textDocument/definition", document, position) as WasmLocation | null;
      return result ? toLocation(result) : undefined;
    }
  }));
  context.subscriptions.push(vscode.languages.registerTypeDefinitionProvider("pkl", {
    provideTypeDefinition(document, position) {
      syncChange(document);
      const result = request("textDocument/typeDefinition", document, position) as WasmLocation | null;
      return result ? toLocation(result) : undefined;
    }
  }));
  context.subscriptions.push(vscode.languages.registerImplementationProvider("pkl", {
    provideImplementation(document, position) {
      syncChange(document);
      const result = request("textDocument/implementation", document, position) as WasmLocation[];
      return result.map(toLocation);
    }
  }));
  context.subscriptions.push(vscode.languages.registerReferenceProvider("pkl", {
    provideReferences(document, position) {
      syncChange(document);
      const result = request("textDocument/references", document, position) as WasmLocation[];
      return result.map(toLocation);
    }
  }));
  context.subscriptions.push(vscode.languages.registerDocumentHighlightProvider("pkl", {
    provideDocumentHighlights(document, position) {
      syncChange(document);
      const result = request("textDocument/documentHighlight", document, position) as { range: WasmRange; kind?: number }[];
      return result.map((highlight) => new vscode.DocumentHighlight(toRange(highlight.range), highlight.kind));
    }
  }));
  context.subscriptions.push(vscode.languages.registerDocumentSymbolProvider("pkl", {
    provideDocumentSymbols(document) {
      syncChange(document);
      return JSON.parse(server!.request("textDocument/documentSymbol", JSON.stringify({ uri: document.uri.toString() })));
    }
  }));
  context.subscriptions.push(vscode.languages.registerWorkspaceSymbolProvider({
    provideWorkspaceSymbols(query) {
      const result = JSON.parse(server!.request("workspace/symbol", JSON.stringify({ query }))) as WasmWorkspaceSymbol[];
      return result.map((symbol) => {
        const workspaceSymbol = new vscode.SymbolInformation(
          symbol.name,
          toSymbolKind(symbol.kind),
          symbol.containerName ?? "",
          toLocation(symbol.location)
        );
        return workspaceSymbol;
      });
    }
  }));
  context.subscriptions.push(vscode.languages.registerDocumentSemanticTokensProvider("pkl", {
    provideDocumentSemanticTokens(document) {
      syncChange(document);
      const result = JSON.parse(server!.request("textDocument/semanticTokens/full", JSON.stringify({ uri: document.uri.toString() }))) as { data: WasmSemanticToken[] };
      const builder = new vscode.SemanticTokensBuilder(semanticLegend);
      result.data.forEach((token) => builder.push(token.line, token.start, token.length, token.tokenType, token.modifiers));
      return builder.build();
    }
  }, semanticLegend));
  context.subscriptions.push(vscode.languages.registerFoldingRangeProvider("pkl", {
    provideFoldingRanges(document) {
      syncChange(document);
      const result = JSON.parse(server!.request("textDocument/foldingRange", JSON.stringify({ uri: document.uri.toString() }))) as { startLine: number; startCharacter?: number; endLine: number; endCharacter?: number; kind?: string }[];
      return result.map((range) => new vscode.FoldingRange(range.startLine, range.endLine, vscode.FoldingRangeKind.Region));
    }
  }));
  context.subscriptions.push(vscode.languages.registerSelectionRangeProvider("pkl", {
    provideSelectionRanges(document, positions) {
      syncChange(document);
      const result = JSON.parse(server!.request("textDocument/selectionRange", JSON.stringify({
        uri: document.uri.toString(),
        positions: positions.map((position) => ({ line: position.line, character: position.character }))
      }))) as { range: WasmRange }[];
      return result.map((range) => new vscode.SelectionRange(toRange(range.range)));
    }
  }));
  context.subscriptions.push(vscode.languages.registerRenameProvider("pkl", {
    prepareRename(document, position) {
      syncChange(document);
      const result = request("textDocument/prepareRename", document, position) as { range: WasmRange; placeholder: string } | null;
      return result ? { range: toRange(result.range), placeholder: result.placeholder } : undefined;
    },
    provideRenameEdits(document, position, newName) {
      syncChange(document);
      const result = JSON.parse(server!.request("textDocument/rename", JSON.stringify({
        uri: document.uri.toString(),
        position: { line: position.line, character: position.character },
        newName
      }))) as { changes?: { uri: string; edits: { range: WasmRange; newText: string }[] }[] } | null;
      const edit = new vscode.WorkspaceEdit();
      result?.changes?.forEach((documentEdit) => {
        const uri = vscode.Uri.parse(documentEdit.uri);
        documentEdit.edits.forEach((textEdit) => edit.replace(uri, toRange(textEdit.range), textEdit.newText));
      });
      return edit;
    }
  }));
  context.subscriptions.push(vscode.languages.registerCodeActionsProvider("pkl", {
    provideCodeActions(document, range) {
      syncChange(document);
      const result = JSON.parse(server!.request("textDocument/codeAction", JSON.stringify({
        uri: document.uri.toString(),
        range: fromRange(range)
      }))) as { title: string; kind?: string }[];
      return result.map((action) => new vscode.CodeAction(action.title, action.kind ? vscode.CodeActionKind.Empty.append(action.kind) : undefined));
    }
  }));
  context.subscriptions.push(vscode.languages.registerSignatureHelpProvider("pkl", {
    provideSignatureHelp(document, position) {
      syncChange(document);
      return request("textDocument/signatureHelp", document, position) as vscode.SignatureHelp | null;
    }
  }, "(", ","));
  context.subscriptions.push(vscode.languages.registerInlayHintsProvider("pkl", {
    provideInlayHints(document, range) {
      syncChange(document);
      const result = JSON.parse(server!.request("textDocument/inlayHint", JSON.stringify({
        uri: document.uri.toString(),
        range: fromRange(range)
      }))) as { position: WasmPosition; label: string; kind?: number }[];
      return result.map((hint) => new vscode.InlayHint(toPosition(hint.position), hint.label, hint.kind));
    }
  }));

  vscode.workspace.textDocuments.filter((document) => document.languageId === "pkl").forEach(syncOpen);
}

export function deactivate(): void {
  diagnostics.clear();
}

function syncOpen(document: vscode.TextDocument): void {
  if (!server || document.languageId !== "pkl") {
    return;
  }
  server.open_document(document.uri.toString(), document.getText(), document.version);
  publishDiagnostics(document);
}

function syncChange(document: vscode.TextDocument): void {
  if (!server || document.languageId !== "pkl") {
    return;
  }
  server.change_document(document.uri.toString(), document.getText(), document.version);
  publishDiagnostics(document);
}

function syncClose(document: vscode.TextDocument): void {
  if (!server || document.languageId !== "pkl") {
    return;
  }
  server.close_document(document.uri.toString());
  diagnostics.delete(document.uri);
}

function publishDiagnostics(document: vscode.TextDocument): void {
  const result = JSON.parse(server!.request("pkl/diagnostics", JSON.stringify({ uri: document.uri.toString() }))) as vscode.Diagnostic[];
  diagnostics.set(document.uri, result);
}

function request(method: string, document: vscode.TextDocument, position: vscode.Position): unknown {
  return JSON.parse(server!.request(method, JSON.stringify({
    uri: document.uri.toString(),
    position: { line: position.line, character: position.character }
  })));
}

function toPosition(position: WasmPosition): vscode.Position {
  return new vscode.Position(position.line, position.character);
}

function toRange(range: WasmRange): vscode.Range {
  return new vscode.Range(toPosition(range.start), toPosition(range.end));
}

function fromRange(range: vscode.Range): { start: WasmPosition; end: WasmPosition } {
  return {
    start: { line: range.start.line, character: range.start.character },
    end: { line: range.end.line, character: range.end.character }
  };
}

function toLocation(location: WasmLocation): vscode.Location {
  return new vscode.Location(vscode.Uri.parse(location.uri), toRange(location.range));
}

function toSymbolKind(kind: number): vscode.SymbolKind {
  switch (kind) {
    case 2:
      return vscode.SymbolKind.Module;
    case 3:
      return vscode.SymbolKind.Namespace;
    case 5:
      return vscode.SymbolKind.Class;
    case 7:
      return vscode.SymbolKind.Property;
    case 12:
      return vscode.SymbolKind.Function;
    case 24:
      return vscode.SymbolKind.Event;
    case 26:
      return vscode.SymbolKind.TypeParameter;
    default:
      return vscode.SymbolKind.Variable;
  }
}
