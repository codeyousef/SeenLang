// src/repl.ts
import * as vscode from 'vscode';

export class SeenReplProvider implements vscode.TerminalProfileProvider {
    constructor(private seenPath: string) {}

    provideTerminalProfile(
        token: vscode.CancellationToken
    ): vscode.ProviderResult<vscode.TerminalProfile> {
        return {
            options: {
                name: 'Seen REPL',
                shellPath: this.seenPath,
                shellArgs: ['repl'],
                iconPath: new vscode.ThemeIcon('console'),
                color: new vscode.ThemeColor('terminal.ansiBlue'),
                env: {
                    SEEN_REPL_MODE: '1',
                    SEEN_COLOR: 'always'
                }
            }
        };
    }
}

export class SeenReplManager {
    private static activeRepl: vscode.Terminal | undefined;

    static createRepl(): vscode.Terminal {
        if (this.activeRepl && this.activeRepl.exitStatus === undefined) {
            this.activeRepl.show();
            return this.activeRepl;
        }

        const seenPath = vscode.workspace.getConfiguration('seen').get<string>('compiler.path', 'seen');
        
        this.activeRepl = vscode.window.createTerminal({
            name: 'Seen REPL',
            shellPath: seenPath,
            shellArgs: ['repl'],
            iconPath: new vscode.ThemeIcon('console'),
            env: {
                SEEN_REPL_MODE: '1',
                SEEN_COLOR: 'always',
                SEEN_REPL_HISTORY: '1'
            }
        });

        // Show welcome message
        this.activeRepl.show();
        
        // Handle terminal disposal
        vscode.window.onDidCloseTerminal(terminal => {
            if (terminal === this.activeRepl) {
                this.activeRepl = undefined;
            }
        });

        return this.activeRepl;
    }

    static sendToRepl(code: string) {
        const repl = this.createRepl();
        repl.sendText(code, true);
    }

    static sendSelectionToRepl() {
        const editor = vscode.window.activeTextEditor;
        if (!editor) {
            return;
        }

        const selection = editor.selection;
        let code: string;

        if (selection.isEmpty) {
            // Send current line
            const lineRange = editor.document.lineAt(selection.active.line).range;
            code = editor.document.getText(lineRange);
        } else {
            // Send selected text
            code = editor.document.getText(selection);
        }

        if (code.trim()) {
            this.sendToRepl(code);
        }
    }

    static sendFileToRepl() {
        const editor = vscode.window.activeTextEditor;
        if (!editor || editor.document.languageId !== 'seen') {
            vscode.window.showErrorMessage('No Seen file is currently open');
            return;
        }

        // Save file first if modified
        if (editor.document.isDirty) {
            editor.document.save();
        }

        const filePath = editor.document.fileName;
        this.sendToRepl(`:load "${filePath}"`);
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
            const currentRepl = (SeenReplManager as any).activeRepl;
            if (currentRepl) {
                currentRepl.dispose();
            }
            SeenReplManager.createRepl();
        })
    );

    // Add keybindings for REPL commands
    const config = vscode.workspace.getConfiguration('seen.repl');
    
    // Register context menu items for REPL
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.repl.sendSelectionContext', () => {
            SeenReplManager.sendSelectionToRepl();
        })
    );
}