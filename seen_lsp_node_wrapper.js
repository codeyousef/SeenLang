#!/usr/bin/env node

// Node.js wrapper for Seen LSP Server
// This ensures VS Code can always spawn the LSP server

const { spawn } = require('child_process');
const path = require('path');

// Log to a file for debugging
const fs = require('fs');
const logFile = 'C:\\Users\\youse\\AppData\\Local\\Temp\\seen_lsp_node_wrapper.log';

function log(message) {
    const timestamp = new Date().toISOString();
    fs.appendFileSync(logFile, `${timestamp}: ${message}\n`);
}

log('=== Seen LSP Node Wrapper Starting ===');

try {
    const lspExecutable = 'C:\\Users\\youse\\AppData\\Local\\Seen\\bin\\seen_lsp_wrapper.exe';
    
    log(`Spawning LSP server: ${lspExecutable}`);
    
    const child = spawn(lspExecutable, [], {
        stdio: ['pipe', 'pipe', 'pipe'],
        shell: false
    });
    
    // Pipe stdin/stdout/stderr
    process.stdin.pipe(child.stdin);
    child.stdout.pipe(process.stdout);
    child.stderr.pipe(process.stderr);
    
    child.on('error', (err) => {
        log(`LSP server error: ${err.message}`);
        process.exit(1);
    });
    
    child.on('exit', (code) => {
        log(`LSP server exited with code: ${code}`);
        process.exit(code);
    });
    
    log('LSP server spawned successfully');
    
} catch (error) {
    log(`Failed to start LSP server: ${error.message}`);
    process.exit(1);
}