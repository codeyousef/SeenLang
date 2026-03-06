// Seen Identity Protection Runtime
// Generational handle masking with XOR region-specific secrets
// Part of UWW Infrastructure (Task 5.2)
//
// This prevents memory probing attacks by masking handle generation
// counters with region-specific cryptographic secrets.

#ifndef SEEN_IDENTITY_H
#define SEEN_IDENTITY_H

#include <stdint.h>
#include <stddef.h>

// Forward declarations
struct SeenRegion;
struct SeenRegionHandle;

// ============================================================================
// Identity Protection Configuration
// ============================================================================

// Maximum number of regions with identity secrets
#define SEEN_MAX_IDENTITY_REGIONS 256

// Secret rotation interval (in allocations)
#define SEEN_SECRET_ROTATION_INTERVAL 1000000

// Identity protection modes
typedef enum SeenIdentityMode {
    SEEN_IDENTITY_DISABLED = 0,    // No masking (fastest, for debugging)
    SEEN_IDENTITY_SIMPLE = 1,      // Simple XOR masking
    SEEN_IDENTITY_ROTATING = 2,    // XOR with periodic rotation
    SEEN_IDENTITY_DETERMINISTIC = 3 // Deterministic mode for bootstrap
} SeenIdentityMode;

// ============================================================================
// Region Secret Structure
// ============================================================================

// Cryptographic secret for a region
typedef struct SeenRegionSecret {
    int64_t region_id;          // Region this secret belongs to
    uint64_t secret;            // Current 64-bit XOR secret
    uint64_t previous_secret;   // Previous secret (for rotation grace period)
    int rotation_epoch;         // Current rotation epoch
    int allocations_since_rotation;  // Counter for rotation timing
    int initialized;            // Whether secret has been initialized
} SeenRegionSecret;

// ============================================================================
// Masked Handle Structure
// ============================================================================

// A handle with masked generation (for external use)
typedef struct SeenMaskedHandle {
    int64_t region_id;          // Region this handle belongs to
    int64_t masked_generation;  // Generation XOR'd with secret
    int64_t slot_index;         // Slot index in handle table
    int rotation_epoch;         // Epoch when handle was created
} SeenMaskedHandle;

// ============================================================================
// Identity Protection Functions
// ============================================================================

// Initialize identity protection subsystem
void __seen_identity_init(void);

// Set identity protection mode
void __seen_identity_set_mode(SeenIdentityMode mode);

// Get current identity protection mode
SeenIdentityMode __seen_identity_get_mode(void);

// ============================================================================
// Secret Management
// ============================================================================

// Generate a new cryptographic secret for a region
// Uses hardware RNG if available (RDRAND), falls back to PRNG
uint64_t __seen_identity_generate_secret(int64_t region_id);

// Get the current secret for a region
uint64_t __seen_identity_get_secret(int64_t region_id);

// Get the previous secret for a region (during rotation grace period)
uint64_t __seen_identity_get_previous_secret(int64_t region_id);

// Register a new region for identity protection
int __seen_identity_register_region(int64_t region_id);

// Unregister a region (when destroyed)
void __seen_identity_unregister_region(int64_t region_id);

// Rotate all region secrets
// Should be called periodically for enhanced security
void __seen_identity_rotate_secrets(void);

// Force rotation of a specific region's secret
void __seen_identity_rotate_region_secret(int64_t region_id);

// ============================================================================
// Handle Masking/Unmasking
// ============================================================================

// Mask a handle's generation with the region secret
// Called when creating or returning a handle to external code
SeenMaskedHandle __seen_identity_mask_handle(struct SeenRegionHandle* handle);

// Unmask a handle's generation to get the real value
// Called when dereferencing a handle
struct SeenRegionHandle __seen_identity_unmask_handle(SeenMaskedHandle masked);

// Validate a masked handle (check if generation is valid after unmasking)
// Returns 1 if valid, 0 if invalid
int __seen_identity_validate_handle(SeenMaskedHandle masked);

// ============================================================================
// Deterministic Mode (for Bootstrap)
// ============================================================================

// Enable deterministic mode with a fixed seed
// Used during compiler bootstrap to ensure reproducible builds
void __seen_identity_set_deterministic_seed(uint64_t seed);

// Check if deterministic mode is enabled
int __seen_identity_is_deterministic(void);

// ============================================================================
// Statistics and Diagnostics
// ============================================================================

// Get number of registered regions
int __seen_identity_get_region_count(void);

// Get total number of secret rotations performed
int64_t __seen_identity_get_rotation_count(void);

// Get number of handle validations performed
int64_t __seen_identity_get_validation_count(void);

// Get number of invalid handle detections
int64_t __seen_identity_get_invalid_count(void);

// Print identity protection statistics
void __seen_identity_print_stats(void);

// ============================================================================
// Integration with Region Runtime
// ============================================================================

// These functions integrate with seen_region.c to provide automatic masking

// Allocate with masked handle (wrapper for __seen_region_alloc_handle)
SeenMaskedHandle __seen_region_alloc_masked_handle(struct SeenRegion* region, int64_t size);

// Dereference masked handle (wrapper for __seen_region_deref_handle)
void* __seen_region_deref_masked_handle(SeenMaskedHandle masked);

// Check if masked handle is valid
int __seen_region_masked_handle_valid(SeenMaskedHandle masked);

#endif // SEEN_IDENTITY_H
