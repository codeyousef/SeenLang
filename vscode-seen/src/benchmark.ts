// src/benchmark.ts
import * as vscode from 'vscode';

export class BenchmarkRunner {
    private static outputChannel: vscode.OutputChannel;

    static showResults(results: any) {
        if (!this.outputChannel) {
            this.outputChannel = vscode.window.createOutputChannel('Seen Benchmarks');
        }

        this.outputChannel.clear();
        this.outputChannel.appendLine('='.repeat(50));
        this.outputChannel.appendLine('SEEN BENCHMARK RESULTS');
        this.outputChannel.appendLine('='.repeat(50));
        
        if (results.benchmarks && Array.isArray(results.benchmarks)) {
            for (const bench of results.benchmarks) {
                this.outputChannel.appendLine('');
                this.outputChannel.appendLine(`📊 ${bench.name}`);
                this.outputChannel.appendLine(`   Time: ${bench.mean}ms (±${bench.stddev || 0}ms)`);
                this.outputChannel.appendLine(`   Iterations: ${bench.iterations || 'N/A'}`);
                
                if (bench.throughput) {
                    this.outputChannel.appendLine(`   Throughput: ${bench.throughput} ops/sec`);
                }
                
                if (bench.comparison) {
                    const improvement = bench.comparison.improvement;
                    const vs = bench.comparison.baseline;
                    if (improvement > 0) {
                        this.outputChannel.appendLine(`   ✅ ${improvement.toFixed(2)}% faster than ${vs}`);
                    } else {
                        this.outputChannel.appendLine(`   ❌ ${Math.abs(improvement).toFixed(2)}% slower than ${vs}`);
                    }
                }
            }
        }
        
        this.outputChannel.appendLine('');
        this.outputChannel.appendLine('='.repeat(50));
        this.outputChannel.show();
    }
}

export interface BenchmarkResult {
    name: string;
    time: number;
    variance: number;
    iterations: number;
    throughput?: number;
    comparison?: {
        baseline: string;
        improvement: number;
    };
}

export class BenchmarkTreeDataProvider implements vscode.TreeDataProvider<BenchmarkItem> {
    private _onDidChangeTreeData = new vscode.EventEmitter<BenchmarkItem | undefined | null | void>();
    readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

    private benchmarks: Map<string, BenchmarkResult[]> = new Map();

    refresh(): void {
        this._onDidChangeTreeData.fire();
    }

    getTreeItem(element: BenchmarkItem): vscode.TreeItem {
        return element;
    }

    getChildren(element?: BenchmarkItem): Thenable<BenchmarkItem[]> {
        if (!element) {
            // Return root level items (benchmark files)
            return Promise.resolve(
                Array.from(this.benchmarks.keys()).map(
                    file => new BenchmarkItem(
                        file, 
                        vscode.TreeItemCollapsibleState.Collapsed,
                        'file'
                    )
                )
            );
        } else if (element.contextValue === 'file') {
            // Return benchmark results for a file
            const results = this.benchmarks.get(element.label as string) || [];
            return Promise.resolve(
                results.map(r => {
                    const label = `${r.name}: ${r.time}ms (±${r.variance}%)`;
                    const item = new BenchmarkItem(
                        label,
                        vscode.TreeItemCollapsibleState.None,
                        'benchmark'
                    );
                    
                    item.tooltip = `
                        Name: ${r.name}
                        Time: ${r.time}ms
                        Variance: ±${r.variance}%
                        Iterations: ${r.iterations}
                        ${r.throughput ? `Throughput: ${r.throughput} ops/sec` : ''}
                    `.trim();
                    
                    // Set icon based on performance
                    if (r.comparison) {
                        if (r.comparison.improvement > 5) {
                            item.iconPath = new vscode.ThemeIcon('trending-up', new vscode.ThemeColor('charts.green'));
                        } else if (r.comparison.improvement < -5) {
                            item.iconPath = new vscode.ThemeIcon('trending-down', new vscode.ThemeColor('charts.red'));
                        } else {
                            item.iconPath = new vscode.ThemeIcon('dashboard', new vscode.ThemeColor('charts.yellow'));
                        }
                    } else {
                        item.iconPath = new vscode.ThemeIcon('pulse');
                    }
                    
                    return item;
                })
            );
        } else {
            return Promise.resolve([]);
        }
    }

    addBenchmarkResult(file: string, result: BenchmarkResult) {
        if (!this.benchmarks.has(file)) {
            this.benchmarks.set(file, []);
        }
        
        const results = this.benchmarks.get(file)!;
        
        // Remove existing result with same name
        const existingIndex = results.findIndex(r => r.name === result.name);
        if (existingIndex !== -1) {
            results.splice(existingIndex, 1);
        }
        
        results.push(result);
        this.refresh();
    }

    clearResults() {
        this.benchmarks.clear();
        this.refresh();
    }
}

export class BenchmarkItem extends vscode.TreeItem {
    constructor(
        public readonly label: string,
        public readonly collapsibleState: vscode.TreeItemCollapsibleState,
        public readonly contextValue: string
    ) {
        super(label, collapsibleState);
        this.tooltip = this.label;
    }
}

