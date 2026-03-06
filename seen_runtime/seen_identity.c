// Seen Identity Protection Runtime Implementation
// Generational handle masking with XOR region-specific secrets
// Part of UWW Infrastructure (Task 5.2)

#include "seen_identity.h"
#include "seen_region.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <time.h>

// Platform-specific includes for hardware RNG
#ifdef __x86_64__
#include <cpuid.h>
#endif

// ============================================================================
// Global State
// ============================================================================

// Region secrets table
static SeenRegionSecret g_region_secrets[SEEN_MAX_IDENTITY_REGIONS];
static int g_secrets_initialized = 0;

// Current identity mode
static SeenIdentityMode g_identity_mode = SEEN_IDENTITY_SIMPLE;

// Deterministic mode state
static int g_deterministic_mode = 0;
static uint64_t g_deterministic_seed = 0;
static uint64_t g_deterministic_counter = 0;

// Statistics
static int64_t g_total_rotations = 0;
static int64_t g_validation_count = 0;
static int64_t g_invalid_count = 0;

// ============================================================================
// Hardware RNG Support
// ============================================================================

#ifdef __x86_64__
// Check if RDRAND is available
static int has_rdrand(void) {
    unsigned int eax, ebx, ecx, edx;
    if (__get_cpuid(1, &eax, &ebx, &ecx, &edx)) {
        return (ecx & (1 << 30)) != 0;  // RDRAND bit
    }
    return 0;
}

// Generate random number using RDRAND
static int rdrand64(uint64_t* value) {
    unsigned char ok;
    __asm__ volatile (
        "rdrand %0; setc %1"
        : "=r" (*value), "=qm" (ok)
    );
    return ok;
}
#else
static int has_rdrand(void) { return 0; }
static int rdrand64(uint64_t* value) { (void)value; return 0; }
#endif

// ============================================================================
// PRNG Fallback
// ============================================================================

// Simple xorshift64 PRNG for fallback
static uint64_t xorshift64_state = 0;

static uint64_t xorshift64(void) {
    uint64_t x = xorshift64_state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    xorshift64_state = x;
    return x;
}

static void seed_prng(uint64_t seed) {
    if (seed == 0) {
        // Use time + address for seeding if no seed provided
        seed = (uint64_t)time(NULL) ^ (uint64_t)(uintptr_t)&g_region_secrets;
    }
    xorshift64_state = seed;
    // Warm up the PRNG
    for (int i = 0; i < 10; i++) {
        xorshift64();
    }
}

// ============================================================================
// Initialization
// ============================================================================

void __seen_identity_init(void) {
    if (g_secrets_initialized) return;
    g_secrets_initialized = 1;

    // Clear secrets table
    memset(g_region_secrets, 0, sizeof(g_region_secrets));

    // Seed PRNG
    seed_prng(0);

    // Default mode
    g_identity_mode = SEEN_IDENTITY_SIMPLE;
}

void __seen_identity_set_mode(SeenIdentityMode mode) {
    g_identity_mode = mode;
}

SeenIdentityMode __seen_identity_get_mode(void) {
    return g_identity_mode;
}

// ============================================================================
// Secret Management
// ============================================================================

uint64_t __seen_identity_generate_secret(int64_t region_id) {
    __seen_identity_init();

    // Deterministic mode uses fixed sequence
    if (g_deterministic_mode) {
        g_deterministic_counter++;
        // Mix region_id into deterministic value
        uint64_t val = g_deterministic_seed ^ (uint64_t)region_id;
        val = val * 6364136223846793005ULL + g_deterministic_counter;
        return val;
    }

    // Try hardware RNG first
    uint64_t secret = 0;
    if (has_rdrand() && rdrand64(&secret)) {
        // Mix in region_id for additional entropy
        secret ^= (uint64_t)region_id * 0x9E3779B97F4A7C15ULL;
        return secret;
    }

    // Fallback to PRNG
    secret = xorshift64();
    secret ^= (uint64_t)region_id * 0x9E3779B97F4A7C15ULL;
    return secret;
}

// Find slot for region_id
static SeenRegionSecret* find_region_secret(int64_t region_id) {
    for (int i = 0; i < SEEN_MAX_IDENTITY_REGIONS; i++) {
        if (g_region_secrets[i].initialized && g_region_secrets[i].region_id == region_id) {
            return &g_region_secrets[i];
        }
    }
    return NULL;
}

