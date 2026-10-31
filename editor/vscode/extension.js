'use strict';
const vscode = require('vscode');
const grid = require('./columns');
let client;
function previewUri(uri) { return vscode.Uri.from({ scheme: 'pillar-preview', path: uri.path + '.rplr', query: uri.toString() }); }
async function replace(editor, result) {
  const full = new vscode.Range(editor.document.positionAt(0), editor.document.positionAt(editor.document.getText().length));
  if (await editor.edit(edit => edit.replace(full, result.text))) {
    const point = editor.document.positionAt(result.offset);
    editor.selection = new vscode.Selection(point, point);
    editor.revealRange(new vscode.Range(point, point));
  }
}
async function activate(context) {
  const { LanguageClient } = await import('vscode-languageclient/node');
  const compiler = vscode.workspace.getConfiguration('pillar').get('compilerPath', 'plrc');
  client = new LanguageClient('pillar', 'Pillar', { command: compiler, args: ['lsp'] }, {
    documentSelector: [{ scheme: 'file', language: 'pillar' }]
  });
  context.subscriptions.push(client);
  client.start().catch(error => vscode.window.showErrorMessage(`Unable to start Pillar. Check pillar.compilerPath: ${error.message}`));
  const changed = new vscode.EventEmitter();
  context.subscriptions.push(changed, vscode.workspace.registerTextDocumentContentProvider('pillar-preview', {
    onDidChange: changed.event,
    async provideTextDocumentContent(uri) {
      const document = await vscode.workspace.openTextDocument(vscode.Uri.parse(uri.query));
      return grid.toHorizontal(document.getText());
    }
  }));
  context.subscriptions.push(vscode.workspace.onDidChangeTextDocument(event => {
    if (event.document.languageId === 'pillar') changed.fire(previewUri(event.document.uri));
  }));
  for (const [name, action] of [['insertColumn', 'insert'], ['removeColumn', 'remove'], ['moveColumnLeft', 'left'], ['moveColumnRight', 'right']]) {
    context.subscriptions.push(vscode.commands.registerCommand(`pillar.${name}`, async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor || editor.document.languageId !== 'pillar') return;
      await replace(editor, grid.column(editor.document.getText(), editor.document.offsetAt(editor.selection.active), action));
    }));
  }
  context.subscriptions.push(vscode.commands.registerCommand('pillar.insertDownward', async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.languageId !== 'pillar') return;
    const text = await vscode.window.showInputBox({ prompt: 'Text to insert downward in the current column', ignoreFocusOut: true });
    if (text !== undefined) await replace(editor, grid.editDown(editor.document.getText(), editor.document.offsetAt(editor.selection.active), text));
  }));
  context.subscriptions.push(vscode.commands.registerCommand('pillar.preview', async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.languageId !== 'pillar') return;
    const document = await vscode.workspace.openTextDocument(previewUri(editor.document.uri));
    await vscode.window.showTextDocument(document, { viewColumn: vscode.ViewColumn.Beside, preserveFocus: true, preview: true });
  }));
}
async function deactivate() { if (client) await client.stop(); }
module.exports = { activate, deactivate };
