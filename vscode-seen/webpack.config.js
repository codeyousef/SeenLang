/* eslint-disable @typescript-eslint/no-var-requires */
const path = require('path');

/**@type {import('webpack').Configuration}*/
const config = {
    target: 'node', // vscode extensions run in node context
    mode: 'none', // this leaves the source code as close as possible to the original
    
    entry: './src/extension.ts', // the entry point of this extension
    output: {
        // the bundle is stored in the 'dist' folder
        path: path.resolve(__dirname, 'dist'),
        filename: 'extension.js',
        libraryTarget: 'commonjs2'
    },
    externals: {
        vscode: 'commonjs vscode' // the vscode-module is created on-the-fly and must be excluded
    },
    resolve: {
        // support reading TypeScript and JavaScript files
        extensions: ['.ts', '.js']
    },
    module: {
        rules: [
            {
                test: /\.ts$/,
                exclude: /node_modules/,
                use: [
                    {
                        loader: 'ts-loader'
                    }
                ]
            }
        ]
    },
    devtool: 'nosources-source-map',
    infrastructureLogging: {
        level: 'log', // enables logging required for problem matchers
    },
};

module.exports = config;