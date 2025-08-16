// Seen Runtime Support Functions
// Provides file I/O and system command execution for the self-hosted compiler

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/wait.h>

// File I/O functions
int WriteFile(const char* path, const char* content) {
    FILE* file = fopen(path, "w");
    if (!file) {
        return 0; // false
    }
    
    fprintf(file, "%s", content);
    fclose(file);
    return 1; // true
}

char* ReadFile(const char* path) {
    FILE* file = fopen(path, "r");
    if (!file) {
        return NULL;
    }
    
    // Get file size
    fseek(file, 0, SEEK_END);
    long size = ftell(file);
    fseek(file, 0, SEEK_SET);
    
    // Allocate buffer
    char* content = malloc(size + 1);
    if (!content) {
        fclose(file);
        return NULL;
    }
    
    // Read content
    fread(content, 1, size, file);
    content[size] = '\0';
    
    fclose(file);
    return content;
}

// System command execution
typedef struct {
    int success;
    char* output;
} CommandResult;

CommandResult ExecuteCommand(const char* command) {
    CommandResult result = {0, NULL};
    
    FILE* pipe = popen(command, "r");
    if (!pipe) {
        return result;
    }
    
    // Read output
    char buffer[1024];
    size_t total_size = 0;
    char* output = NULL;
    
    while (fgets(buffer, sizeof(buffer), pipe)) {
        size_t len = strlen(buffer);
        output = realloc(output, total_size + len + 1);
        if (!output) {
            pclose(pipe);
            return result;
        }
        strcpy(output + total_size, buffer);
        total_size += len;
    }
    
    int status = pclose(pipe);
    result.success = (status == 0) ? 1 : 0;
    result.output = output ? output : strdup("");
    
    return result;
}

// String list functions for bootstrap
typedef struct StringList {
    char** items;
    int count;
    int capacity;
} StringList;

StringList* CreateBootstrapEmptyList() {
    StringList* list = malloc(sizeof(StringList));
    list->items = NULL;
    list->count = 0;
    list->capacity = 0;
    return list;
}

void AddToStringList(StringList* list, const char* item) {
    if (list->count >= list->capacity) {
        list->capacity = (list->capacity == 0) ? 4 : list->capacity * 2;
        list->items = realloc(list->items, list->capacity * sizeof(char*));
    }
    list->items[list->count] = strdup(item);
    list->count++;
}

int GetStringListLength(StringList* list) {
    return list->count;
}