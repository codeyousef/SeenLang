@echo off
REM Build native Windows Seen compiler
echo Building Windows Seen Compiler...

REM Step 1: Generate C code from Seen source using the current compiler
echo Step 1: Generating C code...
wsl -e /mnt/d/Projects/Rust/seenlang/compiler_seen/target/seen build compiler_seen/src/main.seen -o temp_main.c --target c

REM Step 2: Create Windows-compatible runtime
echo Step 2: Creating Windows runtime...
echo #include ^<stdio.h^> > seen_runtime_windows.c
echo #include ^<stdlib.h^> >> seen_runtime_windows.c
echo #include ^<string.h^> >> seen_runtime_windows.c
echo #include ^<windows.h^> >> seen_runtime_windows.c
echo. >> seen_runtime_windows.c
echo int WriteFile_seen(const char* path, const char* content) { >> seen_runtime_windows.c
echo     FILE* file = fopen(path, "w"); >> seen_runtime_windows.c
echo     if (!file) return 0; >> seen_runtime_windows.c
echo     fputs(content, file); >> seen_runtime_windows.c
echo     fclose(file); >> seen_runtime_windows.c
echo     return 1; >> seen_runtime_windows.c
echo } >> seen_runtime_windows.c
echo. >> seen_runtime_windows.c
echo char* ReadFile_seen(const char* path) { >> seen_runtime_windows.c
echo     FILE* file = fopen(path, "r"); >> seen_runtime_windows.c
echo     if (!file) return strdup(""); >> seen_runtime_windows.c
echo     fseek(file, 0, SEEK_END); >> seen_runtime_windows.c
echo     long size = ftell(file); >> seen_runtime_windows.c
echo     fseek(file, 0, SEEK_SET); >> seen_runtime_windows.c
echo     char* content = malloc(size + 1); >> seen_runtime_windows.c
echo     fread(content, 1, size, file); >> seen_runtime_windows.c
echo     content[size] = '\0'; >> seen_runtime_windows.c
echo     fclose(file); >> seen_runtime_windows.c
echo     return content; >> seen_runtime_windows.c
echo } >> seen_runtime_windows.c
echo. >> seen_runtime_windows.c
echo int ExecuteCommand_seen(const char* cmd) { >> seen_runtime_windows.c
echo     return system(cmd); >> seen_runtime_windows.c
echo } >> seen_runtime_windows.c

REM Step 3: Compile to Windows executable
echo Step 3: Compiling to Windows executable...
gcc -O3 -o seen_compiler.exe temp_main.c seen_runtime_windows.c -lws2_32

if exist seen_compiler.exe (
    echo Success! Windows compiler built at seen_compiler.exe
    seen_compiler.exe --version
) else (
    echo Failed to build Windows compiler
)

REM Clean up temporary files
del temp_main.c seen_runtime_windows.c 2>nul

echo Build complete!