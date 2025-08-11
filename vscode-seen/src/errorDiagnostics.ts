// Error Diagnostics Provider for Seen Language VSCode Extension
// Handles new error format from Alpha Phase compiler

import * as vscode from 'vscode';
import { spawn } from 'child_process';
import * as path from 'path';

export interface SeenError {
    code: string;
    severity: 'error' | 'warning' | 'note' | 'help';
    message: string;
    file: string;
    location: {
        line: number;
        column: number;
        length: number;
    };
    suggestions: string[];
}

export class SeenDiagnosticProvider {
    private diagnosticCollection: vscode.DiagnosticCollection;
    private errorDecorationType: vscode.TextEditorDecorationType;
    private warningDecorationType: vscode.TextEditorDecorationType;
    
    constructor() {
        this.diagnosticCollection = vscode.languages.createDiagnosticCollection('seen');
        
        // Create decoration types for inline error hints
        this.errorDecorationType = vscode.window.createTextEditorDecorationType({
            after: {
                color: new vscode.ThemeColor('errorForeground'),
                fontStyle: 'italic',
                margin: '0 0 0 1em'
            },
            isWholeLine: false
        });
        
        this.warningDecorationType = vscode.window.createTextEditorDecorationType({
            after: {
                color: new vscode.ThemeColor('editorWarning.foreground'),
                fontStyle: 'italic',
                margin: '0 0 0 1em'
            },
            isWholeLine: false
        });
    }
    
    public async updateDiagnostics(document: vscode.TextDocument): Promise<void> {
        if (document.languageId !== 'seen') {
            return;
        }
        
        const errors = await this.compileAndGetErrors(document);
        this.setDiagnostics(document.uri, errors);
        this.updateInlineDecorations(document, errors);
    }
    
    private async compileAndGetErrors(document: vscode.TextDocument): Promise<SeenError[]> {
        return new Promise((resolve) => {
            const seenPath = vscode.workspace.getConfiguration('seen').get<string>('compiler.path', 'seen');
            const args = ['check', '--json-errors', document.fileName];
            
            let errorOutput = '';
            const process = spawn(seenPath, args, {
                cwd: vscode.workspace.workspaceFolders?.[0].uri.fsPath
            });
            
            process.stderr.on('data', (data) => {
                errorOutput += data.toString();
            });
            
            process.on('close', () => {
                try {
                    const errors = JSON.parse(errorOutput) as SeenError[];
                    resolve(errors);
                } catch {
                    // If not JSON, parse traditional error format as fallback
                    resolve(this.parseTraditionalErrors(errorOutput));
                }
            });
            
            // Send document content to stdin if file is unsaved
            if (document.isDirty) {
                process.stdin.write(document.getText());
                process.stdin.end();
            }
        });
    }
    
    private parseTraditionalErrors(output: string): SeenError[] {
        const errors: SeenError[] = [];
        const lines = output.split('\n');
        
        const errorRegex = /^(error|warning)\[([A-Z0-9]+)\]:\s*(.+)$/;
        const locationRegex = /^\s*-->\s*(.+):(\d+):(\d+)$/;
        
        let currentError: Partial<SeenError> | null = null;
        
        for (const line of lines) {
            const errorMatch = line.match(errorRegex);
            if (errorMatch) {
                if (currentError && currentError.file) {
                    errors.push(currentError as SeenError);
                }
                currentError = {
                    severity: errorMatch[1] as 'error' | 'warning',
                    code: errorMatch[2],
                    message: errorMatch[3],
                    suggestions: []
                };
                continue;
            }
            
            const locationMatch = line.match(locationRegex);
            if (locationMatch && currentError) {
                currentError.file = locationMatch[1];
                currentError.location = {
                    line: parseInt(locationMatch[2]),
                    column: parseInt(locationMatch[3]),
                    length: 1  // Default length
                };
            }
            
            if (line.includes('help:') && currentError) {
                const helpText = line.substring(line.indexOf('help:') + 5).trim();
                currentError.suggestions = currentError.suggestions || [];
                currentError.suggestions.push(helpText);
            }
        }
        
        if (currentError && currentError.file) {
            errors.push(currentError as SeenError);
        }
        
        return errors;
    }
    
