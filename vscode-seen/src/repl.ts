// src/repl.ts
import * as vscode from 'vscode';

export const REPL_UNSUPPORTED_MESSAGE =
    'seen repl is not supported by the shipped compiler.';

function warnReplUnsupported(): void {
    vscode.window.showWarningMessage(REPL_UNSUPPORTED_MESSAGE);
}

export class SeenReplProvider implements vscode.TerminalProfileProvider {
    provideTerminalProfile(
        _token: vscode.CancellationToken
    ): vscode.ProviderResult<vscode.TerminalProfile> {
        warnReplUnsupported();
        return undefined;
    }
}

export class SeenReplManager {
    static createRepl(): undefined {
        warnReplUnsupported();
        return undefined;
    }

    static sendToRepl(_code: string): void {
        warnReplUnsupported();
    }

    static sendSelectionToRepl(): void {
        warnReplUnsupported();
    }

    static sendFileToRepl(): void {
        warnReplUnsupported();
    }
}

export function registerReplCommands(context: vscode.ExtensionContext) {
    // Send selection or line to REPL
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.repl.sendSelection', () => {
            SeenReplManager.sendSelectionToRepl();
        })
    );

    // Send entire file to REPL
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.repl.sendFile', () => {
            SeenReplManager.sendFileToRepl();
        })
    );

    // Clear REPL
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.repl.clear', () => {
            SeenReplManager.sendToRepl(':clear');
        })
    );

    // Show REPL help
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.repl.help', () => {
            SeenReplManager.sendToRepl(':help');
        })
    );

    // Restart REPL
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.repl.restart', () => {
            warnReplUnsupported();
        })
    );

    // Register context menu items for REPL
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.repl.sendSelectionContext', () => {
            SeenReplManager.sendSelectionToRepl();
        })
    );
}
