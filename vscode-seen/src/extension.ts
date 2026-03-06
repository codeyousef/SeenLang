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
import { BinaryManager } from './binaryManager';

let client: LanguageClient | undefined;
let diagnosticProvider: SeenDiagnosticProvider | undefined;
let lspActive = false;
let binaryManager: BinaryManager | undefined;

export async function activate(context: vscode.ExtensionContext) {
    console.log('Seen Language extension is activating');

    // Initialize binary manager for auto-download support
    binaryManager = new BinaryManager(context);

    // Get Seen compiler path - check user config first
    let seenPath = vscode.workspace.getConfiguration('seen').get<string>('compiler.path', 'seen');

    // If no custom path is set (default "seen"), try to use auto-managed binary
    if (seenPath === 'seen') {
        try {
            // First check if 'seen' is available in PATH
            const pathResult = await checkSeenInPath();
            if (!pathResult) {
                // Not in PATH, use auto-managed binary
                console.log('Seen not found in PATH, using auto-managed binary');
                seenPath = await binaryManager.ensureBinary();
            }
        } catch (error) {
            console.warn('Failed to ensure Seen binary:', error);
            // Continue with default 'seen' - will show warning later if not found
        }
    }

    // Register update command
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.updateCompiler', async () => {
            if (!binaryManager) {
                vscode.window.showErrorMessage('Binary manager not initialized');
                return;
            }
            try {
                await binaryManager.forceUpdate();
                const action = await vscode.window.showInformationMessage(
                    'Seen compiler updated. Reload window to use the new version.',
                    'Reload Window'
                );
                if (action === 'Reload Window') {
                    vscode.commands.executeCommand('workbench.action.reloadWindow');
                }
            } catch (error) {
                vscode.window.showErrorMessage(`Failed to update Seen compiler: ${error}`);
            }
        })
    );

    // Register check for updates command
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.checkForUpdates', async () => {
            if (!binaryManager) {
                vscode.window.showErrorMessage('Binary manager not initialized');
                return;
            }
            try {
                const hasUpdate = await binaryManager.checkForUpdate();
                if (hasUpdate) {
                    const action = await vscode.window.showInformationMessage(
                        'A new version of the Seen compiler is available.',
                        'Update Now'
                    );
                    if (action === 'Update Now') {
                        vscode.commands.executeCommand('seen.updateCompiler');
                    }
                } else {
                    vscode.window.showInformationMessage('Seen compiler is up to date.');
                }
            } catch (error) {
                vscode.window.showErrorMessage(`Failed to check for updates: ${error}`);
            }
        })
    );

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

    // Register quick fix provider (works in both LSP and standalone mode)
    const quickFixProvider = new SeenQuickFixProvider();
    context.subscriptions.push(
        vscode.languages.registerCodeActionsProvider(
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

    // Try to start the Language Server (don't block extension activation)
    try {
        await startLanguageServer(context, seenPath);
        lspActive = true;
    } catch (error) {
        console.warn('Seen LSP not available:', error);
        vscode.window.showWarningMessage(
            'Seen compiler not found. Syntax highlighting is active, but LSP features require the Seen compiler. ' +
            'Set "seen.compiler.path" in settings or install the compiler.'
        );
    }

    // Only use standalone diagnostic provider when LSP is not active
    // (LSP provides its own diagnostics; running both duplicates errors)
    if (!lspActive) {
        diagnosticProvider = new SeenDiagnosticProvider();
        context.subscriptions.push(diagnosticProvider);

        context.subscriptions.push(
            vscode.workspace.onDidChangeTextDocument(event => {
                if (event.document.languageId === 'seen' && diagnosticProvider) {
                    diagnosticProvider.updateDiagnostics(event.document);
                }
            })
        );

        context.subscriptions.push(
            vscode.workspace.onDidOpenTextDocument(document => {
                if (document.languageId === 'seen' && diagnosticProvider) {
                    diagnosticProvider.updateDiagnostics(document);
                }
            })
        );

        context.subscriptions.push(
            vscode.workspace.onDidSaveTextDocument(document => {
                if (document.languageId === 'seen' && diagnosticProvider) {
                    diagnosticProvider.updateDiagnostics(document);
                }
            })
        );
    }

    // Register commands (works with or without LSP)
    setupCommands(context, client as LanguageClient);

    // Register additional command modules
    registerDebugSupport(context);
    registerReplCommands(context);
    registerBenchmarkCommands(context);

    console.log('Seen Language extension activated successfully');
}

async function startLanguageServer(context: vscode.ExtensionContext, seenPath: string): Promise<void> {
    // Verify Seen is installed
    await verifySeenInstallation(seenPath);

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
                multilingual: true
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

    // Start the client and register notification handlers after it's ready
    await client.start();

    // Register custom protocol handlers (client is now started)
    client.onNotification('seen/reactiveStream', (params: any) => {
        ReactiveVisualizer.show(params);
    });

    client.onNotification('seen/benchmarkResult', (params: any) => {
        BenchmarkRunner.showResults(params);
    });

    client.onNotification('seen/languageSuggestion', (params: any) => {
        handleLanguageSuggestion(params);
    });

    // Register inline value provider for reactive streams (needs client)
    const reactiveValueProvider = new ReactiveInlineValueProvider(client);
    context.subscriptions.push(
        vscode.languages.registerInlineValuesProvider(
            { language: 'seen' },
            reactiveValueProvider
        )
    );
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
        cp.exec(`"${seenPath}" --version`, (error: any, stdout: string) => {
            if (error) {
                reject(error);
            } else {
                console.log(`Seen compiler found: ${stdout.trim()}`);
                resolve();
            }
        });
    });
}

async function checkSeenInPath(): Promise<boolean> {
    const cp = require('child_process');
    return new Promise((resolve) => {
        cp.exec('seen --version', (error: any) => {
            resolve(!error);
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
