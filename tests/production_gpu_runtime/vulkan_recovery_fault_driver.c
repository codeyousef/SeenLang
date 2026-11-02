/* Test-only recovery interposition; policy and ownership remain in Seen. */
#define SEEN_VULKAN_RECOVERY_FIXTURE 1
static int g_status(void);
static unsigned g_staging_memory_calls;
#include "vulkan_graph_fault_driver.c"
#include <errno.h>
#include <limits.h>

static int g_status(void) {
    const char *text = getenv("SEEN_VULKAN_FIXTURE_G_STATUS");
    if (!text || !*text) abort();
    errno = 0; char *end = NULL; long value = strtol(text, &end, 10);
    if (errno || !end || *end || value < INT32_MIN || value > INT32_MAX) abort();
    return (int)value;
}
