import * as vscode from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
} from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: vscode.ExtensionContext) {
    const config = vscode.workspace.getConfiguration('mano');
    const serverPath = config.get<string>('server.path') || 'mano-lsp';

    const serverOptions: ServerOptions = {
        command: serverPath,
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'mano' }],
    };

    client = new LanguageClient(
        'mano-lsp',
        'mano Language Server',
        serverOptions,
        clientOptions
    );

    client.start();

    const restartCommand = vscode.commands.registerCommand('mano.restartServer', async () => {
        await client.stop();
        await client.start();
        vscode.window.showInformationMessage('Mano Language Server restarted');
    });

    context.subscriptions.push(restartCommand);
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
