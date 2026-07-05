import * as vscode from "vscode";
import { LanguageClient, LanguageClientOptions, ServerOptions } from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const config = vscode.workspace.getConfiguration("pklLsp");
  const command = resolveServerPath(context, config.get<string>("serverPath", ""));
  const serverOptions: ServerOptions = {
    command,
    args: ["--stdio"]
  };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "pkl" }],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/*.pkl")
    }
  };
  client = new LanguageClient("pkl-lsp", "PKL LSP", serverOptions, clientOptions);
  context.subscriptions.push(client);
  await client.start();
}

function resolveServerPath(context: vscode.ExtensionContext, configuredPath: string): string {
  const trimmed = configuredPath.trim();
  if (trimmed.length > 0) {
    return trimmed;
  }

  const target = bundledTarget();
  if (!target) {
    return "pkl-lsp";
  }

  const executable = process.platform === "win32" ? "pkl-lsp.exe" : "pkl-lsp";
  return vscode.Uri.joinPath(context.extensionUri, "bin", target, executable).fsPath;
}

function bundledTarget(): string | undefined {
  const platform = process.platform;
  const arch = process.arch;

  if (platform === "linux" && arch === "x64") {
    return "linux-x64";
  }
  if (platform === "linux" && arch === "arm64") {
    return "linux-arm64";
  }
  if (platform === "darwin" && arch === "x64") {
    return "darwin-x64";
  }
  if (platform === "darwin" && arch === "arm64") {
    return "darwin-arm64";
  }
  if (platform === "win32" && arch === "x64") {
    return "win32-x64";
  }
  if (platform === "win32" && arch === "arm64") {
    return "win32-arm64";
  }

  return undefined;
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
    client = undefined;
  }
}
