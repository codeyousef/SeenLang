// src/commands.ts
import * as vscode from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';
import * as cp from 'child_process';
import * as path from 'path';
import { ReactiveVisualizer } from './reactive';

const SEEN_LANGUAGES = [
    { label: 'English (en)', value: 'en' },
    { label: 'Arabic (ar)', value: 'ar' },
    { label: 'Spanish (es)', value: 'es' },
    { label: 'Russian (ru)', value: 'ru' },
    { label: 'Chinese (zh)', value: 'zh' },
    { label: 'Japanese (ja)', value: 'ja' }
];

function getSeenPath(): string {
    return vscode.workspace.getConfiguration('seen').get<string>('compiler.path', 'seen');
}

function createSeenTask(name: string, task: string, args: string[]): vscode.Task {
    const seenPath = getSeenPath();
    const seenTask = new vscode.Task(
        { type: 'seen', task },
        vscode.TaskScope.Workspace,
        name,
        'seen',
        new vscode.ShellExecution(seenPath, args),
        '$seen'
    );
    seenTask.presentationOptions = { reveal: vscode.TaskRevealKind.Always, clear: true };
    return seenTask;
}

async function executeSeenTask(name: string, task: string, args: string[]): Promise<void> {
    await vscode.tasks.executeTask(createSeenTask(name, task, args));
}

function workspaceRoot(): string | undefined {
    return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
}

async function promptProjectRoot(prompt: string): Promise<string | undefined> {
    const value = await vscode.window.showInputBox({
        prompt,
        value: workspaceRoot() ?? '',
        placeHolder: 'Path to a Seen project directory or Seen.toml'
    });
    if (value === undefined) {
        return undefined;
    }
    return value.trim();
}

async function findWorkspaceSeenToml(): Promise<vscode.Uri | undefined> {
    const root = workspaceRoot();
    if (root) {
        const rootManifest = vscode.Uri.file(path.join(root, 'Seen.toml'));
        try {
            await vscode.workspace.fs.stat(rootManifest);
            return rootManifest;
        } catch {
            // Fall through to workspace search.
        }
    }
    const matches = await vscode.workspace.findFiles('**/Seen.toml', '**/node_modules/**', 1);
    return matches[0];
}

function setTomlProjectLanguage(content: string, language: string): string {
    const lines = content.split(/\r?\n/);
    let inProject = false;
    let projectStart = -1;

    for (let i = 0; i < lines.length; i++) {
        const trimmed = lines[i].trim();
        const section = trimmed.match(/^\[([^\]]+)\]$/);
        if (section) {
            if (inProject) {
                lines.splice(i, 0, `language = "${language}"`);
                return lines.join('\n');
            }
            inProject = section[1] === 'project';
            if (inProject) {
                projectStart = i;
            }
            continue;
        }

        if (inProject && trimmed.match(/^language\s*=/)) {
            lines[i] = `language = "${language}"`;
            return lines.join('\n');
        }
    }

    if (inProject && projectStart >= 0) {
        lines.push(`language = "${language}"`);
        return lines.join('\n');
    }

    const suffix = content.endsWith('\n') || content.length === 0 ? '' : '\n';
    return `${content}${suffix}[project]\nlanguage = "${language}"\n`;
}

async function updateWorkspaceLanguage(language: string): Promise<boolean> {
    await vscode.workspace.getConfiguration('seen').update(
        'language.default',
        language,
        vscode.ConfigurationTarget.Workspace
    );

    const manifest = await findWorkspaceSeenToml();
    if (!manifest) {
        return false;
    }

    const bytes = await vscode.workspace.fs.readFile(manifest);
    const content = Buffer.from(bytes).toString('utf8');
    const updated = setTomlProjectLanguage(content, language);
    if (updated !== content) {
        await vscode.workspace.fs.writeFile(manifest, Buffer.from(updated, 'utf8'));
    }
    return true;
}