export class BenchmarkCodeLensProvider implements vscode.CodeLensProvider {
    private _onDidChangeCodeLenses = new vscode.EventEmitter<void>();
    readonly onDidChangeCodeLenses = this._onDidChangeCodeLenses.event;

    constructor() {
        // Refresh code lenses when benchmark configuration changes
        vscode.workspace.onDidChangeConfiguration(e => {
            if (e.affectsConfiguration('seen.benchmark')) {
                this._onDidChangeCodeLenses.fire();
            }
        });
    }

    async provideCodeLenses(
        document: vscode.TextDocument,
        token: vscode.CancellationToken
    ): Promise<vscode.CodeLens[]> {
        const codeLenses: vscode.CodeLens[] = [];
        
        if (!vscode.workspace.getConfiguration('seen.benchmark').get('showInline', true)) {
            return codeLenses;
        }

        // Match @benchmark annotations
        const benchmarkRegex = /@benchmark\s+fun\s+(\w+)/g;
        const text = document.getText();
        let matches;

        while ((matches = benchmarkRegex.exec(text)) !== null) {
            const line = document.positionAt(matches.index).line;
            const range = new vscode.Range(line, 0, line, 0);
            const benchmarkName = matches[1];
            
            // Run benchmark code lens
            codeLenses.push(
                new vscode.CodeLens(range, {
                    title: '▶ Run Benchmark',
                    command: 'seen.benchmark.run',
                    arguments: [document.uri, benchmarkName]
                })
            );
            
            // Compare benchmark code lens
            codeLenses.push(
                new vscode.CodeLens(range, {
                    title: '📊 Compare',
                    command: 'seen.benchmark.compare',
                    arguments: [document.uri, benchmarkName]
                })
            );
            
            // View history code lens
            codeLenses.push(
                new vscode.CodeLens(range, {
                    title: '📈 History',
                    command: 'seen.benchmark.history',
                    arguments: [document.uri, benchmarkName]
                })
            );
        }

        // Match regular functions that could be benchmarked
        const functionRegex = /fun\s+(\w+)\s*\([^)]*\)/g;
        
        while ((matches = functionRegex.exec(text)) !== null) {
            const line = document.positionAt(matches.index).line;
            const functionName = matches[1];
            
            // Skip if already has @benchmark
            const lineText = document.lineAt(line).text;
            const previousLine = line > 0 ? document.lineAt(line - 1).text : '';
            
            if (!lineText.includes('@benchmark') && !previousLine.includes('@benchmark')) {
                const range = new vscode.Range(line, 0, line, 0);
                
                codeLenses.push(
                    new vscode.CodeLens(range, {
                        title: '⚡ Add Benchmark',
                        command: 'seen.benchmark.add',
                        arguments: [document.uri, functionName, line]
                    })
                );
            }
        }

        return codeLenses;
    }

    refresh() {
        this._onDidChangeCodeLenses.fire();
    }
}

// Register additional benchmark commands
export function registerBenchmarkCommands(context: vscode.ExtensionContext) {
    // Add benchmark annotation
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.benchmark.add', async (uri: vscode.Uri, functionName: string, line: number) => {
            const document = await vscode.workspace.openTextDocument(uri);
            const editor = await vscode.window.showTextDocument(document);
            
            const edit = new vscode.WorkspaceEdit();
            const insertPosition = new vscode.Position(line, 0);
            edit.insert(uri, insertPosition, '@benchmark\n');
            
            await vscode.workspace.applyEdit(edit);
            vscode.window.showInformationMessage(`Added @benchmark annotation to ${functionName}`);
        })
    );
    
    // Show benchmark history
    context.subscriptions.push(
        vscode.commands.registerCommand('seen.benchmark.history', async (uri: vscode.Uri, benchmarkName: string) => {
            // This would show historical benchmark data
            const panel = vscode.window.createWebviewPanel(
                'benchmarkHistory',
                `Benchmark History: ${benchmarkName}`,
                vscode.ViewColumn.Two,
                {
                    enableScripts: true
                }
            );
            
            panel.webview.html = getBenchmarkHistoryHtml(benchmarkName);
        })
    );
}

function getBenchmarkHistoryHtml(benchmarkName: string): string {
    return `<!DOCTYPE html>
    <html>
    <head>
        <meta charset="UTF-8">
        <title>Benchmark History</title>
        <style>
            body {
                font-family: var(--vscode-font-family);
                background: var(--vscode-editor-background);
                color: var(--vscode-editor-foreground);
                padding: 20px;
            }
            .chart-container {
                margin: 20px 0;
                padding: 15px;
                border: 1px solid var(--vscode-panel-border);
                border-radius: 8px;
            }
            .no-data {
                text-align: center;
                color: var(--vscode-descriptionForeground);
                font-style: italic;
                margin: 40px 0;
            }
        </style>
    </head>
    <body>
        <h1>Benchmark History: ${benchmarkName}</h1>
        
        <div class="chart-container">
            <div class="no-data">
                No historical data available yet.
                <br><br>
                Run benchmarks multiple times to see performance trends.
            </div>
        </div>
        
        <script>
            // Chart implementation would go here
            // Could use Chart.js or similar library
        </script>
    </body>
    </html>`;
}