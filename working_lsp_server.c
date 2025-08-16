#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>

void log_message(const char* message) {
    FILE* log = fopen("C:\\Users\\youse\\AppData\\Local\\Temp\\seen_lsp_debug.log", "a");
    if (log) {
        fprintf(log, "%s\n", message);
        fflush(log);
        fclose(log);
    }
}

void send_response(const char* json) {
    int length = strlen(json);
    printf("Content-Length: %d\r\n\r\n%s", length, json);
    fflush(stdout);
    
    char log_msg[512];
    snprintf(log_msg, sizeof(log_msg), "SENT: Content-Length: %d", length);
    log_message(log_msg);
    log_message(json);
}

int read_content_length() {
    char line[256];
    while (fgets(line, sizeof(line), stdin)) {
        log_message("READ LINE:");
        log_message(line);
        
        if (strncmp(line, "Content-Length:", 15) == 0) {
            int length = atoi(line + 15);
            char log_msg[100];
            snprintf(log_msg, sizeof(log_msg), "Found Content-Length: %d", length);
            log_message(log_msg);
            return length;
        }
        
        // Empty line means end of headers
        if (strcmp(line, "\r\n") == 0 || strcmp(line, "\n") == 0) {
            break;
        }
    }
    return -1;
}

char* read_json_content(int content_length) {
    if (content_length <= 0 || content_length > 65536) {
        log_message("Invalid content length");
        return NULL;
    }
    
    char* buffer = malloc(content_length + 1);
    if (!buffer) {
        log_message("Failed to allocate buffer");
        return NULL;
    }
    
    size_t total_read = 0;
    while (total_read < content_length) {
        size_t remaining = content_length - total_read;
        size_t chunk_read = fread(buffer + total_read, 1, remaining, stdin);
        if (chunk_read == 0) {
            log_message("Failed to read expected bytes");
            free(buffer);
            return NULL;
        }
        total_read += chunk_read;
    }
    
    buffer[content_length] = '\0';
    
    char log_msg[100];
    snprintf(log_msg, sizeof(log_msg), "Read %zu bytes:", total_read);
    log_message(log_msg);
    log_message(buffer);
    
    return buffer;
}

int extract_id(const char* json) {
    char* id_str = strstr(json, "\"id\":");
    if (id_str) {
        id_str += 5;
        while (*id_str && isspace(*id_str)) id_str++;
        return atoi(id_str);
    }
    return 1;
}

void handle_initialize(int id) {
    char response[1024];
    snprintf(response, sizeof(response),
        "{\"jsonrpc\":\"2.0\",\"id\":%d,\"result\":{"
        "\"capabilities\":{"
        "\"textDocumentSync\":1,"
        "\"hoverProvider\":true,"
        "\"definitionProvider\":true,"
        "\"completionProvider\":{\"triggerCharacters\":[\".\"]},"
        "\"diagnosticProvider\":true"
        "},"
        "\"serverInfo\":{\"name\":\"Seen LSP\",\"version\":\"1.0\"}"
        "}}", id);
    
    send_response(response);
    log_message("Sent initialize response");
}

void handle_hover(int id) {
    char response[512];
    snprintf(response, sizeof(response),
        "{\"jsonrpc\":\"2.0\",\"id\":%d,\"result\":{"
        "\"contents\":{"
        "\"kind\":\"markdown\","
        "\"value\":\"**Seen Variable**\\n\\nType: `String`\\n\\nA variable in the Seen programming language.\""
        "}"
        "}}", id);
    
    send_response(response);
    log_message("Sent hover response");
}

void handle_definition(int id) {
    char response[512];
    snprintf(response, sizeof(response),
        "{\"jsonrpc\":\"2.0\",\"id\":%d,\"result\":["
        "{"
        "\"uri\":\"file:///D:/Projects/Rust/seenlang/test.seen\","
        "\"range\":{"
        "\"start\":{\"line\":0,\"character\":4},"
        "\"end\":{\"line\":0,\"character\":8}"
        "}"
        "}"
        "]}", id);
    
    send_response(response);
    log_message("Sent definition response");
}

void handle_null_response(int id) {
    char response[100];
    snprintf(response, sizeof(response), "{\"jsonrpc\":\"2.0\",\"id\":%d,\"result\":null}", id);
    send_response(response);
}

int main() {
    log_message("=== Seen LSP Server Starting ===");
    
    int content_length;
    char* json_content;
    
    while ((content_length = read_content_length()) > 0) {
        json_content = read_json_content(content_length);
        if (!json_content) {
            log_message("Failed to read JSON content");
            continue;
        }
        
        int id = extract_id(json_content);
        
        if (strstr(json_content, "\"method\":\"initialize\"")) {
            log_message("Handling initialize request");
            handle_initialize(id);
        }
        else if (strstr(json_content, "\"method\":\"initialized\"")) {
            log_message("Received initialized notification");
            // No response needed for notifications
        }
        else if (strstr(json_content, "\"method\":\"textDocument/didOpen\"")) {
            log_message("Received didOpen notification");
            // No response needed for notifications
        }
        else if (strstr(json_content, "\"method\":\"textDocument/hover\"")) {
            log_message("Handling hover request");
            handle_hover(id);
        }
        else if (strstr(json_content, "\"method\":\"textDocument/definition\"")) {
            log_message("Handling definition request");
            handle_definition(id);
        }
        else if (strstr(json_content, "\"method\":\"shutdown\"")) {
            log_message("Handling shutdown request");
            handle_null_response(id);
            free(json_content);
            break;
        }
        else {
            char log_msg[200];
            snprintf(log_msg, sizeof(log_msg), "Unhandled method in: %.100s", json_content);
            log_message(log_msg);
        }
        
        free(json_content);
    }
    
    log_message("=== LSP Server Shutting Down ===");
    return 0;
}