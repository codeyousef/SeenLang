@echo off
echo Testing LSP server manually...
echo.

echo Testing hover request:
echo {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}} | D:\Projects\Rust\seenlang\compiler_seen\target\seen.exe lsp

echo.
echo Testing definition request:
echo {"jsonrpc":"2.0","id":1,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///test.seen"},"position":{"line":1,"character":5}}} | D:\Projects\Rust\seenlang\compiler_seen\target\seen.exe lsp

pause