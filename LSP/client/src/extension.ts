/* --------------------------------------------------------------------------------------------
 * XLang Language Server — VS Code extension client
 * ------------------------------------------------------------------------------------------ */

import { workspace, ExtensionContext, window } from "vscode";

import {
  Executable,
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient;

export async function activate(_context: ExtensionContext) {
  const traceOutputChannel = window.createOutputChannel("XLang Language Server");
  const command = process.env.SERVER_PATH || "xlang-language-server";
  const run: Executable = {
    command,
    options: {
      env: {
        ...process.env,
        RUST_LOG: "info",
      },
    },
  };
  const serverOptions: ServerOptions = {
    run,
    debug: run,
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "xlang" }],
    synchronize: {
      fileEvents: workspace.createFileSystemWatcher("**/*.x"),
    },
    traceOutputChannel,
  };

  client = new LanguageClient(
    "xlang-language-server",
    "XLang Language Server",
    serverOptions,
    clientOptions,
  );
  client.start();
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}
