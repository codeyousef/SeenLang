#include "iocp_abi.h"
#include <stddef.h>
#if defined(_WIN32) && defined(_WIN64)
#include <windows.h>
#include <limits.h>
_Static_assert(sizeof(OVERLAPPED) == 32, "Seen Windows x64 OVERLAPPED ABI");
_Static_assert(offsetof(OVERLAPPED, Offset) == 16, "Seen Windows positional offset ABI");
_Static_assert(offsetof(OVERLAPPED, hEvent) == 24, "Seen Windows event ABI");
uint32_t seen_iocp_create(uint32_t concurrency, int64_t *port) {
    if (!port || *port != -1) return ERROR_INVALID_PARAMETER;
    HANDLE value = CreateIoCompletionPort(INVALID_HANDLE_VALUE, NULL, 0, concurrency);
    if (!value) return GetLastError();
    *port = (int64_t)(intptr_t)value; return 0;
}
uint32_t seen_iocp_associate(int64_t port, int64_t file, uint64_t key) {
    HANDLE value = CreateIoCompletionPort((HANDLE)(intptr_t)file, (HANDLE)(intptr_t)port, (ULONG_PTR)key, 0);
    return value ? 0 : GetLastError();
}
int64_t seen_iocp_utf16(const uint8_t *input, uint32_t bytes, uint16_t *output, uint32_t capacity) {
    if (!input || !output || !bytes || bytes > INT_MAX || !capacity || capacity > INT_MAX)
        return -(int64_t)ERROR_INVALID_PARAMETER;
    int count = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, (const char *)input,
        (int)bytes, (WCHAR *)output, (int)capacity);
    return count ? (int64_t)count : -(int64_t)GetLastError();
}
uint32_t seen_iocp_open(const uint16_t *path, uint32_t access, uint32_t share,
    uint32_t creation, uint32_t flags, int64_t *file) {
    if (!path || !file || *file != -1 || !(flags & FILE_FLAG_OVERLAPPED)) return ERROR_INVALID_PARAMETER;
    HANDLE value = CreateFileW((const WCHAR *)path, access, share, NULL, creation, flags, NULL);
    if (value == INVALID_HANDLE_VALUE) return GetLastError();
    *file = (int64_t)(intptr_t)value; return 0;
}
uint32_t seen_iocp_transfer(int64_t file, void *buffer, uint32_t bytes,
    void *overlapped, uint64_t overlapped_bytes, uint32_t write) {
    if (!buffer || !overlapped || overlapped_bytes != sizeof(OVERLAPPED) ||
        (uintptr_t)overlapped % _Alignof(OVERLAPPED) || write > 1) return ERROR_INVALID_PARAMETER;
    BOOL result = write ? WriteFile((HANDLE)(intptr_t)file, buffer, bytes, NULL, (OVERLAPPED *)overlapped) :
        ReadFile((HANDLE)(intptr_t)file, buffer, bytes, NULL, (OVERLAPPED *)overlapped);
    return result ? 0 : GetLastError();
}
uint32_t seen_iocp_cancel(int64_t file, void *overlapped) {
    return CancelIoEx((HANDLE)(intptr_t)file, (OVERLAPPED *)overlapped) ? 0 : GetLastError();
}
uint32_t seen_iocp_dequeue(int64_t port, uint32_t *bytes, uint64_t *key, uint64_t *overlapped) {
    if (!bytes || !key || !overlapped) return ERROR_INVALID_PARAMETER;
    DWORD transferred = 0; ULONG_PTR identity = 0; OVERLAPPED *request = NULL;
    BOOL result = GetQueuedCompletionStatus((HANDLE)(intptr_t)port, &transferred, &identity, &request, 0);
    DWORD error = result ? 0 : GetLastError();
    *overlapped = (uint64_t)(uintptr_t)request;
    *bytes = request ? transferred : 0; *key = request ? (uint64_t)identity : 0;
    return error;
}
#else
uint32_t seen_iocp_create(uint32_t c,int64_t*p) {(void)c;(void)p;return 50;}
uint32_t seen_iocp_associate(int64_t p,int64_t f,uint64_t k) {(void)p;(void)f;(void)k;return 50;}
int64_t seen_iocp_utf16(const uint8_t*i,uint32_t b,uint16_t*o,uint32_t c) {(void)i;(void)b;(void)o;(void)c;return -50;}
uint32_t seen_iocp_open(const uint16_t*p,uint32_t a,uint32_t s,uint32_t c,uint32_t f,int64_t*v) {(void)p;(void)a;(void)s;(void)c;(void)f;(void)v;return 50;}
uint32_t seen_iocp_transfer(int64_t f,void*b,uint32_t n,void*o,uint64_t z,uint32_t w) {(void)f;(void)b;(void)n;(void)o;(void)z;(void)w;return 50;}
uint32_t seen_iocp_cancel(int64_t f,void*o) {(void)f;(void)o;return 50;}
uint32_t seen_iocp_dequeue(int64_t p,uint32_t*b,uint64_t*k,uint64_t*o) {(void)p;(void)b;(void)k;(void)o;return 50;}
#endif