// Find free slot
static SeenRegionSecret* find_free_slot(void) {
    for (int i = 0; i < SEEN_MAX_IDENTITY_REGIONS; i++) {
        if (!g_region_secrets[i].initialized) {
            return &g_region_secrets[i];
        }
    }
    return NULL;
}

uint64_t __seen_identity_get_secret(int64_t region_id) {
    __seen_identity_init();

    if (g_identity_mode == SEEN_IDENTITY_DISABLED) {
        return 0;  // No masking
    }

    SeenRegionSecret* secret = find_region_secret(region_id);
    if (secret) {
        return secret->secret;
    }

    // Auto-register if not found
    if (__seen_identity_register_region(region_id)) {
        secret = find_region_secret(region_id);
        if (secret) {
            return secret->secret;
        }
    }

    return 0;  // Fallback
}

uint64_t __seen_identity_get_previous_secret(int64_t region_id) {
    SeenRegionSecret* secret = find_region_secret(region_id);
    if (secret) {
        return secret->previous_secret;
    }
    return 0;
}

int __seen_identity_register_region(int64_t region_id) {
    __seen_identity_init();

    // Check if already registered
    if (find_region_secret(region_id) != NULL) {
        return 1;  // Already registered
    }

    // Find free slot
    SeenRegionSecret* slot = find_free_slot();
    if (!slot) {
        fprintf(stderr, "WARNING: Identity protection: max regions reached\n");
        return 0;
    }

    // Initialize slot
    slot->region_id = region_id;
    slot->secret = __seen_identity_generate_secret(region_id);
    slot->previous_secret = slot->secret;  // Initially same
    slot->rotation_epoch = 0;
    slot->allocations_since_rotation = 0;
    slot->initialized = 1;

    return 1;
}

void __seen_identity_unregister_region(int64_t region_id) {
    SeenRegionSecret* secret = find_region_secret(region_id);
    if (secret) {
        memset(secret, 0, sizeof(*secret));
    }
}

void __seen_identity_rotate_secrets(void) {
    __seen_identity_init();

    for (int i = 0; i < SEEN_MAX_IDENTITY_REGIONS; i++) {
        if (g_region_secrets[i].initialized) {
            __seen_identity_rotate_region_secret(g_region_secrets[i].region_id);
        }
    }
}

void __seen_identity_rotate_region_secret(int64_t region_id) {
    SeenRegionSecret* secret = find_region_secret(region_id);
    if (!secret) return;

    // Save old secret for grace period
    secret->previous_secret = secret->secret;

    // Generate new secret
    secret->secret = __seen_identity_generate_secret(region_id);
    secret->rotation_epoch++;
    secret->allocations_since_rotation = 0;

    g_total_rotations++;
}

// ============================================================================
// Handle Masking/Unmasking
// ============================================================================

SeenMaskedHandle __seen_identity_mask_handle(SeenRegionHandle* handle) {
    SeenMaskedHandle masked;
    masked.region_id = handle->region_id;
    masked.slot_index = handle->slot_index;

    if (g_identity_mode == SEEN_IDENTITY_DISABLED) {
        masked.masked_generation = handle->generation;
        masked.rotation_epoch = 0;
    } else {
        uint64_t secret = __seen_identity_get_secret(handle->region_id);
        masked.masked_generation = handle->generation ^ (int64_t)secret;

        SeenRegionSecret* rs = find_region_secret(handle->region_id);
        masked.rotation_epoch = rs ? rs->rotation_epoch : 0;

        // Check if rotation is needed
        if (g_identity_mode == SEEN_IDENTITY_ROTATING && rs) {
            rs->allocations_since_rotation++;
            if (rs->allocations_since_rotation >= SEEN_SECRET_ROTATION_INTERVAL) {
                __seen_identity_rotate_region_secret(handle->region_id);
            }
        }
    }

    return masked;
}

SeenRegionHandle __seen_identity_unmask_handle(SeenMaskedHandle masked) {
    SeenRegionHandle handle;
    handle.region_id = masked.region_id;
    handle.slot_index = masked.slot_index;

    if (g_identity_mode == SEEN_IDENTITY_DISABLED) {
        handle.generation = masked.masked_generation;
    } else {
        uint64_t secret = __seen_identity_get_secret(masked.region_id);
        handle.generation = masked.masked_generation ^ (int64_t)secret;

        // If generation doesn't validate, try previous secret (rotation grace period)
        SeenRegionSecret* rs = find_region_secret(masked.region_id);
        if (rs && rs->rotation_epoch != masked.rotation_epoch) {
            // Try with previous secret
            int64_t alt_generation = masked.masked_generation ^ (int64_t)rs->previous_secret;
            // The caller will validate which one is correct
            handle.generation = alt_generation;
        }
    }

    return handle;
}

