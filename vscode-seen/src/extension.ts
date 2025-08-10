// src/extension.ts
import * as vscode from 'vscode';
import * as path from 'path';
import { LanguageClient, LanguageClientOptions, ServerOptions, TransportKind } from 'vscode-languageclient/node';
import { setupCommands } from './commands';
import { SeenDebugAdapterFactory, registerDebugSupport } from './debugger';
import { SeenReplProvider, registerReplCommands } from './repl';
import { ReactiveVisualizer, ReactiveStreamViewProvider, ReactiveInlineValueProvider } from './reactive';
import { BenchmarkRunner, BenchmarkTreeDataProvider, BenchmarkCodeLensProvider, registerBenchmarkCommands } from './benchmark';
import { SeenDiagnosticProvider, SeenQuickFixProvider, SeenErrorLens } from './errorDiagnostics';

let client: LanguageClient;
let diagnosticProvider: SeenDiagnosticProvider;

export async function activate(context: vscode.ExtensionContext) {
    console.log('Seen Language extension is activating');

    // Get Seen compiler path
    const seenPath = vscode.workspace.getConfiguration('seen').get<string>('compiler.path', 'seen');
    
    // Verify Seen is installed
    try {
        await verifySeenInstallation(seenPath);
    } catch (error) {
        const installAction = 'Install Seen';
        const selected = await vscode.window.showErrorMessage(
            'Seen compiler not found. Please install it first.',
            installAction
        );
        
        if (selected === installAction) {
            vscode.env.openExternal(vscode.Uri.parse('https://seen-lang.org/install'));
        }
        return;
    }

    // Check if we're in a Seen project
    const workspaceFolders = vscode.workspace.workspaceFolders;
    if (workspaceFolders) {
        for (const folder of workspaceFolders) {
            const seenTomlPath = path.join(folder.uri.fsPath, 'Seen.toml');
            try {
                await vscode.workspace.fs.stat(vscode.Uri.file(seenTomlPath));
                vscode.commands.executeCommand('setContext', 'seenProject', true);
                break;
            } catch {
                // Not a Seen project folder
            }
        }
    }

    // Start Language Server
    const serverOptions: ServerOptions = {
        command: seenPath,
        args: ['lsp'],
        transport: TransportKind.stdio,
        options: {
            env: {
                ...process.env,
                SEEN_LSP_LOG: vscode.workspace.getConfiguration('seen.lsp.trace').get('server')
            }
        }
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'seen' }],
        synchronize: {
            fileEvents: [
                vscode.workspace.createFileSystemWatcher('**/*.seen'),
                vscode.workspace.createFileSystemWatcher('**/Seen.toml'),
                vscode.workspace.createFileSystemWatcher('**/languages/*.toml')
            ]
        },
        initializationOptions: {
            capabilities: {
                reactive: true,          // Enable reactive programming features
                multilingual: true,      // Enable multilingual support
                benchmarking: true,      // Enable benchmark support
                riscv: true             // Enable RISC-V features
            }
        }
    };

    // Create and start the language client
    client = new LanguageClient(
        'seen',
        'Seen Language Server',
        serverOptions,
        clientOptions
    );

    // Register custom protocol handlers
    client.onReady().then(() => {
        // Handle reactive stream visualizations
        client.onNotification('seen/reactiveStream', (params: any) => {
            ReactiveVisualizer.show(params);
        });

        // Handle benchmark results
        client.onNotification('seen/benchmarkResult', (params: any) => {
            BenchmarkRunner.showResults(params);
        });

        // Handle multilingual suggestions
        client.onNotification('seen/languageSuggestion', (params: any) => {
            handleLanguageSuggestion(params);
        });
    });

    // Start the client
    await client.start();

    // Initialize error diagnostics provider
    diagnosticProvider = new SeenDiagnosticProvider();
    context.subscriptions.push(diagnosticProvider);
    
    // Register quick fix provider
    const quickFixProvider = new SeenQuickFixProvider();
    context.subscriptions.push(
        vscode.languages.registerCodeActionProvider(
            { language: 'seen' },
            quickFixProvider,
            {
                providedCodeActionKinds: [vscode.CodeActionKind.QuickFix]
            }
        )
    );
    
    // Register error lens provider
    const errorLensProvider = new SeenErrorLens();
    context.subscriptions.push(
        vscode.languages.registerCodeLensProvider(
            { language: 'seen' },
            errorLensProvider
        )
    );
    
    // Update diagnostics on document changes
    context.subscriptions.push(
        vscode.workspace.onDidChangeTextDocument(event => {
            if (event.document.languageId === 'seen') {
                diagnosticProvider.updateDiagnostics(event.document);
            }
        })
    );
    
    context.subscriptions.push(
        vscode.workspace.onDidOpenTextDocument(document => {
            if (document.languageId === 'seen') {
                diagnosticProvider.updateDiagnostics(document);
            }
        })
    );
    
    context.subscriptions.push(
        vscode.workspace.onDidSaveTextDocument(document => {
            if (document.languageId === 'seen') {
                diagnosticProvider.updateDiagnostics(document);
            }
        })
    );

    // Register commands
    setupCommands(context, client);
    
    // Register additional command modules
    registerDebugSupport(context);
    registerReplCommands(context);
    registerBenchmarkCommands(context);

    // Register debug adapter
    const debugAdapterFactory = new SeenDebugAdapterFactory();
    context.subscriptions.push(
        vscode.debug.registerDebugAdapterDescriptorFactory('seen', debugAdapterFactory)
    );

    // Register REPL provider
    const replProvider = new SeenReplProvider(seenPath);
    context.subscriptions.push(
        vscode.window.registerTerminalProfileProvider('seen.repl', replProvider)
    );

    // Register custom views
    registerCustomViews(context);

    // Register code lens provider for benchmarks
    const benchmarkLensProvider = new BenchmarkCodeLensProvider();
    context.subscriptions.push(
        vscode.languages.registerCodeLensProvider(
            { language: 'seen' },
            benchmarkLensProvider
        )
    );

    // Register inline value provider for reactive streams
    const reactiveValueProvider = new ReactiveInlineValueProvider(client);
    context.subscriptions.push(
        vscode.languages.registerInlineValuesProvider(
            { language: 'seen' },
            reactiveValueProvider
        )
    );

    console.log('Seen Language extension activated successfully');
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}

