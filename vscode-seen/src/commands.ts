// src/commands.ts
import * as vscode from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';
import * as cp from 'child_process';
import * as path from 'path';

export function setupCommands(context: vscode.ExtensionContext, client: LanguageClient) {
    // Build command
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.build', async () => {
            const terminal = vscode.window.createTerminal('Seen Build');
            const target = vscode.workspace.getConfiguration('seen').get<string>('target.default', 'native');
            if (target === 'native') {
                terminal.sendText('seen build --release');
            } else {
                terminal.sendText(`seen build --release --target ${target}`);
            }
            terminal.show();
        })
    );

    // Run command
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.run', async () => {
            const terminal = vscode.window.createTerminal('Seen Run');
            terminal.sendText('seen run');
            terminal.show();
        })
    );

    // Test command
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.test', async () => {
            const terminal = vscode.window.createTerminal('Seen Test');
            terminal.sendText('seen test');
            terminal.show();
        })
    );

    // Benchmark command
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.benchmark', async () => {
            const config = vscode.workspace.getConfiguration('seen');
            const showInline = config.get<boolean>('benchmark.showInline', true);
            
            if (showInline) {
                // Run benchmarks and show results inline
                const results = await runBenchmarks();
                showBenchmarkResults(results);
            } else {
                // Run in terminal
                const terminal = vscode.window.createTerminal('Seen Benchmark');
                terminal.sendText('seen benchmark --json');
                terminal.show();
            }
        })
    );

    // Individual benchmark run (from CodeLens)
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.benchmark.run', async (uri: vscode.Uri, benchmarkName: string) => {
            const terminal = vscode.window.createTerminal(`Benchmark: ${benchmarkName}`);
            terminal.sendText(`seen benchmark --filter ${benchmarkName}`);
            terminal.show();
        })
    );

    // Benchmark comparison (from CodeLens)
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.benchmark.compare', async (uri: vscode.Uri, benchmarkName: string) => {
            const results = await runBenchmarks(benchmarkName);
            // Show comparison view
            vscode.window.showInformationMessage(`Benchmark comparison for ${benchmarkName} completed`);
        })
    );

    // Format command
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.format', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                return;
            }
            
            const document = editor.document;
            if (document.languageId !== 'seen') {
                return;
            }

            try {
                const formatted = await client.sendRequest('textDocument/formatting', {
                    textDocument: { uri: document.uri.toString() },
                    options: {
                        tabSize: editor.options.tabSize as number,
                        insertSpaces: editor.options.insertSpaces as boolean
                    }
                });
                
                if (formatted && Array.isArray(formatted) && formatted.length > 0) {
                    await editor.edit(editBuilder => {
                        formatted.forEach((edit: any) => {
                            editBuilder.replace(
                                client.protocol2CodeConverter.asRange(edit.range),
                                edit.newText
                            );
                        });
                    });
                }
            } catch (error) {
                vscode.window.showErrorMessage('Failed to format document: ' + error);
            }
        })
    );

    // Check command
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.check', async () => {
            const terminal = vscode.window.createTerminal('Seen Check');
            terminal.sendText('seen check');
            terminal.show();
        })
    );

    // Clean command
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.clean', async () => {
            const terminal = vscode.window.createTerminal('Seen Clean');
            terminal.sendText('seen clean');
            terminal.show();
        })
    );

    // Initialize new project
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.init', async () => {
            const projectName = await vscode.window.showInputBox({
                prompt: 'Enter project name',
                placeHolder: 'my-project',
                validateInput: (input) => {
                    if (!input) {
                        return 'Project name is required';
                    }
                    if (!/^[a-zA-Z0-9_-]+$/.test(input)) {
                        return 'Project name can only contain letters, numbers, underscores, and hyphens';
                    }
                    return null;
                }
            });
            
            if (!projectName) {
                return;
            }
            
            const language = await vscode.window.showQuickPick(
                [
                    { label: 'English (en)', value: 'en' },
                    { label: 'Arabic (ar)', value: 'ar' }
                ],
                { placeHolder: 'Select project language' }
            );
            
            const langCode = language?.value || 'en';
            
            const projectType = await vscode.window.showQuickPick(
                [
                    { label: 'Application', value: 'application' },
                    { label: 'Library', value: 'library' },
                    { label: 'Reactive', value: 'reactive' },
                    { label: 'Benchmark', value: 'benchmark' }
                ],
                { placeHolder: 'Select project type' }
            );
            
            const typeValue = projectType?.value || 'application';
            
            const terminal = vscode.window.createTerminal('Seen Init');
            terminal.sendText(`seen init ${projectName} --lang ${langCode} --type ${typeValue}`);
            terminal.show();
            
            // Open the new project after creation
            setTimeout(() => {
                const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
                if (workspaceRoot) {
                    const projectPath = path.join(workspaceRoot, projectName);
                    const uri = vscode.Uri.file(projectPath);
                    vscode.commands.executeCommand('vscode.openFolder', uri);
                }
            }, 2000);
        })
    );

    // Switch project language
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.switchLanguage', async (targetLang?: string) => {
            if (!targetLang) {
                const language = await vscode.window.showQuickPick(
                    [
                        { label: 'English', value: 'en' },
                        { label: 'Arabic', value: 'ar' }
                    ],
                    { placeHolder: 'Select target language' }
                );
                targetLang = language?.value;
            }
            
            if (!targetLang) {
                return;
            }
            
            try {
                const result = await client.sendRequest('seen/switchLanguage', {
                    targetLanguage: targetLang
                });
                
                if (result && (result as any).success) {
                    vscode.window.showInformationMessage(`Switched to ${targetLang}`);
                    // Reload window to apply changes
                    vscode.commands.executeCommand('workbench.action.reloadWindow');
                }
            } catch (error) {
                vscode.window.showErrorMessage('Failed to switch language: ' + error);
            }
        })
    );

    // Translate code
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.translateCode', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                return;
            }
            
            const targetLanguage = await vscode.window.showQuickPick(
                [
                    { label: 'English', value: 'en' },
                    { label: 'Arabic', value: 'ar' }
                ],
                { placeHolder: 'Translate to' }
            );
            
            if (!targetLanguage) {
                return;
            }
            
            try {
                const translated = await client.sendRequest('seen/translate', {
                    uri: editor.document.uri.toString(),
                    targetLanguage: targetLanguage.value
                });
                
                // Open translated code in new editor
                const doc = await vscode.workspace.openTextDocument({
                    content: (translated as any).content,
                    language: 'seen'
                });
                vscode.window.showTextDocument(doc);
            } catch (error) {
                vscode.window.showErrorMessage('Failed to translate code: ' + error);
            }
        })
    );

    // Visualize reactive stream
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.reactive.visualize', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                return;
            }
            
            const position = editor.selection.active;
            try {
                const streamInfo = await client.sendRequest('seen/getStreamInfo', {
                    uri: editor.document.uri.toString(),
                    position: { line: position.line, character: position.character }
                });
                
                if (streamInfo) {
                    // Show marble diagram in webview
                    vscode.commands.executeCommand('seen.showReactiveView', streamInfo);
                }
            } catch (error) {
                vscode.window.showErrorMessage('No reactive stream found at current position');
            }
        })
    );

    // Show references
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.showReferences', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                return;
            }
            
            const position = editor.selection.active;
            vscode.commands.executeCommand('editor.action.goToReferences', 
                editor.document.uri, position);
        })
    );

    // Open REPL
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.repl', async () => {
            const terminal = vscode.window.createTerminal({
                name: 'Seen REPL',
                shellPath: vscode.workspace.getConfiguration('seen').get<string>('compiler.path', 'seen'),
                shellArgs: ['repl']
            });
            terminal.show();
        })
    );
}

async function runBenchmarks(filter?: string): Promise<any> {
    return new Promise((resolve, reject) => {
        const command = filter ? `seen benchmark --json --filter ${filter}` : 'seen benchmark --json';
        cp.exec(command, (error, stdout, stderr) => {
            if (error) {
                reject(error);
                return;
            }
            try {
                const results = JSON.parse(stdout);
                resolve(results);
            } catch (e) {
                reject(e);
            }
        });
    });
}

function showBenchmarkResults(results: any) {
    // Create output channel for benchmark results
    const output = vscode.window.createOutputChannel('Seen Benchmarks');
    output.clear();
    output.appendLine('Benchmark Results');
    output.appendLine('=================');
    
    if (results.benchmarks) {
        for (const bench of results.benchmarks) {
            output.appendLine(`\n${bench.name}:`);
            output.appendLine(`  Time: ${bench.mean}ms (±${bench.stddev}ms)`);
            output.appendLine(`  Iterations: ${bench.iterations}`);
            if (bench.throughput) {
                output.appendLine(`  Throughput: ${bench.throughput} ops/sec`);
            }
        }
    }
    
    output.show();
}