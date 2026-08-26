#include <stdint.h>

typedef struct QwenAbiProbe {
    uint32_t tag;
    uint64_t bytes;
    const uint8_t *data;
} QwenAbiProbe;

uint64_t qwen_probe_fixed_width(uint32_t value, const QwenAbiProbe *probe) {
    if (!probe || probe->data != 0) return 0;
    return (uint64_t)value + (uint64_t)probe->tag + probe->bytes;
}
