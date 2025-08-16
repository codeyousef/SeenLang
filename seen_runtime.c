#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/wait.h>

// File I/O functions for Seen compiler
int WriteFile(const char* path, const char* content) {
    FILE* file = fopen(path, "w");
    if (!file) {
        fprintf(stderr, "Error: Could not open file '%s' for writing\n", path);
        return 0;
    }
    
    if (fputs(content, file) == EOF) {
        fprintf(stderr, "Error: Could not write to file '%s'\n", path);
        fclose(file);
        return 0;
    }
    
    fclose(file);
    return 1;
}

char* ReadFile(const char* path) {
    FILE* file = fopen(path, "r");
    if (!file) {
        fprintf(stderr, "Error: Could not open file '%s' for reading\n", path);
        return strdup(""); // Return empty string on error
    }
    
    // Get file size
    fseek(file, 0, SEEK_END);
    long size = ftell(file);
    fseek(file, 0, SEEK_SET);
    
    // Allocate buffer
    char* content = malloc(size + 1);
    if (!content) {
        fprintf(stderr, "Error: Could not allocate memory for file '%s'\n", path);
        fclose(file);
        return strdup("");
    }
    
    // Read file
    size_t read_size = fread(content, 1, size, file);
    content[read_size] = '\0';
    
    fclose(file);
    return content;
}

// Command execution result structure
typedef struct {
    int success;
    char* output;
} CommandResult;

CommandResult ExecuteCommand(const char* command) {
    CommandResult result = {0, NULL};
    
    // Execute command and capture output
    FILE* pipe = popen(command, "r");
    if (!pipe) {
        result.success = 0;
        result.output = strdup("Error: Could not execute command");
        return result;
    }
    
    // Read output
    char buffer[4096];
    size_t total_size = 0;
    char* output = malloc(1);
    output[0] = '\0';
    
    while (fgets(buffer, sizeof(buffer), pipe)) {
        size_t buffer_len = strlen(buffer);
        output = realloc(output, total_size + buffer_len + 1);
        strcat(output, buffer);
        total_size += buffer_len;
    }
    
    int exit_status = pclose(pipe);
    result.success = (exit_status == 0) ? 1 : 0;
    result.output = output;
    
    return result;
}