int __seen_identity_validate_handle(SeenMaskedHandle masked) {
    g_validation_count++;

    SeenRegionHandle unmasked = __seen_identity_unmask_handle(masked);

    // Use the region runtime's validation
    int valid = __seen_region_handle_valid(unmasked);

    if (!valid) {
        // Try with previous secret if in rotation grace period
        SeenRegionSecret* rs = find_region_secret(masked.region_id);
        if (rs && g_identity_mode != SEEN_IDENTITY_DISABLED) {
            SeenRegionHandle alt;
            alt.region_id = masked.region_id;
            alt.slot_index = masked.slot_index;
            alt.generation = masked.masked_generation ^ (int64_t)rs->previous_secret;
            valid = __seen_region_handle_valid(alt);
        }
    }

    if (!valid) {
        g_invalid_count++;
    }

    return valid;
}

// ============================================================================
// Deterministic Mode
// ============================================================================

void __seen_identity_set_deterministic_seed(uint64_t seed) {
    g_deterministic_mode = 1;
    g_deterministic_seed = seed;
    g_deterministic_counter = 0;

    // Re-initialize all existing secrets with deterministic values
    for (int i = 0; i < SEEN_MAX_IDENTITY_REGIONS; i++) {
        if (g_region_secrets[i].initialized) {
            g_region_secrets[i].secret = __seen_identity_generate_secret(
                g_region_secrets[i].region_id
            );
            g_region_secrets[i].previous_secret = g_region_secrets[i].secret;
        }
    }
}

int __seen_identity_is_deterministic(void) {
    return g_deterministic_mode;
}

// ============================================================================
// Statistics
// ============================================================================

int __seen_identity_get_region_count(void) {
    int count = 0;
    for (int i = 0; i < SEEN_MAX_IDENTITY_REGIONS; i++) {
        if (g_region_secrets[i].initialized) {
            count++;
        }
    }
    return count;
}

int64_t __seen_identity_get_rotation_count(void) {
    return g_total_rotations;
}

int64_t __seen_identity_get_validation_count(void) {
    return g_validation_count;
}

int64_t __seen_identity_get_invalid_count(void) {
    return g_invalid_count;
}

void __seen_identity_print_stats(void) {
    fprintf(stderr, "\n=== IDENTITY PROTECTION STATS ===\n");
    fprintf(stderr, "Mode: %s\n",
            g_identity_mode == SEEN_IDENTITY_DISABLED ? "Disabled" :
            g_identity_mode == SEEN_IDENTITY_SIMPLE ? "Simple XOR" :
            g_identity_mode == SEEN_IDENTITY_ROTATING ? "Rotating XOR" :
            g_identity_mode == SEEN_IDENTITY_DETERMINISTIC ? "Deterministic" : "Unknown");
    fprintf(stderr, "Deterministic Mode: %s\n", g_deterministic_mode ? "Yes" : "No");
    fprintf(stderr, "Registered Regions: %d\n", __seen_identity_get_region_count());
    fprintf(stderr, "Secret Rotations: %ld\n", (long)g_total_rotations);
    fprintf(stderr, "Handle Validations: %ld\n", (long)g_validation_count);
    fprintf(stderr, "Invalid Handles Detected: %ld\n", (long)g_invalid_count);
    fprintf(stderr, "Hardware RNG Available: %s\n", has_rdrand() ? "Yes" : "No");
    fprintf(stderr, "=================================\n\n");
}

// ============================================================================
// Integration with Region Runtime
// ============================================================================

SeenMaskedHandle __seen_region_alloc_masked_handle(SeenRegion* region, int64_t size) {
    SeenRegionHandle handle = __seen_region_alloc_handle(region, size);

    // Register region if needed
    __seen_identity_register_region(region->region_id);

    return __seen_identity_mask_handle(&handle);
}

void* __seen_region_deref_masked_handle(SeenMaskedHandle masked) {
    SeenRegionHandle handle = __seen_identity_unmask_handle(masked);
    return __seen_region_deref_handle(handle);
}

int __seen_region_masked_handle_valid(SeenMaskedHandle masked) {
    return __seen_identity_validate_handle(masked);
}
