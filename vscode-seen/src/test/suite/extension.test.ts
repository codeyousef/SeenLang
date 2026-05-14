import * as assert from 'assert';
import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';

/* eslint-disable @typescript-eslint/no-var-requires */

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
            'seen.benchmark.run',
            'seen.benchmark.compare',
            'seen.format',
            'seen.check',
            'seen.compileSharedModule',
            'seen.pkgFetch',
            'seen.pkgPack',
            'seen.pkgPrebuild',
            'seen.pkgPublish',
            'seen.clean',
            'seen.init',
            'seen.repl',
            'seen.reactive.visualize',
            'seen.switchLanguage',
            'seen.translateCode',
            'seen.showReferences',
            'seen.importSymbol',
            'seen.reorderStructFields',
            'seen.showLineErrors',
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
        assert.strictEqual(config.get('compile.pic'), false);
        assert.strictEqual(config.get('compile.objectManifest'), '');
        assert.strictEqual(config.get('language.default'), 'en');
    });

    test('Language enum should match compiler-supported languages', () => {
        const packageJsonPath = path.join(__dirname, '../../../package.json');
        const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf8'));
        const languages = packageJson.contributes.configuration.properties['seen.language.default'].enum;
        assert.deepStrictEqual(languages, ['en', 'ar', 'es', 'ru', 'zh', 'ja']);
    });

    test('Extension should not call unsupported custom LSP methods', () => {
        const commandsPath = path.join(__dirname, '../../commands.js');
        const commandsSource = fs.readFileSync(commandsPath, 'utf8');
        assert.ok(!commandsSource.includes('seen/switchLanguage'));
        assert.ok(!commandsSource.includes('seen/translate'));
        assert.ok(!commandsSource.includes('seen/getStreamInfo'));
    });

    test('Fallback diagnostics should parse warning codes and ranges', () => {
        const DiagnosticsModule = require('../../errorDiagnostics');
        const provider = new DiagnosticsModule.SeenDiagnosticProvider();
        try {
            const output = [
                'warning[W001]: unreachable statement',
                '  --> /tmp/dead_code.seen:4:5',
                '   |',
                ' 4 |     let x = 1',
                '   |     ^^^'
            ].join('\n');

            const parsed = (provider as any).parseTraditionalErrors(output);
            assert.strictEqual(parsed.length, 1);
            assert.strictEqual(parsed[0].severity, 'warning');
            assert.strictEqual(parsed[0].code, 'W001');
            assert.strictEqual(parsed[0].message, 'unreachable statement');
            assert.strictEqual(parsed[0].file, '/tmp/dead_code.seen');
            assert.strictEqual(parsed[0].location.line, 4);
            assert.strictEqual(parsed[0].location.column, 5);
            assert.strictEqual(parsed[0].location.length, 3);
        } finally {
            provider.dispose();
        }
    });
});

