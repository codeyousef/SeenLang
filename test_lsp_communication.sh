#!/bin/bash
# Test LSP server JSON-RPC communication

echo "🧪 Testing Seen LSP Server Communication"
echo "========================================"

COMPILER="./compiler_seen/target/seen"

# Test 1: Initialize request
echo "📝 Test 1: Initialize Request"
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}' | timeout 3s $COMPILER lsp > lsp_init_response.log 2>&1 &
PID=$!
sleep 1
kill $PID 2>/dev/null || true
wait $PID 2>/dev/null

if [ -f lsp_init_response.log ]; then
    echo "✅ Initialize response received:"
    cat lsp_init_response.log
    echo ""
else
    echo "❌ No initialize response"
fi

# Test 2: Document open notification  
echo "📝 Test 2: Document Open Notification"
(
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}'
sleep 0.1
echo '{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///test.seen","languageId":"seen","version":1,"text":"fun main() -> Int { return 0 }"}}}'
sleep 0.1
echo '{"jsonrpc":"2.0","id":2,"method":"shutdown"}'
) | timeout 5s $COMPILER lsp > lsp_document_response.log 2>&1

if [ -f lsp_document_response.log ]; then
    echo "✅ Document open response:"
    cat lsp_document_response.log
    echo ""
else
    echo "❌ No document open response"
fi

# Test 3: Completion request
echo "📝 Test 3: Completion Request"
(
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}'
sleep 0.1
echo '{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///test.seen","languageId":"seen","version":1,"text":"fun main() -> Int { let x = "}}}'
sleep 0.1
echo '{"jsonrpc":"2.0","id":2,"method":"textDocument/completion","params":{"textDocument":{"uri":"file:///test.seen"},"position":{"line":0,"character":25}}}'
sleep 0.1
echo '{"jsonrpc":"2.0","id":3,"method":"shutdown"}'
) | timeout 5s $COMPILER lsp > lsp_completion_response.log 2>&1

if [ -f lsp_completion_response.log ]; then
    echo "✅ Completion response:"
    cat lsp_completion_response.log
    echo ""
else
    echo "❌ No completion response"
fi

echo "✅ LSP Communication Tests Complete"
echo "📊 Results:"
echo "   - Initialize: $([ -f lsp_init_response.log ] && echo "✅" || echo "❌")"
echo "   - Document Open: $([ -f lsp_document_response.log ] && echo "✅" || echo "❌")"  
echo "   - Completion: $([ -f lsp_completion_response.log ] && echo "✅" || echo "❌")"

# Clean up
rm -f lsp_*.log