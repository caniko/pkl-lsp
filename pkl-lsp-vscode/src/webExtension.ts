import * as vscode from "vscode";

type WasmServer = {
  open_document(uri: string, text: string, version: number): void;
  change_document(uri: string, text: string, version: number): void;
  close_document(uri: string): void;
  request(method: string, paramsJson: string): string;
};

let server: WasmServer | undefined;
const diagnostics = vscode.languages.createDiagnosticCollection("pkl-lsp");

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
      const result = request("textDocument/definition", document, position) as [string, vscode.Range] | null;
      return result ? new vscode.Location(vscode.Uri.parse(result[0]), result[1]) : undefined;
    }
  }));
  context.subscriptions.push(vscode.languages.registerReferenceProvider("pkl", {
    provideReferences(document, position) {
      syncChange(document);
      const result = request("textDocument/references", document, position) as [string, vscode.Range][];
      return result.map(([uri, range]) => new vscode.Location(vscode.Uri.parse(uri), range));
    }
  }));
  context.subscriptions.push(vscode.languages.registerDocumentSymbolProvider("pkl", {
    provideDocumentSymbols(document) {
      syncChange(document);
      return JSON.parse(server!.request("textDocument/documentSymbol", JSON.stringify({ uri: document.uri.toString() })));
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