function getConfiguredLanguage(): string {
    return vscode.workspace.getConfiguration('seen').get<string>('language.default', 'en');
}

function runSeenCapture(args: string[], cwd?: string): Promise<string> {
    return new Promise((resolve, reject) => {
        const child = cp.spawn(getSeenPath(), args, { cwd });
        let stdout = '';
        let stderr = '';
        child.stdout.on('data', data => { stdout += data.toString(); });
        child.stderr.on('data', data => { stderr += data.toString(); });
        child.on('error', reject);
        child.on('close', code => {
            if (code === 0) {
                resolve(stdout);
            } else {
                reject(new Error((stderr || stdout || `seen exited with ${code}`).trim()));
            }
        });
    });
}

function currentReactiveStreamInfo(document: vscode.TextDocument, position: vscode.Position): any | undefined {
    const line = document.lineAt(position.line).text.trim();
    const reactiveTokens = ['Observable', 'Flow', 'Subject', 'BehaviorSubject', 'emit', '.map(', '.filter(', '.flatMap(', '.merge(', '.zip('];
    if (!reactiveTokens.some(token => line.includes(token))) {
        return undefined;
    }
    const operator = reactiveTokens.find(token => line.includes(token)) ?? 'stream';
    return {
        operator,
        description: line,
        sourceEvents: [
            { time: 0, value: 'a', type: 'value' },
            { time: 1, value: 'b', type: 'value' },
            { time: 2, value: '', type: 'complete' }
        ],
        outputEvents: [
            { time: 0.2, value: 'a', type: 'value' },
            { time: 1.2, value: 'b', type: 'value' },
            { time: 2.2, value: '', type: 'complete' }
        ]
    };
}

