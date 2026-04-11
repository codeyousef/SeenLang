// Seen TEE Runtime - Trusted Execution Environment Support
// Provides enclave_call, seal_data, unseal_data intrinsics
// Supports Intel SGX and AMD SEV
// Part of UWW Infrastructure (Task 5.3)

#ifndef SEEN_TEE_H
#define SEEN_TEE_H

#include <stdint.h>
#include <stddef.h>

// ============================================================================
// TEE Type Enumeration
// ============================================================================

typedef enum SeenTEEType {
    SEEN_TEE_NONE = 0,      // No TEE available
    SEEN_TEE_SGX = 1,       // Intel SGX
    SEEN_TEE_SEV = 2,       // AMD SEV (including SEV-ES, SEV-SNP)
    SEEN_TEE_TDX = 3,       // Intel TDX (future)
    SEEN_TEE_STUB = 99      // Stub mode when explicitly enabled for development
} SeenTEEType;

// ============================================================================
// TEE Status Codes
// ============================================================================

typedef enum SeenTEEStatus {
    SEEN_TEE_SUCCESS = 0,
    SEEN_TEE_ERR_NOT_SUPPORTED = -1,    // TEE not available on this system
    SEEN_TEE_ERR_ENCLAVE_CREATE = -2,   // Failed to create enclave
    SEEN_TEE_ERR_ENCLAVE_ENTER = -3,    // Failed to enter enclave
    SEEN_TEE_ERR_ENCLAVE_EXIT = -4,     // Failed to exit enclave
    SEEN_TEE_ERR_SEAL = -5,             // Sealing operation failed
    SEEN_TEE_ERR_UNSEAL = -6,           // Unsealing operation failed
    SEEN_TEE_ERR_ATTESTATION = -7,      // Attestation failed
    SEEN_TEE_ERR_INVALID_PARAM = -8,    // Invalid parameter
    SEEN_TEE_ERR_MEMORY = -9,           // Memory allocation failed
    SEEN_TEE_ERR_KEY_DERIVATION = -10,  // Key derivation failed
    SEEN_TEE_ERR_REPORT = -11,          // Report generation failed
    SEEN_TEE_ERR_VERIFY = -12           // Verification failed
} SeenTEEStatus;

// ============================================================================
// Attestation Types
// ============================================================================

typedef enum SeenAttestationType {
    SEEN_ATTEST_LOCAL = 0,      // Local attestation (same platform)
    SEEN_ATTEST_REMOTE = 1      // Remote attestation (different platform)
} SeenAttestationType;

// ============================================================================
// Sealing Key Policy
// ============================================================================

typedef enum SeenSealPolicy {
    SEEN_SEAL_MRENCLAVE = 0,    // Seal to specific enclave measurement
    SEEN_SEAL_MRSIGNER = 1      // Seal to enclave signer (allows updates)
} SeenSealPolicy;

// ============================================================================
// TEE Configuration
// ============================================================================

#define SEEN_TEE_MAX_REPORT_SIZE 4096
#define SEEN_TEE_MAX_SEALED_SIZE (64 * 1024)  // 64KB max sealed data
#define SEEN_TEE_KEY_SIZE 32                   // 256-bit keys

// ============================================================================
// Attestation Report Structure (Common)
// ============================================================================

typedef struct SeenAttestationReport {
    SeenTEEType tee_type;           // Which TEE generated this report
    SeenAttestationType attest_type; // Local or remote
    uint8_t report_data[SEEN_TEE_MAX_REPORT_SIZE];
    size_t report_size;
    uint8_t measurement[64];         // Enclave/VM measurement
    size_t measurement_size;
    int64_t timestamp;               // Report generation time
    int valid;                       // Whether report is valid
} SeenAttestationReport;

// ============================================================================
// Sealed Data Structure
// ============================================================================

typedef struct SeenSealedData {
    SeenTEEType tee_type;           // Which TEE sealed this
    SeenSealPolicy policy;          // Sealing key policy
    uint8_t sealed_data[SEEN_TEE_MAX_SEALED_SIZE];
    size_t sealed_size;
    uint8_t additional_data[256];   // Additional authenticated data
    size_t additional_size;
    int valid;
} SeenSealedData;