    private setDiagnostics(uri: vscode.Uri, errors: SeenError[]): void {
        const diagnostics: vscode.Diagnostic[] = [];
        
        for (const error of errors) {
            if (error.file !== uri.fsPath && !error.file.endsWith(path.basename(uri.fsPath))) {
                continue;
            }
            
            const range = new vscode.Range(
                error.location.line - 1,
                error.location.column - 1,
                error.location.line - 1,
                error.location.column - 1 + error.location.length
            );
            
            const severity = error.severity === 'error' 
                ? vscode.DiagnosticSeverity.Error
                : error.severity === 'warning'
                ? vscode.DiagnosticSeverity.Warning
                : vscode.DiagnosticSeverity.Information;
            
            const diagnostic = new vscode.Diagnostic(
                range,
                error.message,
                severity
            );
            
            diagnostic.code = error.code;
            diagnostic.source = 'seen';
            
            // Add related information for suggestions
            if (error.suggestions.length > 0) {
                diagnostic.relatedInformation = error.suggestions.map(suggestion => {
                    return new vscode.DiagnosticRelatedInformation(
                        new vscode.Location(uri, range),
                        suggestion
                    );
                });
            }
            
            diagnostics.push(diagnostic);
        }
        
        this.diagnosticCollection.set(uri, diagnostics);
    }
    
    private updateInlineDecorations(document: vscode.TextDocument, errors: SeenError[]): void {
        const editor = vscode.window.activeTextEditor;
        if (!editor || editor.document !== document) {
            return;
        }
        
        const errorDecorations: vscode.DecorationOptions[] = [];
        const warningDecorations: vscode.DecorationOptions[] = [];
        
        for (const error of errors) {
            if (error.file !== document.uri.fsPath && !error.file.endsWith(path.basename(document.uri.fsPath))) {
                continue;
            }
            
            const range = new vscode.Range(
                error.location.line - 1,
                error.location.column - 1,
                error.location.line - 1,
                error.location.column - 1 + error.location.length
            );
            
            // Create inline hint text
            let hintText = '';
            if (error.message.includes('expected') && error.message.includes('found')) {
                // For type mismatches, show compact inline hint
                const match = error.message.match(/expected\s+(\w+),\s+found\s+(\w+)/);
                if (match) {
                    hintText = `${match[2]} != ${match[1]}`;
                }
            } else if (error.message.includes('Undefined symbol')) {
                hintText = 'not found';
            }
            
            if (hintText) {
                const decoration: vscode.DecorationOptions = {
                    range,
                    renderOptions: {
                        after: {
                            contentText: ` // ${hintText}`
                        }
                    }
                };
                
                if (error.severity === 'error') {
                    errorDecorations.push(decoration);
                } else {
                    warningDecorations.push(decoration);
                }
            }
        }
        
        editor.setDecorations(this.errorDecorationType, errorDecorations);
        editor.setDecorations(this.warningDecorationType, warningDecorations);
    }
    
    public clearDiagnostics(uri: vscode.Uri): void {
        this.diagnosticCollection.delete(uri);
    }
    
    public dispose(): void {
        this.diagnosticCollection.dispose();
        this.errorDecorationType.dispose();
        this.warningDecorationType.dispose();
    }
}

