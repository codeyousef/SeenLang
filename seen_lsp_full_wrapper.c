#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void send_response(const char* response) {
    int len = strlen(response);
    printf("Content-Length: %d\r\n\r\n%s", len, response);
    fflush(stdout);
}

void send_initialize_response(int id) {
    char response[1024];
    snprintf(response, sizeof(response),
        "{\"jsonrpc\":\"2.0\",\"id\":%d,\"result\":{\"capabilities\":{"
        "\"textDocumentSync\":1,"
        "\"hoverProvider\":true,"
        "\"definitionProvider\":true,"
        "\"completionProvider\":{\"triggerCharacters\":[\".\"]},"
        "\"diagnosticProvider\":true"
        "}}}",
        id);
    send_response(response);
}

void send_hover_response(int id) {
    char response[512];
    snprintf(response, sizeof(response),
        "{\"jsonrpc\":\"2.0\",\"id\":%d,\"result\":{"
        "\"contents\":{\"kind\":\"markdown\",\"value\":\"**Seen Variable**\\nType: String\"}"
        "}}",
        id);
    send_response(response);
}

void send_null_response(int id) {
    char response[256];
    snprintf(response, sizeof(response),
        "{\"jsonrpc\":\"2.0\",\"id\":%d,\"result\":null}",
        id);
    send_response(response);
}

int main() {
    // Redirect stderr to a log file
    freopen("C:\\Users\\youse\\AppData\\Local\\Temp\\seen_lsp.log", "w", stderr);
    
    fprintf(stderr, "Seen LSP Wrapper: Starting with proper JSON-RPC protocol...\n");
    fflush(stderr);
    
    char buffer[8192];
    int content_length = 0;
    
    while (1) {
        // Read Content-Length header
        if (fgets(buffer, sizeof(buffer), stdin) == NULL) break;
        
        if (strncmp(buffer, "Content-Length:", 15) == 0) {
            content_length = atoi(buffer + 15);
            fprintf(stderr, "Content-Length: %d\n", content_length);
            
            // Read the empty line
            fgets(buffer, sizeof(buffer), stdin);
            
            // Read the JSON content
            if (content_length > 0 && content_length < sizeof(buffer)) {
                size_t read = fread(buffer, 1, content_length, stdin);
                buffer[read] = '\0';
                
                fprintf(stderr, "Received: %s\n", buffer);
                
                // Parse the JSON request
                if (strstr(buffer, "\"method\":\"initialize\"")) {
                    // Extract ID
                    char* id_str = strstr(buffer, "\"id\":");
                    int id = 1;
                    if (id_str) {
                        id = atoi(id_str + 5);
                    }
                    fprintf(stderr, "Handling initialize with ID: %d\n", id);
                    send_initialize_response(id);
                }
                else if (strstr(buffer, "\"method\":\"initialized\"")) {
                    fprintf(stderr, "Received initialized notification\n");
                    // No response needed for notifications
                }
                else if (strstr(buffer, "\"method\":\"textDocument/hover\"")) {
                    char* id_str = strstr(buffer, "\"id\":");
                    int id = 1;
                    if (id_str) {
                        id = atoi(id_str + 5);
                    }
                    fprintf(stderr, "Handling hover with ID: %d\n", id);
                    send_hover_response(id);
                }
                else if (strstr(buffer, "\"method\":\"textDocument/definition\"")) {
                    char* id_str = strstr(buffer, "\"id\":");
                    int id = 1;
                    if (id_str) {
                        id = atoi(id_str + 5);
                    }
                    fprintf(stderr, "Handling definition with ID: %d\n", id);
                    send_null_response(id);
                }
                else if (strstr(buffer, "\"method\":\"shutdown\"")) {
                    char* id_str = strstr(buffer, "\"id\":");
                    int id = 1;
                    if (id_str) {
                        id = atoi(id_str + 5);
                    }
                    fprintf(stderr, "Handling shutdown with ID: %d\n", id);
                    send_null_response(id);
                    break;
                }
                else {
                    fprintf(stderr, "Unhandled method in: %s\n", buffer);
                }
            }
        }
    }
    
    fprintf(stderr, "LSP Wrapper: Shutting down\n");
    return 0;
}