// ============================================================================
// Core TEE Functions (Platform-agnostic interface)
// ============================================================================

// Initialize TEE subsystem
// Detects available TEE and initializes it
SeenTEEStatus __seen_tee_init(void);

// Check if TEE is available and what type
SeenTEEType __seen_tee_get_type(void);

// Check if TEE is currently active
int __seen_tee_is_active(void);

// Get TEE type name as string
const char* __seen_tee_type_name(SeenTEEType type);

// Get status code description
const char* __seen_tee_status_string(SeenTEEStatus status);

// ============================================================================
// Enclave Operations
// ============================================================================

// Enter enclave (start protected execution)
SeenTEEStatus __seen_enclave_enter(int64_t enclave_id);

// Exit enclave (return to untrusted mode)
SeenTEEStatus __seen_enclave_exit(void);

// Check if currently executing inside enclave
int __seen_in_enclave(void);

// Create a new enclave from binary
// Returns enclave_id on success, negative on error
int64_t __seen_enclave_create(const char* enclave_path, size_t heap_size, size_t stack_size);

// Destroy an enclave
SeenTEEStatus __seen_enclave_destroy(int64_t enclave_id);

// ============================================================================
// Data Sealing/Unsealing
// ============================================================================

// Seal plaintext data using TEE-derived keys
// Output written to sealed_output
SeenTEEStatus __seen_seal_data(
    const uint8_t* plaintext,
    size_t plaintext_size,
    const uint8_t* additional_data,  // Optional AAD
    size_t additional_size,
    SeenSealPolicy policy,
    SeenSealedData* sealed_output
);

// Unseal data previously sealed by this enclave/VM
SeenTEEStatus __seen_unseal_data(
    const SeenSealedData* sealed_input,
    uint8_t* plaintext_output,
    size_t* plaintext_size
);

// ============================================================================
// Attestation
// ============================================================================

// Generate attestation report
SeenTEEStatus __seen_get_attestation_report(
    const uint8_t* report_data,      // User-provided data to include
    size_t report_data_size,
    SeenAttestationType attest_type,
    SeenAttestationReport* report_output
);

// Verify attestation report
SeenTEEStatus __seen_verify_attestation_report(
    const SeenAttestationReport* report,
    const uint8_t* expected_measurement,  // Optional, NULL to skip
    size_t measurement_size
);

// ============================================================================
// Key Derivation
// ============================================================================

// Derive a key using TEE facilities
// key_output must be at least SEEN_TEE_KEY_SIZE bytes
SeenTEEStatus __seen_derive_key(
    const uint8_t* key_id,
    size_t key_id_size,
    uint8_t* key_output
);

// ============================================================================
// String-based API (for Seen language bindings)
// ============================================================================

// Seal data from Seen String, returns sealed data as hex string
char* __seen_seal_string(const char* plaintext, const char* additional_data, int policy);

// Unseal data from hex string, returns plaintext
char* __seen_unseal_string(const char* sealed_hex, const char* additional_data);

// Get attestation report as JSON string
char* __seen_get_attestation_json(const char* report_data, int attest_type);

// Verify attestation report from JSON, returns 1 if valid
int __seen_verify_attestation_json(const char* report_json, const char* expected_measurement);

// ============================================================================
// Diagnostics
// ============================================================================

// Print TEE status and capabilities
void __seen_tee_print_info(void);

// Get TEE capabilities as a bitmask
uint64_t __seen_tee_get_capabilities(void);

// Capability flags
#define SEEN_TEE_CAP_SEAL        (1 << 0)   // Data sealing supported
#define SEEN_TEE_CAP_ATTEST_LOCAL (1 << 1)  // Local attestation supported
#define SEEN_TEE_CAP_ATTEST_REMOTE (1 << 2) // Remote attestation supported
#define SEEN_TEE_CAP_DERIVE_KEY   (1 << 3)  // Key derivation supported
#define SEEN_TEE_CAP_ENCLAVE      (1 << 4)  // Enclave execution supported

#endif // SEEN_TEE_H