// Quick fix provider for Seen errors
export class SeenQuickFixProvider implements vscode.CodeActionProvider {
    public provideCodeActions(
        document: vscode.TextDocument,
        range: vscode.Range,
        context: vscode.CodeActionContext
    ): vscode.CodeAction[] {
        const actions: vscode.CodeAction[] = [];
        
        for (const diagnostic of context.diagnostics) {
            if (diagnostic.source !== 'seen') {
                continue;
            }
            
            // Type mismatch quick fixes
            if (diagnostic.message.includes('Type mismatch')) {
                const match = diagnostic.message.match(/expected\s+(\w+),\s+found\s+(\w+)/);
                if (match) {
                    const [, expected, found] = match;
                    
                    if (expected === 'Int' && found === 'String') {
                        const fix = new vscode.CodeAction(
                            'Parse string to Int',
                            vscode.CodeActionKind.QuickFix
                        );
                        fix.edit = new vscode.WorkspaceEdit();
                        const text = document.getText(diagnostic.range);
                        fix.edit.replace(document.uri, diagnostic.range, `${text}.parse::<Int>()`);
                        actions.push(fix);
                    } else if (expected === 'String' && found === 'Int') {
                        const fix = new vscode.CodeAction(
                            'Convert to string',
                            vscode.CodeActionKind.QuickFix
                        );
                        fix.edit = new vscode.WorkspaceEdit();
                        const text = document.getText(diagnostic.range);
                        fix.edit.replace(document.uri, diagnostic.range, `${text}.to_string()`);
                        actions.push(fix);
                    }
                }
            }
            
            // Undefined symbol quick fixes
            if (diagnostic.message.includes('Undefined symbol')) {
                // Extract suggestions from related information
                if (diagnostic.relatedInformation) {
                    for (const info of diagnostic.relatedInformation) {
                        if (info.message.startsWith('Did you mean:')) {
                            const suggestion = info.message.substring('Did you mean:'.length).trim();
                            const fix = new vscode.CodeAction(
                                `Change to '${suggestion}'`,
                                vscode.CodeActionKind.QuickFix
                            );
                            fix.edit = new vscode.WorkspaceEdit();
                            fix.edit.replace(document.uri, diagnostic.range, suggestion);
                            actions.push(fix);
                        }
                    }
                }
                
                // Import quick fix
                const symbolName = document.getText(diagnostic.range);
                const importFix = new vscode.CodeAction(
                    `Import '${symbolName}'`,
                    vscode.CodeActionKind.QuickFix
                );
                importFix.command = {
                    title: 'Import symbol',
                    command: 'seen.importSymbol',
                    arguments: [document.uri, symbolName]
                };
                actions.push(importFix);
            }
            
            // Memory alignment quick fixes
            if (diagnostic.message.includes('Memory alignment')) {
                const reorderFix = new vscode.CodeAction(
                    'Reorder struct fields by alignment',
                    vscode.CodeActionKind.QuickFix
                );
                reorderFix.command = {
                    title: 'Reorder fields',
                    command: 'seen.reorderStructFields',
                    arguments: [document.uri, diagnostic.range]
                };
                actions.push(reorderFix);
            }
        }
        
        return actions;
    }
}

// Error lens feature - show errors inline with code
export class SeenErrorLens implements vscode.CodeLensProvider {
    public provideCodeLenses(
        document: vscode.TextDocument,
        token: vscode.CancellationToken
    ): vscode.CodeLens[] {
        const lenses: vscode.CodeLens[] = [];
        const diagnostics = vscode.languages.getDiagnostics(document.uri);
        
        // Group diagnostics by line
        const diagnosticsByLine = new Map<number, vscode.Diagnostic[]>();
        for (const diagnostic of diagnostics) {
            const line = diagnostic.range.start.line;
            if (!diagnosticsByLine.has(line)) {
                diagnosticsByLine.set(line, []);
            }
            diagnosticsByLine.get(line)!.push(diagnostic);
        }
        
        // Create code lenses for lines with multiple errors
        for (const [line, lineDiagnostics] of diagnosticsByLine) {
            if (lineDiagnostics.length > 1) {
                const range = new vscode.Range(line, 0, line, 0);
                const lens = new vscode.CodeLens(range, {
                    title: `${lineDiagnostics.length} issues on this line`,
                    command: 'seen.showLineErrors',
                    arguments: [document.uri, line]
                });
                lenses.push(lens);
            }
        }
        
        return lenses;
    }
}