// Minimal Seen extension - syntax highlighting only
import * as vscode from 'vscode';

export function activate(context: vscode.ExtensionContext) {
    console.log('🚀 Seen Language extension is activating (minimal mode)...');
    
    // Just provide basic file association and syntax highlighting
    // The syntax highlighting is handled by syntaxes/seen.tmLanguage.json
    
    vscode.window.showInformationMessage('Seen Language support loaded! (Syntax highlighting enabled)');
    
    // Register a simple command
    const helloCommand = vscode.commands.registerCommand('seen.helloWorld', () => {
        vscode.window.showInformationMessage('Hello from Seen Language!');
    });
    
    context.subscriptions.push(helloCommand);
    
    console.log('✅ Seen Language extension activated successfully (minimal mode)');
}

export function deactivate(): void {
    console.log('Seen Language extension deactivated');
}