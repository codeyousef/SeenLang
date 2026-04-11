// Seen TEE - Intel SGX Implementation
// Provides SGX-specific enclave operations, sealing, and attestation
// Part of UWW Infrastructure (Task 5.3)
//
// Build with: -DSEEN_TEE_ENABLE_SGX -lsgx_urts -lsgx_uae_service

#include "seen_tee.h"

#ifdef SEEN_TEE_ENABLE_SGX

#include <sgx_urts.h>
#include <sgx_uae_service.h>
#include <sgx_tcrypto.h>
#include <sgx_tseal.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

// ============================================================================
// SGX State
// ============================================================================

static sgx_enclave_id_t g_sgx_enclave_id = 0;
static int g_sgx_initialized = 0;
static int g_sgx_in_enclave = 0;

// Maximum number of enclaves we can track
#define MAX_SGX_ENCLAVES 16
static sgx_enclave_id_t g_sgx_enclaves[MAX_SGX_ENCLAVES];
static int g_sgx_enclave_count = 0;

// ============================================================================
// SGX Availability Check
// ============================================================================

int __seen_sgx_available(void) {
    // Check if SGX is supported
    sgx_device_status_t status;
    sgx_status_t ret = sgx_cap_get_status(&status);

    if (ret != SGX_SUCCESS) {
        return 0;
    }

    return (status == SGX_ENABLED);
}

// ============================================================================
// Initialization
// ============================================================================

SeenTEEStatus __seen_sgx_init(void) {
    if (g_sgx_initialized) {
        return SEEN_TEE_SUCCESS;
    }

    if (!__seen_sgx_available()) {
        return SEEN_TEE_ERR_NOT_SUPPORTED;
    }

    g_sgx_initialized = 1;
    return SEEN_TEE_SUCCESS;
}

// ============================================================================
// Enclave Management
// ============================================================================

int64_t __seen_sgx_enclave_create(const char* enclave_path, size_t heap_size, size_t stack_size) {
    if (!g_sgx_initialized) {
        if (__seen_sgx_init() != SEEN_TEE_SUCCESS) {
            return -1;
        }
    }

    if (g_sgx_enclave_count >= MAX_SGX_ENCLAVES) {
        fprintf(stderr, "SGX: Maximum number of enclaves reached\n");
        return -1;
    }

    sgx_enclave_id_t eid;
    sgx_launch_token_t token = {0};
    int updated = 0;

    // Create enclave with specified heap and stack sizes
    // Note: heap_size and stack_size are configured in the enclave signing config,
    // not directly in sgx_create_enclave. We ignore them here but could
    // verify they match the enclave config.
    (void)heap_size;
    (void)stack_size;

    sgx_status_t ret = sgx_create_enclave(
        enclave_path,       // Signed enclave .so file
        SGX_DEBUG_FLAG,     // Debug mode (change for production)
        &token,
        &updated,
        &eid,
        NULL                // No misc attributes
    );

    if (ret != SGX_SUCCESS) {
        fprintf(stderr, "SGX: Failed to create enclave: 0x%x\n", ret);
        return -1;
    }

    g_sgx_enclaves[g_sgx_enclave_count++] = eid;
    return (int64_t)eid;
}

SeenTEEStatus __seen_sgx_enclave_destroy(int64_t enclave_id) {
    sgx_enclave_id_t eid = (sgx_enclave_id_t)enclave_id;

    sgx_status_t ret = sgx_destroy_enclave(eid);
    if (ret != SGX_SUCCESS) {
        return SEEN_TEE_ERR_ENCLAVE_EXIT;
    }

    // Remove from tracking
    for (int i = 0; i < g_sgx_enclave_count; i++) {
        if (g_sgx_enclaves[i] == eid) {
            for (int j = i; j < g_sgx_enclave_count - 1; j++) {
                g_sgx_enclaves[j] = g_sgx_enclaves[j + 1];
            }
            g_sgx_enclave_count--;
            break;
        }
    }

    if (g_sgx_enclave_id == eid) {
        g_sgx_enclave_id = 0;
        g_sgx_in_enclave = 0;
    }

    return SEEN_TEE_SUCCESS;
}

SeenTEEStatus __seen_sgx_enclave_enter(int64_t enclave_id) {
    // Note: SGX enclave entry is done via ECALL, not a separate function.
    // This sets up state for tracking.
    g_sgx_enclave_id = (sgx_enclave_id_t)enclave_id;
    g_sgx_in_enclave = 1;
    return SEEN_TEE_SUCCESS;
}

SeenTEEStatus __seen_sgx_enclave_exit(void) {
    g_sgx_in_enclave = 0;
    return SEEN_TEE_SUCCESS;
}

