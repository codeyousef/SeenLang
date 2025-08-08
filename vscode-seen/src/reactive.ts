// src/reactive.ts
import * as vscode from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';

export class ReactiveVisualizer {
    private static webviewPanel: vscode.WebviewPanel | undefined;

    static show(streamData: any) {
        if (this.webviewPanel) {
            this.webviewPanel.reveal();
        } else {
            this.webviewPanel = vscode.window.createWebviewPanel(
                'seenReactiveStream',
                'Reactive Stream Visualization',
                vscode.ViewColumn.Two,
                {
                    enableScripts: true,
                    retainContextWhenHidden: true
                }
            );

            this.webviewPanel.onDidDispose(() => {
                this.webviewPanel = undefined;
            });
        }

        this.webviewPanel.webview.html = this.getMarbleDiagramHtml(streamData);
    }

    private static getMarbleDiagramHtml(streamData: any): string {
        return `<!DOCTYPE html>
        <html>
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>Reactive Stream Visualization</title>
            <style>
                body {
                    font-family: var(--vscode-font-family);
                    background: var(--vscode-editor-background);
                    color: var(--vscode-editor-foreground);
                    margin: 0;
                    padding: 20px;
                }
                
                .stream-container {
                    margin: 20px 0;
                    padding: 15px;
                    border: 1px solid var(--vscode-panel-border);
                    border-radius: 8px;
                }
                
                .stream-title {
                    font-size: 16px;
                    font-weight: bold;
                    margin-bottom: 10px;
                    color: var(--vscode-textLink-foreground);
                }
                
                .timeline {
                    position: relative;
                    height: 60px;
                    background: linear-gradient(to right, 
                        var(--vscode-terminal-ansiBlue) 0%,
                        var(--vscode-terminal-ansiBlue) 100%);
                    border-radius: 4px;
                    margin: 10px 0;
                }
                
                .timeline::before {
                    content: '';
                    position: absolute;
                    top: 50%;
                    left: 0;
                    right: 0;
                    height: 2px;
                    background: var(--vscode-terminal-ansiBlue);
                    transform: translateY(-50%);
                }
                
                .timeline::after {
                    content: '→';
                    position: absolute;
                    right: 10px;
                    top: 50%;
                    transform: translateY(-50%);
                    font-size: 18px;
                    color: var(--vscode-terminal-ansiBlue);
                }
                
                .marble {
                    position: absolute;
                    width: 24px;
                    height: 24px;
                    border-radius: 50%;
                    top: 50%;
                    transform: translateY(-50%);
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    font-size: 12px;
                    font-weight: bold;
                    color: white;
                    box-shadow: 0 2px 4px rgba(0,0,0,0.3);
                }
                
                .marble.value {
                    background: var(--vscode-terminal-ansiGreen);
                }
                
                .marble.error {
                    background: var(--vscode-terminal-ansiRed);
                }
                
                .marble.complete {
                    background: var(--vscode-terminal-ansiYellow);
                    width: 4px;
                    height: 24px;
                    border-radius: 2px;
                }
                
                .operator-info {
                    margin: 10px 0;
                    padding: 10px;
                    background: var(--vscode-textBlockQuote-background);
                    border-left: 4px solid var(--vscode-textLink-foreground);
                    border-radius: 4px;
                }
                
                .operator-name {
                    font-weight: bold;
                    color: var(--vscode-textLink-foreground);
                }
                
                .operator-description {
                    margin-top: 5px;
                    font-size: 14px;
                    color: var(--vscode-descriptionForeground);
                }
                
                .controls {
                    margin: 20px 0;
                    text-align: center;
                }
                
                .btn {
                    background: var(--vscode-button-background);
                    color: var(--vscode-button-foreground);
                    border: none;
                    padding: 8px 16px;
                    margin: 0 5px;
                    border-radius: 4px;
                    cursor: pointer;
                }
                
                .btn:hover {
                    background: var(--vscode-button-hoverBackground);
                }
            </style>
        </head>
        <body>
            <h1>Reactive Stream Visualization</h1>
            
            <div class="stream-container">
                <div class="stream-title">Source Stream</div>
                <div class="timeline" id="source-timeline"></div>
            </div>
            
            <div class="operator-info">
                <div class="operator-name">${streamData.operator || 'Unknown'}</div>
                <div class="operator-description">${streamData.description || 'No description available'}</div>
            </div>
            
            <div class="stream-container">
                <div class="stream-title">Output Stream</div>
                <div class="timeline" id="output-timeline"></div>
            </div>
            
            <div class="controls">
                <button class="btn" onclick="playAnimation()">▶ Play</button>
                <button class="btn" onclick="pauseAnimation()">⏸ Pause</button>
                <button class="btn" onclick="resetAnimation()">⏹ Reset</button>
            </div>
            
            <script>
                const vscode = acquireVsCodeApi();
                
                let animationState = 'stopped';
                let currentTime = 0;
                
                const streamData = ${JSON.stringify(streamData)};
                
                function createMarble(value, type = 'value') {
                    const marble = document.createElement('div');
                    marble.className = \`marble \${type}\`;
                    marble.textContent = type === 'complete' ? '' : value;
                    return marble;
                }
                
                function renderTimeline(timelineId, events) {
                    const timeline = document.getElementById(timelineId);
                    timeline.innerHTML = '';
                    
                    events.forEach((event, index) => {
                        const marble = createMarble(event.value, event.type);
                        const position = (index / (events.length - 1)) * 80; // 80% to leave space for arrow
                        marble.style.left = position + '%';
                        timeline.appendChild(marble);
                    });
                }
                
                function playAnimation() {
                    if (animationState !== 'playing') {
                        animationState = 'playing';
                        animate();
                    }
                }
                
                function pauseAnimation() {
                    animationState = 'paused';
                }
                
                function resetAnimation() {
                    animationState = 'stopped';
                    currentTime = 0;
                    initializeVisualization();
                }
                
                function animate() {
                    if (animationState !== 'playing') return;
                    
                    // Animation logic here
                    currentTime += 100;
                    
                    // Continue animation
                    setTimeout(animate, 100);
                }
                
                function initializeVisualization() {
                    // Render initial state
                    if (streamData.sourceEvents) {
                        renderTimeline('source-timeline', streamData.sourceEvents);
                    }
                    
                    if (streamData.outputEvents) {
                        renderTimeline('output-timeline', streamData.outputEvents);
                    }
                }
                
                // Initialize on load
                initializeVisualization();
                
                // Handle messages from extension
                window.addEventListener('message', event => {
                    const message = event.data;
                    switch (message.type) {
                        case 'updateStream':
                            streamData = message.data;
                            initializeVisualization();
                            break;
                    }
                });
            </script>
        </body>
        </html>`;
    }
}

