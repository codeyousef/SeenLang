// src/debugger.ts
import * as vscode from 'vscode';
import { DebugAdapterExecutable, DebugConfiguration, WorkspaceFolder } from 'vscode';

export class SeenDebugAdapterFactory implements vscode.DebugAdapterDescriptorFactory {
    createDebugAdapterDescriptor(
        session: vscode.DebugSession,
        executable: DebugAdapterExecutable | undefined
    ): vscode.ProviderResult<vscode.DebugAdapterDescriptor> {
        
        // Use the Seen CLI as debug adapter
        const seenPath = vscode.workspace.getConfiguration('seen').get<string>('compiler.path', 'seen');
        
        return new vscode.DebugAdapterExecutable(seenPath, ['debug-adapter']);
    }
}

export class SeenDebugConfigurationProvider implements vscode.DebugConfigurationProvider {
    
    resolveDebugConfiguration(
        folder: WorkspaceFolder | undefined,
        config: DebugConfiguration,
        token?: vscode.CancellationToken
    ): vscode.ProviderResult<DebugConfiguration> {
        
        // If launch.json is missing or empty
        if (!config.type && !config.request && !config.name) {
            const editor = vscode.window.activeTextEditor;
            if (editor && editor.document.languageId === 'seen') {
                config.type = 'seen';
                config.name = 'Launch Seen Program';
                config.request = 'launch';
                config.program = '${workspaceFolder}/src/main.seen';
                config.console = 'integratedTerminal';
                config.internalConsoleOptions = 'neverOpen';
            }
        }

        // Ensure required properties are set
        if (!config.program) {
            return vscode.window.showErrorMessage(
                "Cannot find a program to debug"
            ).then(_ => {
                return undefined; // abort launch
            });
        }

        return config;
    }
}

export class SeenDebugSession {
    // This would implement the Debug Adapter Protocol
    // For now, we'll use the CLI's debug functionality
}

// Register debug configuration provider
export function registerDebugSupport(context: vscode.ExtensionContext) {
    const provider = new SeenDebugConfigurationProvider();
    context.subscriptions.push(
        vscode.debug.registerDebugConfigurationProvider('seen', provider)
    );
    
    // Add debug configuration snippets
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.debug.getCurrentFile', () => {
            const editor = vscode.window.activeTextEditor;
            if (editor && editor.document.languageId === 'seen') {
                return editor.document.fileName;
            }
            return undefined;
        })
    );
    
    // Debug hover provider for variable inspection
    const debugHoverProvider = new SeenDebugHoverProvider();
    context.subscriptions.push(
        vscode.languages.registerHoverProvider('seen', debugHoverProvider)
    );
}

class SeenDebugHoverProvider implements vscode.HoverProvider {
    provideHover(
        document: vscode.TextDocument,
        position: vscode.Position,
        token: vscode.CancellationToken
    ): vscode.ProviderResult<vscode.Hover> {
        
        // Only provide debug info during debug sessions
        const activeSession = vscode.debug.activeDebugSession;
        if (!activeSession || activeSession.type !== 'seen') {
            return null;
        }
        
        const range = document.getWordRangeAtPosition(position);
        if (!range) {
            return null;
        }
        
        const word = document.getText(range);
        
        // This would query the debug adapter for variable values
        // For now, return a placeholder
        return new vscode.Hover(
            new vscode.MarkdownString(`**Debug Info**: \`${word}\`\n\nValue inspection during debugging.`),
            range
        );
    }
}