// src/commands.ts
import * as vscode from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';
import * as cp from 'child_process';
import * as path from 'path';

export function setupCommands(context: vscode.ExtensionContext, client: LanguageClient) {
    // Build command
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.build', async () => {
            const seenPath = vscode.workspace.getConfiguration('seen').get<string>('compiler.path', 'seen');
            const target = vscode.workspace.getConfiguration('seen').get<string>('target.default', 'native');
            const args = ['build', '--release'];
            if (target !== 'native') {
                args.push('--target', target);
            }

            const task = new vscode.Task(
                { type: 'seen', task: 'build' },
                vscode.TaskScope.Workspace,
                'Seen Build',
                'seen',
                new vscode.ShellExecution(seenPath, args),
                '$seen'
            );
            task.presentationOptions = { reveal: vscode.TaskRevealKind.Always, clear: true };
            await vscode.tasks.executeTask(task);
        })
    );

    // Run command
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.run', async () => {
            const editor = vscode.window.activeTextEditor;
            const filePath = editor?.document.languageId === 'seen'
                ? editor.document.fileName
                : '';
            const seenPath = vscode.workspace.getConfiguration('seen').get<string>('compiler.path', 'seen');
            const args = filePath ? ['run', filePath] : ['run'];

            const task = new vscode.Task(
                { type: 'seen', task: 'run' },
                vscode.TaskScope.Workspace,
                'Seen Run',
                'seen',
                new vscode.ShellExecution(seenPath, args),
                '$seen'
            );
            task.presentationOptions = { reveal: vscode.TaskRevealKind.Always, clear: true };
            await vscode.tasks.executeTask(task);
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
                            const range = client.protocol2CodeConverter.asRange(edit.range);
                            if (range) {
                                editBuilder.replace(range, edit.newText);
                            }
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
            const editor = vscode.window.activeTextEditor;
            const filePath = editor?.document.languageId === 'seen'
                ? editor.document.fileName
                : '';
            const seenPath = vscode.workspace.getConfiguration('seen').get<string>('compiler.path', 'seen');
            const args = filePath ? ['check', filePath] : ['check'];

            const task = new vscode.Task(
                { type: 'seen', task: 'check' },
                vscode.TaskScope.Workspace,
                'Seen Check',
                'seen',
                new vscode.ShellExecution(seenPath, args),
                '$seen'
            );
            task.presentationOptions = { reveal: vscode.TaskRevealKind.Always, clear: true };
            await vscode.tasks.executeTask(task);
        })
    );

    // Compile current file as PIC objects for downstream shared-library linking
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.compileSharedModule', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor || editor.document.languageId !== 'seen') {
                vscode.window.showWarningMessage('Open a Seen file to compile shared module objects.');
                return;
            }

            if (editor.document.isDirty) {
                await editor.document.save();
            }

            const seenPath = vscode.workspace.getConfiguration('seen').get<string>('compiler.path', 'seen');
            const compileConfig = vscode.workspace.getConfiguration('seen.compile');
            const filePath = editor.document.fileName;
            const parsed = path.parse(filePath);
            const outputBase = path.join(parsed.dir, parsed.name);
            const configuredManifest = compileConfig.get<string>('objectManifest', '');
            const manifestPath = configuredManifest && configuredManifest.length > 0
                ? configuredManifest
                : outputBase + '.objects.tsv';

            const args = [
                'compile',
                filePath,
                outputBase,
                '--pic',
                '--object-manifest',
                manifestPath,
                '--no-cache',
                '--no-fork'
            ];

            const task = new vscode.Task(
                { type: 'seen', task: 'compile-shared' },
                vscode.TaskScope.Workspace,
                'Seen Shared Module Objects',
                'seen',
                new vscode.ShellExecution(seenPath, args),
                '$seen'
            );
            task.presentationOptions = { reveal: vscode.TaskRevealKind.Always, clear: true };
            await vscode.tasks.executeTask(task);
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
                    { label: 'Arabic (ar)', value: 'ar' },
                    { label: 'Spanish (es)', value: 'es' },
                    { label: 'Russian (ru)', value: 'ru' },
                    { label: 'Chinese (zh)', value: 'zh' },
                    { label: 'French (fr)', value: 'fr' }
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
                        { label: 'English (en)', value: 'en' },
                        { label: 'Arabic (ar)', value: 'ar' },
                        { label: 'Spanish (es)', value: 'es' },
                        { label: 'Russian (ru)', value: 'ru' },
                        { label: 'Chinese (zh)', value: 'zh' },
                        { label: 'French (fr)', value: 'fr' }
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
                    { label: 'English (en)', value: 'en' },
                    { label: 'Arabic (ar)', value: 'ar' },
                    { label: 'Spanish (es)', value: 'es' },
                    { label: 'Russian (ru)', value: 'ru' },
                    { label: 'Chinese (zh)', value: 'zh' },
                    { label: 'French (fr)', value: 'fr' }
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

    // Import symbol - search workspace for matching exports and add import
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.importSymbol', async (uri?: vscode.Uri, symbolName?: string) => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                return;
            }

            // If no symbol name provided, get word under cursor
            if (!symbolName) {
                const position = editor.selection.active;
                const range = editor.document.getWordRangeAtPosition(position);
                if (range) {
                    symbolName = editor.document.getText(range);
                }
            }

            if (!symbolName) {
                return;
            }

            // Search workspace for files containing this symbol as a declaration
            const files = await vscode.workspace.findFiles('**/*.seen', '**/node_modules/**');
            const candidates: { label: string; modulePath: string }[] = [];

            for (const file of files) {
                if (file.toString() === editor.document.uri.toString()) {
                    continue;
                }
                const doc = await vscode.workspace.openTextDocument(file);
                const text = doc.getText();
                // Look for function, struct, class, enum declarations
                const declPattern = new RegExp(`\\b(fun|struct|class|enum|interface)\\s+${symbolName}\\b`);
                if (declPattern.test(text)) {
                    const relativePath = vscode.workspace.asRelativePath(file);
                    const modulePath = relativePath.replace(/\.seen$/, '').replace(/\//g, '.');
                    candidates.push({
                        label: `import ${symbolName} from ${modulePath}`,
                        modulePath
                    });
                }
            }

            if (candidates.length === 0) {
                vscode.window.showInformationMessage(`No declarations found for '${symbolName}'`);
                return;
            }

            const selected = candidates.length === 1
                ? candidates[0]
                : await vscode.window.showQuickPick(candidates, {
                    placeHolder: `Import '${symbolName}' from...`
                });

            if (selected) {
                const importLine = `import ${selected.modulePath} { ${symbolName} }\n`;
                await editor.edit(editBuilder => {
                    // Insert at the top of the file (after any existing imports)
                    const text = editor.document.getText();
                    const lines = text.split('\n');
                    let insertLine = 0;
                    for (let i = 0; i < lines.length; i++) {
                        if (lines[i].startsWith('import ')) {
                            insertLine = i + 1;
                        }
                    }
                    editBuilder.insert(new vscode.Position(insertLine, 0), importLine);
                });
            }
        })
    );

    // Reorder struct fields by alignment
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.reorderStructFields', async (uri?: vscode.Uri, range?: vscode.Range) => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                return;
            }

            // Find the struct definition at or near the cursor
            const document = editor.document;
            const text = document.getText();
            const cursorOffset = document.offsetAt(range?.start ?? editor.selection.active);

            // Simple struct field extraction
            const structPattern = /struct\s+\w+[^{]*\{([^}]*)\}/g;
            let match;
            while ((match = structPattern.exec(text)) !== null) {
                const matchStart = match.index;
                const matchEnd = matchStart + match[0].length;
                if (cursorOffset >= matchStart && cursorOffset <= matchEnd) {
                    const fieldsStr = match[1];
                    const fieldLines = fieldsStr.split('\n')
                        .map(l => l.trim())
                        .filter(l => l.length > 0 && l.includes(':'));

                    // Parse fields and sort by type size (heuristic)
                    const typeSizes: Record<string, number> = {
                        'Bool': 1, 'Char': 1,
                        'i8': 1, 'u8': 1,
                        'i16': 2, 'u16': 2,
                        'i32': 4, 'u32': 4, 'Int': 4, 'UInt': 4, 'Float': 4,
                        'i64': 8, 'u64': 8, 'f64': 8,
                        'String': 8, 'i128': 16, 'u128': 16
                    };

                    const parsedFields = fieldLines.map(line => {
                        const colonIdx = line.indexOf(':');
                        const fieldName = line.substring(0, colonIdx).trim();
                        const fieldType = line.substring(colonIdx + 1).trim();
                        const size = typeSizes[fieldType] ?? 8; // default pointer size
                        return { line, fieldName, fieldType, size };
                    });

                    // Sort by descending size (largest alignment first)
                    parsedFields.sort((a, b) => b.size - a.size);

                    const sorted = parsedFields.map(f => `    ${f.line}`).join('\n');
                    const bodyStart = text.indexOf('{', matchStart) + 1;
                    const bodyEnd = text.indexOf('}', bodyStart);

                    const startPos = document.positionAt(bodyStart);
                    const endPos = document.positionAt(bodyEnd);
                    const replaceRange = new vscode.Range(startPos, endPos);

                    await editor.edit(editBuilder => {
                        editBuilder.replace(replaceRange, '\n' + sorted + '\n');
                    });

                    vscode.window.showInformationMessage('Struct fields reordered by alignment');
                    return;
                }
            }

            vscode.window.showInformationMessage('No struct definition found at cursor position');
        })
    );

    // Show line errors - show quick pick with all diagnostics on a line
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.showLineErrors', async (uri?: vscode.Uri, line?: number) => {
            if (!uri || line === undefined) {
                const editor = vscode.window.activeTextEditor;
                if (!editor) {
                    return;
                }
                uri = editor.document.uri;
                line = editor.selection.active.line;
            }

            const diagnostics = vscode.languages.getDiagnostics(uri);
            const lineDiagnostics = diagnostics.filter(d => d.range.start.line === line);

            if (lineDiagnostics.length === 0) {
                vscode.window.showInformationMessage('No diagnostics on this line');
                return;
            }

            const items = lineDiagnostics.map(d => ({
                label: d.severity === vscode.DiagnosticSeverity.Error ? '$(error) ' + d.message : '$(warning) ' + d.message,
                description: d.source ?? '',
                detail: d.relatedInformation?.map(r => r.message).join('; '),
                diagnostic: d
            }));

            const selected = await vscode.window.showQuickPick(items, {
                placeHolder: `${lineDiagnostics.length} issue(s) on line ${(line ?? 0) + 1}`
            });

            if (selected) {
                // Navigate to the diagnostic location
                const editor = vscode.window.activeTextEditor;
                if (editor) {
                    const range = selected.diagnostic.range;
                    editor.selection = new vscode.Selection(range.start, range.end);
                    editor.revealRange(range, vscode.TextEditorRevealType.InCenter);
                }
            }
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
            output.appendLine(`  Time: ${bench.mean}ms (\u00b1${bench.stddev}ms)`);
            output.appendLine(`  Iterations: ${bench.iterations}`);
            if (bench.throughput) {
                output.appendLine(`  Throughput: ${bench.throughput} ops/sec`);
            }
        }
    }

    output.show();
}
