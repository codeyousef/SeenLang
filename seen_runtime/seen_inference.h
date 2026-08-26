#ifndef SEEN_INFERENCE_H
#define SEEN_INFERENCE_H

#include <stddef.h>
#include <stdint.h>

typedef struct SeenFloat16 { uint16_t bits; } SeenFloat16;
typedef struct SeenBFloat16 { uint16_t bits; } SeenBFloat16;
typedef struct SeenFloat8E4M3 { uint8_t bits; } SeenFloat8E4M3;
typedef struct SeenFloat8E5M2 { uint8_t bits; } SeenFloat8E5M2;
typedef struct SeenPackedQ4Block64 {
    const SeenFloat16 *scales;
    const uint8_t *values;
} SeenPackedQ4Block64;

typedef struct SeenSpan {
    const void *data;
    uint64_t length;
} SeenSpan;

typedef struct SeenMutableSpan {
    void *data;
    uint64_t length;
} SeenMutableSpan;

typedef struct SeenPackedView {
    const uint8_t *data;
    uint64_t element_count;
    uint64_t byte_count;
    uint8_t bits_per_element;
} SeenPackedView;

_Static_assert(sizeof(SeenFloat16) == 2, "SeenFloat16 ABI");
_Static_assert(sizeof(SeenBFloat16) == 2, "SeenBFloat16 ABI");
_Static_assert(sizeof(SeenFloat8E4M3) == 1, "SeenFloat8E4M3 ABI");
_Static_assert(sizeof(SeenFloat8E5M2) == 1, "SeenFloat8E5M2 ABI");
_Static_assert(offsetof(SeenSpan, length) == sizeof(void *), "SeenSpan ABI");
_Static_assert(offsetof(SeenMutableSpan, length) == sizeof(void *),
               "SeenMutableSpan ABI");

#endif
