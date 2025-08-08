import * as path from 'path';
import { runTests } from '@vscode/test-electron';

async function main() {
    try {
        // The folder containing the Extension Manifest package.json
        const extensionDevelopmentPath = path.resolve(__dirname, '../../');

        // The path to test runner
        const extensionTestsPath = path.resolve(__dirname, './suite/index');

        // The workspace path for tests
        const workspacePath = path.resolve(__dirname, '../../test-fixtures');

        // Download VS Code, unzip it and run the integration test
        await runTests({
            extensionDevelopmentPath,
            extensionTestsPath,
            launchArgs: [
                workspacePath,
                '--disable-extensions'
            ]
        });
    } catch (err) {
        console.error('Failed to run tests');
        process.exit(1);
    }
}

main();