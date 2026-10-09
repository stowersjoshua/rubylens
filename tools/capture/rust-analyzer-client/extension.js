'use strict';
// Stand-in for the official rust-analyzer VS Code extension (same extension id, so rbview's
// extensionDependencies are satisfied). Like the real one, it just launches the
// rust-analyzer binary over LSP; it also shows a status-bar check once indexing is done.
const vscode = require('vscode');
const { LanguageClient } = require('vscode-languageclient/node');
let client;
exports.activate = async (context) => {
  const cfg = vscode.workspace.getConfiguration('rust-analyzer');
  const status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
  status.text = '$(sync~spin) rust-analyzer';
  status.show();
  client = new LanguageClient(
    'rust-analyzer', 'rust-analyzer',
    { command: cfg.get('server.path') || 'rust-analyzer', options: { env: process.env } },
    {
      documentSelector: [{ scheme: 'file', language: 'rust' }],
      initializationOptions: Object.assign({}, vscode.workspace.getConfiguration().get('rust-analyzer')),
    },
  );
  // advertise serverStatus so we can show when indexing is done
  client.registerFeature({
    fillClientCapabilities(caps) { caps.experimental = Object.assign({}, caps.experimental, { serverStatusNotification: true }); },
    initialize() {}, getState() { return { kind: 'static' }; }, dispose() {},
  });
  context.subscriptions.push(status, client);
  await client.start();
  client.onNotification('experimental/serverStatus', (p) => {
    status.text = p.quiescent ? '$(check) rust-analyzer' : '$(sync~spin) rust-analyzer';
  });
};
exports.deactivate = () => client && client.stop();