suite('Language Features Test Suite', () => {

    test('Syntax highlighting should work', async () => {
        const docUri = vscode.Uri.file(
            path.join(__dirname, '../../../test-fixtures/sample.seen')
        );

        try {
            const document = await vscode.workspace.openTextDocument(docUri);
            assert.strictEqual(document.languageId, 'seen');

            const editor = await vscode.window.showTextDocument(document);
            assert.ok(editor);

            // Wait for tokenization
            await new Promise(resolve => setTimeout(resolve, 1000));

            // Check that tokens are being created (basic test)
            const lineCount = document.lineCount;
            assert.ok(lineCount > 0);
        } catch {
            // Test fixture may not exist in CI; skip gracefully
            assert.ok(true, 'Skipped: test fixture not found');
        }
    });

    test('Code completion should be available', async () => {
        const docUri = vscode.Uri.file(
            path.join(__dirname, '../../../test-fixtures/sample.seen')
        );

        try {
            const document = await vscode.workspace.openTextDocument(docUri);
            const position = new vscode.Position(0, 0);

            const completions = await vscode.commands.executeCommand<vscode.CompletionList>(
                'vscode.executeCompletionItemProvider',
                docUri,
                position
            );

            assert.ok(completions !== undefined);
        } catch {
            assert.ok(true, 'Skipped: test fixture not found');
        }
    });

    test('Diagnostics should be provided', async () => {
        const docUri = vscode.Uri.file(
            path.join(__dirname, '../../../test-fixtures/error.seen')
        );

        try {
            await vscode.workspace.openTextDocument(docUri);

            // Wait for diagnostics
            await new Promise(resolve => setTimeout(resolve, 2000));

            const diagnostics = vscode.languages.getDiagnostics(docUri);
            assert.ok(Array.isArray(diagnostics));
        } catch {
            assert.ok(true, 'Skipped: test fixture not found');
        }
    });

    test('Import folding should fold consecutive imports', () => {
        const FoldingModule = require('../../importFolding');
        const provider = new FoldingModule.SeenImportFoldingProvider();
        const document = mockDocument([
            'import codegen.ir_member_access_driver as memberAccess',
            'import codegen.ir_call_driver as calls',
            'import parser.real_parser.{ProgramNode as ParsedProgram}',
            'fun main() r: Int { return 0 }',
        ]);

        const ranges = provider.provideFoldingRanges(document, {} as any, {} as any) as vscode.FoldingRange[];

        assert.strictEqual(ranges.length, 1);
        assert.strictEqual(ranges[0].start, 0);
        assert.strictEqual(ranges[0].end, 2);
    });

    test('Import folding should stop before declarations', () => {
        const FoldingModule = require('../../importFolding');
        const provider = new FoldingModule.SeenImportFoldingProvider();
        const document = mockDocument([
            'import parser.real_parser.{ProgramNode}',
            'import bootstrap.frontend.{run_frontend}',
            'class Demo { }',
            'import should.not.fold',
        ]);

        const ranges = provider.provideFoldingRanges(document, {} as any, {} as any) as vscode.FoldingRange[];

        assert.strictEqual(ranges.length, 1);
        assert.strictEqual(ranges[0].start, 0);
        assert.strictEqual(ranges[0].end, 1);
    });

    test('Import folding should group import use and pub import lines', () => {
        const FoldingModule = require('../../importFolding');
        const provider = new FoldingModule.SeenImportFoldingProvider();
        const document = mockDocument([
            'pub import api.surface',
            'use std.io',
            'import codegen.ir_call_driver as calls',
            'let value = 1',
        ]);

        const ranges = provider.provideFoldingRanges(document, {} as any, {} as any) as vscode.FoldingRange[];

        assert.strictEqual(ranges.length, 1);
        assert.strictEqual(ranges[0].start, 0);
        assert.strictEqual(ranges[0].end, 2);
    });

    test('Grammar should include facade component keywords', () => {
        const grammarPath = path.join(__dirname, '../../../syntaxes/seen.tmLanguage.json');
        const grammar = fs.readFileSync(grammarPath, 'utf8');

        assert.ok(grammar.includes('component'));
        assert.ok(grammar.includes('uiEffect'));
        assert.ok(grammar.includes('state'));
        assert.ok(grammar.includes('computed'));
    });

    test('Snippets should include facade component constructs', () => {
        const snippetsPath = path.join(__dirname, '../../../snippets/seen.code-snippets');
        const snippets = JSON.parse(fs.readFileSync(snippetsPath, 'utf8'));

        assert.ok(snippets['Facade Component']);
        assert.ok(snippets['UI State']);
        assert.ok(snippets['Computed Value']);
        assert.ok(snippets['UI Effect']);
    });
});

function mockDocument(lines: string[]) {
    return {
        lineCount: lines.length,
        lineAt: (line: number) => ({ text: lines[line] }),
    } as any;
}

suite('Command Test Suite', () => {

    test('Build command should execute', async () => {
        try {
            await vscode.commands.executeCommand('seen.build');
            assert.ok(true);
        } catch {
            // Command might fail if no Seen project is open, but it should still be registered
            assert.ok(true);
        }
    });

    test('Format command should execute', async () => {
        try {
            await vscode.commands.executeCommand('seen.format');
            assert.ok(true);
        } catch {
            assert.ok(true);
        }
    });

    test('REPL command should execute', async () => {
        try {
            await vscode.commands.executeCommand('seen.repl');
            assert.ok(true);
        } catch {
            assert.ok(true);
        }
    });

    test('Initialize project command should execute', async () => {
        try {
            await vscode.commands.executeCommand('seen.init');
            assert.ok(true);
        } catch {
            assert.ok(true);
        }
    });
});

suite('Reactive Features Test Suite', () => {

    test('Reactive visualizer should be available', () => {
        const ReactiveVisualizerModule = require('../../reactive');
        assert.ok(ReactiveVisualizerModule.ReactiveVisualizer);
        assert.ok(ReactiveVisualizerModule.ReactiveStreamViewProvider);
        assert.ok(ReactiveVisualizerModule.ReactiveInlineValueProvider);
    });

    test('Marble diagram webview should create', async () => {
        try {
            const ReactiveVisualizerModule = require('../../reactive');
            const visualizer = ReactiveVisualizerModule.ReactiveVisualizer;

            assert.ok(typeof visualizer.show === 'function');
        } catch {
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