export function setupCommands(context: vscode.ExtensionContext, client: LanguageClient) {
    // Build command
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.build', async () => {
            const target = vscode.workspace.getConfiguration('seen').get<string>('target.default', 'native');
            const args = ['build', '--release'];
            if (target !== 'native') {
                args.push('--target', target);
            }

            await executeSeenTask('Seen Build', 'build', args);
        })
    );

    // Run command
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.run', async () => {
            const editor = vscode.window.activeTextEditor;
            const filePath = editor?.document.languageId === 'seen'
                ? editor.document.fileName
                : '';
            const args = filePath ? ['run', filePath] : ['run'];

            await executeSeenTask('Seen Run', 'run', args);
        })
    );

    // Test command
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.test', async () => {
            await executeSeenTask('Seen Test', 'test', ['test']);
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
                await executeSeenTask('Seen Benchmark', 'benchmark', ['benchmark', '--json']);
            }
        })
    );

    // Individual benchmark run (from CodeLens)
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.benchmark.run', async (uri: vscode.Uri, benchmarkName: string) => {
            await executeSeenTask(`Benchmark: ${benchmarkName}`, 'benchmark', ['benchmark', '--filter', benchmarkName]);
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
            const args = filePath ? ['check', filePath] : ['check'];

            await executeSeenTask('Seen Check', 'check', args);
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

            await executeSeenTask('Seen Shared Module Objects', 'compile-shared', args);
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('seen.pkgFetch', async () => {
            const root = await promptProjectRoot('Fetch dependencies for which Seen project?');
            if (root === undefined) {
                return;
            }
            const args = root.length > 0 ? ['pkg', 'fetch', root] : ['pkg', 'fetch'];
            await executeSeenTask('Seen Package Fetch', 'pkg-fetch', args);
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('seen.pkgPack', async () => {
            const root = await promptProjectRoot('Pack which Seen project?');
            if (root === undefined) {
                return;
            }
            const output = await vscode.window.showInputBox({
                prompt: 'Optional output archive path',
                placeHolder: 'Leave empty to use the compiler default'
            });
            if (output === undefined) {
                return;
            }
            const args = ['pkg', 'pack'];
            if (root.length > 0) {
                args.push(root);
            }
            if (output.trim().length > 0) {
                args.push(output.trim());
            }
            await executeSeenTask('Seen Package Pack', 'pkg-pack', args);
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('seen.pkgPrebuild', async () => {
            const root = await promptProjectRoot('Prebuild which Seen project?');
            if (root === undefined) {
                return;
            }
            const output = await vscode.window.showInputBox({
                prompt: 'Optional output artifact directory',
                placeHolder: 'Leave empty to use the compiler default'
            });
            if (output === undefined) {
                return;
            }
            const args = ['pkg', 'prebuild'];
            if (root.length > 0) {
                args.push(root);
            }
            if (output.trim().length > 0) {
                args.push(output.trim());
            }
            await executeSeenTask('Seen Package Prebuild', 'pkg-prebuild', args);
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('seen.pkgPublish', async () => {
            const registry = await vscode.window.showOpenDialog({
                canSelectFiles: false,
                canSelectFolders: true,
                canSelectMany: false,
                openLabel: 'Select registry directory'
            });
            if (!registry || registry.length === 0) {
                return;
            }
            const root = await promptProjectRoot('Publish which Seen project?');
            if (root === undefined) {
                return;
            }
            const args = ['pkg', 'publish', registry[0].fsPath];
            if (root.length > 0) {
                args.push(root);
            }
            await executeSeenTask('Seen Package Publish', 'pkg-publish', args);
        })
    );

    // Clean command
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.clean', async () => {
            await executeSeenTask('Seen Clean', 'clean', ['clean']);
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
                SEEN_LANGUAGES,
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

            await executeSeenTask('Seen Init', 'init', ['init', projectName, '--lang', langCode, '--type', typeValue]);

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
                    SEEN_LANGUAGES,
                    { placeHolder: 'Select target language' }
                );
                targetLang = language?.value;
            }

            if (!targetLang) {
                return;
            }

            try {
                const updatedManifest = await updateWorkspaceLanguage(targetLang);
                const manifestNote = updatedManifest ? ' and Seen.toml' : '';
                const action = await vscode.window.showInformationMessage(
                    `Switched Seen language default${manifestNote} to ${targetLang}.`,
                    'Reload Window'
                );
                if (action === 'Reload Window') {
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
                SEEN_LANGUAGES,
                { placeHolder: 'Translate to' }
            );

            if (!targetLanguage) {
                return;
            }

            try {
                if (editor.document.isDirty) {
                    await editor.document.save();
                }
                const translated = await runSeenCapture(
                    ['translate', editor.document.fileName, '--from', getConfiguredLanguage(), '--to', targetLanguage.value],
                    workspaceRoot()
                );

                // Open translated code in new editor
                const doc = await vscode.workspace.openTextDocument({
                    content: translated,
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
            const streamInfo = currentReactiveStreamInfo(editor.document, position);
            if (streamInfo) {
                ReactiveVisualizer.show(streamInfo);
            } else {
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
                const declPattern = new RegExp(`\\b(fun|struct|class|enum|interface|trait|type|extern\\s+fun)\\s+${symbolName}\\b`);
                if (declPattern.test(text)) {
                    const relativePath = vscode.workspace.asRelativePath(file);
                    const modulePath = relativePath.replace(/\.seen$/, '').replace(/\\/g, '/').replace(/\//g, '.');
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
                const importLine = `import ${selected.modulePath}.{${symbolName}}\n`;
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
        const args = filter ? ['benchmark', '--json', '--filter', filter] : ['benchmark', '--json'];
        const child = cp.spawn(getSeenPath(), args, { cwd: workspaceRoot() });
        let stdout = '';
        let stderr = '';
        child.stdout.on('data', data => { stdout += data.toString(); });
        child.stderr.on('data', data => { stderr += data.toString(); });
        child.on('error', reject);
        child.on('close', code => {
            if (code !== 0) {
                reject(new Error(stderr || stdout || `seen benchmark exited with ${code}`));
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
