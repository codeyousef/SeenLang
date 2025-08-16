// Inline LSP server implementation to avoid spawn issues
import { 
    createConnection, 
    TextDocuments, 
    ProposedFeatures, 
    InitializeParams, 
    DidChangeConfigurationNotification, 
    CompletionItem, 
    CompletionItemKind, 
    TextDocumentPositionParams, 
    TextDocumentSyncKind, 
    InitializeResult,
    Hover,
    Definition,
    Location,
    Range,
    Position
} from 'vscode-languageserver/node';

import { TextDocument } from 'vscode-languageserver-textdocument';

export class InlineLSPServer {
    private connection = createConnection(ProposedFeatures.all);
    private documents = new TextDocuments(TextDocument);
    
    constructor() {
        this.setupHandlers();
    }
    
    private setupHandlers() {
        // Initialize handler
        this.connection.onInitialize((params: InitializeParams) => {
            const result: InitializeResult = {
                capabilities: {
                    textDocumentSync: TextDocumentSyncKind.Incremental,
                    hoverProvider: true,
                    definitionProvider: true,
                    completionProvider: {
                        resolveProvider: true,
                        triggerCharacters: ['.']
                    }
                }
            };
            return result;
        });

        this.connection.onInitialized(() => {
            this.connection.client.register(DidChangeConfigurationNotification.type, undefined);
        });

        // Hover handler
        this.connection.onHover((textDocumentPosition: TextDocumentPositionParams): Hover => {
            const document = this.documents.get(textDocumentPosition.textDocument.uri);
            if (!document) {
                return { contents: [] };
            }

            const position = textDocumentPosition.position;
            const text = document.getText();
            const lines = text.split('\n');
            const currentLine = lines[position.line];
            
            // Simple word extraction at cursor position
            const wordMatch = currentLine.match(/\b\w+\b/g);
            if (wordMatch) {
                const word = this.getWordAtPosition(currentLine, position.character);
                if (word) {
                    return {
                        contents: {
                            kind: 'markdown',
                            value: `**Seen Variable: \`${word}\`**\n\nType: \`String\`\n\nA variable in the Seen programming language.`
                        }
                    };
                }
            }

            return { contents: [] };
        });

        // Definition handler
        this.connection.onDefinition((textDocumentPosition: TextDocumentPositionParams): Definition => {
            const document = this.documents.get(textDocumentPosition.textDocument.uri);
            if (!document) {
                return [];
            }

            const position = textDocumentPosition.position;
            const text = document.getText();
            const lines = text.split('\n');
            const currentLine = lines[position.line];
            const word = this.getWordAtPosition(currentLine, position.character);

            if (word) {
                // Find the definition (simple search for "let word" or "fun word")
                for (let i = 0; i < lines.length; i++) {
                    const line = lines[i];
                    const letMatch = line.match(new RegExp(`\\blet\\s+${word}\\b`));
                    const funMatch = line.match(new RegExp(`\\bfun\\s+${word}\\b`));
                    
                    if (letMatch || funMatch) {
                        const startChar = line.indexOf(word);
                        return [{
                            uri: textDocumentPosition.textDocument.uri,
                            range: {
                                start: { line: i, character: startChar },
                                end: { line: i, character: startChar + word.length }
                            }
                        }];
                    }
                }
            }

            return [];
        });

        // Completion handler
        this.connection.onCompletion((textDocumentPosition: TextDocumentPositionParams): CompletionItem[] => {
            return [
                {
                    label: 'let',
                    kind: CompletionItemKind.Keyword,
                    data: 1
                },
                {
                    label: 'fun',
                    kind: CompletionItemKind.Keyword,
                    data: 2
                },
                {
                    label: 'String',
                    kind: CompletionItemKind.Class,
                    data: 3
                },
                {
                    label: 'return',
                    kind: CompletionItemKind.Keyword,
                    data: 4
                }
            ];
        });

        this.connection.onCompletionResolve((item: CompletionItem): CompletionItem => {
            if (item.data === 1) {
                item.detail = 'Seen variable declaration';
                item.documentation = 'Declares a new variable in Seen';
            } else if (item.data === 2) {
                item.detail = 'Seen function declaration';
                item.documentation = 'Declares a new function in Seen';
            }
            return item;
        });
    }

    private getWordAtPosition(line: string, character: number): string | null {
        const beforeCursor = line.substring(0, character);
        const afterCursor = line.substring(character);
        
        const beforeMatch = beforeCursor.match(/\w+$/);
        const afterMatch = afterCursor.match(/^\w+/);
        
        const before = beforeMatch ? beforeMatch[0] : '';
        const after = afterMatch ? afterMatch[0] : '';
        
        const word = before + after;
        return word.length > 0 ? word : null;
    }

    public start() {
        this.documents.listen(this.connection);
        this.connection.listen();
    }
}