// ============================================================================
// SGX Sealing
// ============================================================================

SeenTEEStatus __seen_sgx_seal_data(
    const uint8_t* plaintext,
    size_t plaintext_size,
    const uint8_t* additional_data,
    size_t additional_size,
    SeenSealPolicy policy,
    SeenSealedData* sealed_output
) {
    (void)plaintext;
    (void)plaintext_size;
    (void)additional_data;
    (void)additional_size;
    (void)policy;
    (void)sealed_output;
    return SEEN_TEE_ERR_NOT_SUPPORTED;
}

SeenTEEStatus __seen_sgx_unseal_data(
    const SeenSealedData* sealed_input,
    uint8_t* plaintext_output,
    size_t* plaintext_size
) {
    (void)sealed_input;
    (void)plaintext_output;
    (void)plaintext_size;
    return SEEN_TEE_ERR_NOT_SUPPORTED;
}

// ============================================================================
// SGX Attestation
// ============================================================================

SeenTEEStatus __seen_sgx_get_report(
    const uint8_t* report_data,
    size_t report_data_size,
    SeenAttestationType attest_type,
    SeenAttestationReport* report_output
) {
    (void)report_data;
    (void)report_data_size;
    (void)attest_type;
    (void)report_output;
    return SEEN_TEE_ERR_NOT_SUPPORTED;
}

SeenTEEStatus __seen_sgx_verify_report(
    const SeenAttestationReport* report,
    const uint8_t* expected_measurement,
    size_t measurement_size
) {
    (void)report;
    (void)expected_measurement;
    (void)measurement_size;
    return SEEN_TEE_ERR_NOT_SUPPORTED;
}

// ============================================================================
// SGX Key Derivation
// ============================================================================

SeenTEEStatus __seen_sgx_derive_key(
    const uint8_t* key_id,
    size_t key_id_size,
    uint8_t* key_output
) {
    (void)key_id;
    (void)key_id_size;
    (void)key_output;
    return SEEN_TEE_ERR_NOT_SUPPORTED;
}

#else // SEEN_TEE_ENABLE_SGX not defined

// Stub implementations when SGX is not enabled
int __seen_sgx_available(void) { return 0; }
SeenTEEStatus __seen_sgx_init(void) { return SEEN_TEE_ERR_NOT_SUPPORTED; }
int64_t __seen_sgx_enclave_create(const char* enclave_path, size_t heap_size, size_t stack_size) { (void)enclave_path; (void)heap_size; (void)stack_size; return -1; }
SeenTEEStatus __seen_sgx_enclave_destroy(int64_t enclave_id) { (void)enclave_id; return SEEN_TEE_ERR_NOT_SUPPORTED; }
SeenTEEStatus __seen_sgx_enclave_enter(int64_t enclave_id) { (void)enclave_id; return SEEN_TEE_ERR_NOT_SUPPORTED; }
SeenTEEStatus __seen_sgx_enclave_exit(void) { return SEEN_TEE_ERR_NOT_SUPPORTED; }
SeenTEEStatus __seen_sgx_seal_data(const uint8_t* plaintext, size_t plaintext_size, const uint8_t* additional_data, size_t additional_size, SeenSealPolicy policy, SeenSealedData* sealed_output) { (void)plaintext; (void)plaintext_size; (void)additional_data; (void)additional_size; (void)policy; (void)sealed_output; return SEEN_TEE_ERR_NOT_SUPPORTED; }
SeenTEEStatus __seen_sgx_unseal_data(const SeenSealedData* sealed_input, uint8_t* plaintext_output, size_t* plaintext_size) { (void)sealed_input; (void)plaintext_output; (void)plaintext_size; return SEEN_TEE_ERR_NOT_SUPPORTED; }
SeenTEEStatus __seen_sgx_get_report(const uint8_t* report_data, size_t report_data_size, SeenAttestationType attest_type, SeenAttestationReport* report_output) { (void)report_data; (void)report_data_size; (void)attest_type; (void)report_output; return SEEN_TEE_ERR_NOT_SUPPORTED; }
SeenTEEStatus __seen_sgx_verify_report(const SeenAttestationReport* report, const uint8_t* expected_measurement, size_t measurement_size) { (void)report; (void)expected_measurement; (void)measurement_size; return SEEN_TEE_ERR_NOT_SUPPORTED; }
SeenTEEStatus __seen_sgx_derive_key(const uint8_t* key_id, size_t key_id_size, uint8_t* key_output) { (void)key_id; (void)key_id_size; (void)key_output; return SEEN_TEE_ERR_NOT_SUPPORTED; }

#endif // SEEN_TEE_ENABLE_SGX
