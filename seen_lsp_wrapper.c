#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main() {
    // Redirect stderr to a log file
    freopen("C:\\Users\\youse\\AppData\\Local\\Temp\\seen_lsp.log", "w", stderr);
    
    // Print startup messages to stderr only
    fprintf(stderr, "Seen LSP Wrapper: Starting clean LSP server...\n");
    fflush(stderr);
    
    // Start the main loop for JSON-RPC
    char buffer[8192];
    while (fgets(buffer, sizeof(buffer), stdin)) {
        // Simple echo for now - just to test the pipe works
        if (strstr(buffer, "initialize")) {
            printf("Content-Length: 52\r\n\r\n");
            printf("{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"capabilities\":{}}}\n");
            fflush(stdout);
        }
        
        if (strstr(buffer, "shutdown")) {
            fprintf(stderr, "LSP Wrapper: Shutting down\n");
            break;
        }
    }
    
    return 0;
}