export class ReactiveStreamViewProvider implements vscode.WebviewViewProvider {
    constructor(private readonly extensionUri: vscode.Uri) {}

    resolveWebviewView(
        webviewView: vscode.WebviewView,
        context: vscode.WebviewViewResolveContext,
        token: vscode.CancellationToken
    ) {
        webviewView.webview.options = {
            enableScripts: true,
            localResourceRoots: [this.extensionUri]
        };

        webviewView.webview.html = this.getHtmlContent(webviewView.webview);
    }

    private getHtmlContent(webview: vscode.Webview): string {
        return `<!DOCTYPE html>
        <html>
        <head>
            <style>
                body { 
                    padding: 10px; 
                    font-family: var(--vscode-font-family);
                    background: var(--vscode-editor-background);
                    color: var(--vscode-editor-foreground);
                }
                .stream { 
                    margin: 10px 0; 
                    padding: 8px;
                    border: 1px solid var(--vscode-panel-border);
                    border-radius: 4px;
                }
                .marble { 
                    display: inline-block; 
                    width: 20px; 
                    height: 20px; 
                    border-radius: 50%; 
                    margin: 2px;
                    text-align: center;
                    line-height: 20px;
                    font-size: 12px;
                }
                .timeline { 
                    border-bottom: 2px solid var(--vscode-editor-foreground);
                    position: relative;
                    height: 30px;
                    margin: 10px 0;
                }
                .no-streams {
                    text-align: center;
                    color: var(--vscode-descriptionForeground);
                    font-style: italic;
                    margin: 20px 0;
                }
            </style>
        </head>
        <body>
            <h3>Reactive Streams</h3>
            <div id="streams">
                <div class="no-streams">
                    No active reactive streams detected.
                    <br><br>
                    Position cursor on a reactive operator and use "Visualize Reactive Stream" command.
                </div>
            </div>
            <script>
                const vscode = acquireVsCodeApi();
                
                window.addEventListener('message', event => {
                    const message = event.data;
                    if (message.type === 'updateStream') {
                        updateStreamVisualization(message.data);
                    }
                });

                function updateStreamVisualization(data) {
                    const container = document.getElementById('streams');
                    
                    if (data && data.streams && data.streams.length > 0) {
                        container.innerHTML = '';
                        
                        data.streams.forEach(stream => {
                            const streamDiv = document.createElement('div');
                            streamDiv.className = 'stream';
                            streamDiv.innerHTML = \`
                                <strong>\${stream.name}</strong>
                                <div class="timeline"></div>
                                <small>Operator: \${stream.operator}</small>
                            \`;
                            container.appendChild(streamDiv);
                        });
                    } else {
                        container.innerHTML = \`
                            <div class="no-streams">
                                No active reactive streams detected.
                                <br><br>
                                Position cursor on a reactive operator and use "Visualize Reactive Stream" command.
                            </div>
                        \`;
                    }
                }
            </script>
        </body>
        </html>`;
    }
}

export class ReactiveInlineValueProvider implements vscode.InlineValuesProvider {
    constructor(private client: LanguageClient) {}

    async provideInlineValues(
        document: vscode.TextDocument,
        viewPort: vscode.Range,
        context: vscode.InlineValueContext,
        token: vscode.CancellationToken
    ): Promise<vscode.InlineValue[]> {
        const values: vscode.InlineValue[] = [];

        // Show reactive stream values during debugging
        if (context.stoppedLocation) {
            try {
                const streamStates = await this.client.sendRequest('seen/getStreamStates', {
                    uri: document.uri.toString(),
                    line: context.stoppedLocation.start.line
                });

                if (streamStates && Array.isArray(streamStates)) {
                    for (const state of streamStates) {
                        values.push(
                            new vscode.InlineValueText(
                                new vscode.Range(state.line, state.column, state.line, state.column),
                                `[${state.values.join(', ')}]`
                            )
                        );
                    }
                }
            } catch (error) {
                // Stream states not available
            }
        }

        return values;
    }
}