async function verifySeenInstallation(seenPath: string): Promise<void> {
    const cp = require('child_process');
    return new Promise((resolve, reject) => {
        cp.exec(`${seenPath} --version`, (error: any, stdout: string) => {
            if (error) {
                reject(error);
            } else {
                console.log(`Seen compiler found: ${stdout.trim()}`);
                resolve();
            }
        });
    });
}

function registerCustomViews(context: vscode.ExtensionContext) {
    // Register reactive stream visualizer webview
    const reactiveProvider = new ReactiveStreamViewProvider(context.extensionUri);
    context.subscriptions.push(
        vscode.window.registerWebviewViewProvider(
            'seen.reactiveStreams',
            reactiveProvider
        )
    );

    // Register benchmark results tree view
    const benchmarkProvider = new BenchmarkTreeDataProvider();
    context.subscriptions.push(
        vscode.window.createTreeView('seen.benchmarks', {
            treeDataProvider: benchmarkProvider,
            showCollapseAll: true
        })
    );
}

async function handleLanguageSuggestion(params: any) {
    const { currentLang, suggestedLang, reason } = params;
    
    const message = `Switch from ${currentLang} to ${suggestedLang}? ${reason}`;
    const action = await vscode.window.showInformationMessage(
        message,
        'Switch',
        'Keep Current'
    );
    
    if (action === 'Switch') {
        await vscode.commands.executeCommand('seen.switchLanguage', suggestedLang);
    }
}