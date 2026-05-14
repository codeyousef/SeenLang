import * as vscode from 'vscode';

export class SeenImportFoldingProvider implements vscode.FoldingRangeProvider {
    provideFoldingRanges(
        document: vscode.TextDocument,
        _context: vscode.FoldingContext,
        _token: vscode.CancellationToken
    ): vscode.ProviderResult<vscode.FoldingRange[]> {
        const ranges: vscode.FoldingRange[] = [];
        let blockStart = -1;

        for (let line = 0; line < document.lineCount; line += 1) {
            if (this.isTopLevelImportLine(document.lineAt(line).text)) {
                if (blockStart < 0) {
                    blockStart = line;
                }
            } else {
                this.closeRange(ranges, blockStart, line - 1);
                blockStart = -1;
            }
        }

        this.closeRange(ranges, blockStart, document.lineCount - 1);
        return ranges;
    }

    private isTopLevelImportLine(line: string): boolean {
        if (line.trim() === '') {
            return false;
        }
        if (/^\s/.test(line)) {
            return false;
        }
        return /^(?:pub\s+)?(?:import|use)\b/.test(line);
    }

    private closeRange(
        ranges: vscode.FoldingRange[],
        startLine: number,
        endLine: number
    ): void {
        if (startLine >= 0 && endLine > startLine) {
            ranges.push(new vscode.FoldingRange(
                startLine,
                endLine,
                vscode.FoldingRangeKind.Imports
            ));
        }
    }
}
