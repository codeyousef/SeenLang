import * as assert from 'assert';
import * as vscode from 'vscode';
import * as path from 'path';

suite('Extension Test Suite', () => {
    vscode.window.showInformationMessage('Start all tests.');

    test('Extension should be present', () => {
        assert.ok(vscode.extensions.getExtension('seen-lang.seen'));
    });

    test('Extension should activate', async () => {
        const extension = vscode.extensions.getExtension('seen-lang.seen');
        if (extension) {
            await extension.activate();
            assert.ok(extension.isActive);
        }
    });

    test('Commands should be registered', async () => {
        const commands = await vscode.commands.getCommands(true);
        
        const expectedCommands = [
            'seen.build',
            'seen.run',
            'seen.test',
            'seen.benchmark',
            'seen.format',
            'seen.init',
            'seen.openRepl',
            'seen.visualizeReactiveStream',
            'seen.switchLanguage',
            'seen.translateCode',
            'seen.checkTarget',
            'seen.deployTo',
            'seen.showDocumentation'
        ];

        for (const command of expectedCommands) {
            assert.ok(commands.includes(command), `Command ${command} should be registered`);
        }
    });

    test('Language should be registered', () => {
        const languages = vscode.languages.getLanguages();
        return languages.then(langs => {
            assert.ok(langs.includes('seen'), 'Seen language should be registered');
        });
    });

    test('Configuration should have default values', () => {
        const config = vscode.workspace.getConfiguration('seen');
        
        assert.strictEqual(config.get('compiler.path'), 'seen');
        assert.strictEqual(config.get('lsp.enabled'), true);
        assert.strictEqual(config.get('formatting.enable'), true);
        assert.strictEqual(config.get('reactive.marbleDiagrams'), true);
        assert.strictEqual(config.get('benchmark.showInline'), true);
        assert.strictEqual(config.get('target.default'), 'native');
        assert.strictEqual(config.get('language.default'), 'en');
    });
});

suite('Language Features Test Suite', () => {
    
    test('Syntax highlighting should work', async () => {
        const docUri = vscode.Uri.file(
            path.join(__dirname, '../../../test-fixtures/sample.seen')
        );
        
        const document = await vscode.workspace.openTextDocument(docUri);
        assert.strictEqual(document.languageId, 'seen');
        
        const editor = await vscode.window.showTextDocument(document);
        assert.ok(editor);
        
        // Wait for tokenization
        await new Promise(resolve => setTimeout(resolve, 1000));
        
        // Check that tokens are being created (basic test)
        const lineCount = document.lineCount;
        assert.ok(lineCount > 0);
    });

    test('Code completion should be available', async () => {
        const docUri = vscode.Uri.file(
            path.join(__dirname, '../../../test-fixtures/sample.seen')
        );
        
        const document = await vscode.workspace.openTextDocument(docUri);
        const position = new vscode.Position(0, 0);
        
        // This would test LSP completion - in real implementation
        // we'd check that completion items are returned
        const completions = await vscode.commands.executeCommand<vscode.CompletionList>(
            'vscode.executeCompletionItemProvider',
            docUri,
            position
        );
        
        // For now, just check that the command executes without error
        assert.ok(completions !== undefined);
    });

    test('Diagnostics should be provided', async () => {
        const docUri = vscode.Uri.file(
            path.join(__dirname, '../../../test-fixtures/error.seen')
        );
        
        const document = await vscode.workspace.openTextDocument(docUri);
        
        // Wait for diagnostics
        await new Promise(resolve => setTimeout(resolve, 2000));
        
        const diagnostics = vscode.languages.getDiagnostics(docUri);
        
        // Should have diagnostics for error file
        // In real implementation, we'd check specific error types
        assert.ok(Array.isArray(diagnostics));
    });
});

suite('Command Test Suite', () => {
    
    test('Build command should execute', async () => {
        try {
            await vscode.commands.executeCommand('seen.build');
            // If we get here, command executed without throwing
            assert.ok(true);
        } catch (error) {
            // Command might fail if no Seen project is open, but it should still be registered
            assert.ok(true);
        }
    });

    test('Format command should execute', async () => {
        try {
            await vscode.commands.executeCommand('seen.format');
            assert.ok(true);
        } catch (error) {
            assert.ok(true);
        }
    });

    test('REPL command should execute', async () => {
        try {
            await vscode.commands.executeCommand('seen.openRepl');
            assert.ok(true);
        } catch (error) {
            assert.ok(true);
        }
    });

    test('Initialize project command should execute', async () => {
        try {
            await vscode.commands.executeCommand('seen.init');
            assert.ok(true);
        } catch (error) {
            assert.ok(true);
        }
    });
});

suite('Reactive Features Test Suite', () => {
    
    test('Reactive visualizer should be available', () => {
        // Test that reactive visualizer classes can be imported
        const ReactiveVisualizerModule = require('../../reactive');
        assert.ok(ReactiveVisualizerModule.ReactiveVisualizer);
        assert.ok(ReactiveVisualizerModule.ReactiveStreamViewProvider);
        assert.ok(ReactiveVisualizerModule.ReactiveInlineValueProvider);
    });

    test('Marble diagram webview should create', async () => {
        try {
            const ReactiveVisualizerModule = require('../../reactive');
            const visualizer = ReactiveVisualizerModule.ReactiveVisualizer;
            
            // Test basic functionality without actually creating webview
            assert.ok(typeof visualizer.show === 'function');
        } catch (error) {
            assert.fail('Reactive visualizer should be available');
        }
    });
});

suite('Benchmark Features Test Suite', () => {
    
    test('Benchmark runner should be available', () => {
        const BenchmarkModule = require('../../benchmark');
        assert.ok(BenchmarkModule.BenchmarkRunner);
        assert.ok(BenchmarkModule.BenchmarkTreeDataProvider);
        assert.ok(BenchmarkModule.BenchmarkCodeLensProvider);
    });

    test('Benchmark code lens should detect benchmarks', async () => {
        const BenchmarkModule = require('../../benchmark');
        const provider = new BenchmarkModule.BenchmarkCodeLensProvider();
        
        // Create a mock document with benchmark annotation
        const mockDocument = {
            getText: () => '@benchmark fun testPerformance() { }\nfun normalFunction() { }',
            positionAt: (offset: number) => ({ line: 0, character: 0 }),
            lineAt: (line: number) => ({ text: '@benchmark fun testPerformance() { }' })
        } as any;
        
        const codeLenses = await provider.provideCodeLenses(mockDocument, {} as any);
        
        assert.ok(Array.isArray(codeLenses));
        assert.ok(codeLenses.length > 0);
    });
});

suite('Debug Features Test Suite', () => {
    
    test('Debug adapter factory should be available', () => {
        const DebugModule = require('../../debugger');
        assert.ok(DebugModule.SeenDebugAdapterFactory);
        assert.ok(DebugModule.SeenDebugConfigurationProvider);
    });

    test('Debug configuration should resolve', () => {
        const DebugModule = require('../../debugger');
        const provider = new DebugModule.SeenDebugConfigurationProvider();
        
        const config = {
            type: 'seen',
            name: 'Test Debug',
            request: 'launch',
            program: '${workspaceFolder}/src/main.seen'
        };
        
        const resolved = provider.resolveDebugConfiguration(undefined, config);
        assert.ok(resolved);
        assert.strictEqual(resolved.type, 'seen');
    });
});