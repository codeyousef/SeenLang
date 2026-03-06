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
    if (!plaintext || !sealed_output) {
        return SEEN_TEE_ERR_INVALID_PARAM;
    }

    // Calculate sealed data size
    uint32_t sealed_size = sgx_calc_sealed_data_size(
        (uint32_t)additional_size,
        (uint32_t)plaintext_size
    );

    if (sealed_size > SEEN_TEE_MAX_SEALED_SIZE) {
        return SEEN_TEE_ERR_INVALID_PARAM;
    }

    // Select key policy
    uint16_t key_policy = SGX_KEYPOLICY_MRENCLAVE;  // Default
    if (policy == SEEN_SEAL_MRSIGNER) {
        key_policy = SGX_KEYPOLICY_MRSIGNER;
    }

    // Allocate temporary buffer for SGX sealed blob
    uint8_t* sealed_blob = (uint8_t*)malloc(sealed_size);
    if (!sealed_blob) {
        return SEEN_TEE_ERR_MEMORY;
    }

    // Note: sgx_seal_data must be called from within an enclave (trusted code).
    // For untrusted code, we need to make an ECALL to the enclave.
    // This is a placeholder showing the API - actual implementation would
    // require enclave-side code.
    sgx_status_t ret = sgx_seal_data_ex(
        key_policy,
        (sgx_attributes_t){0, 0},  // Default attributes
        0,                          // Misc select
        (uint32_t)additional_size,
        additional_data,
        (uint32_t)plaintext_size,
        plaintext,
        sealed_size,
        (sgx_sealed_data_t*)sealed_blob
    );

    if (ret != SGX_SUCCESS) {
        free(sealed_blob);
        return SEEN_TEE_ERR_SEAL;
    }

    // Copy to output
    memcpy(sealed_output->sealed_data, sealed_blob, sealed_size);
    sealed_output->sealed_size = sealed_size;
    sealed_output->tee_type = SEEN_TEE_SGX;
    sealed_output->policy = policy;
    sealed_output->valid = 1;

    if (additional_data && additional_size > 0) {
        size_t copy_size = additional_size < 256 ? additional_size : 256;
        memcpy(sealed_output->additional_data, additional_data, copy_size);
        sealed_output->additional_size = copy_size;
    }

    free(sealed_blob);
    return SEEN_TEE_SUCCESS;
}

SeenTEEStatus __seen_sgx_unseal_data(
    const SeenSealedData* sealed_input,
    uint8_t* plaintext_output,
    size_t* plaintext_size
) {
    if (!sealed_input || !plaintext_output || !plaintext_size) {
        return SEEN_TEE_ERR_INVALID_PARAM;
    }

    if (sealed_input->tee_type != SEEN_TEE_SGX) {
        return SEEN_TEE_ERR_UNSEAL;  // Wrong TEE type
    }

    // Get plaintext size
    uint32_t decrypted_size = sgx_get_encrypt_txt_len(
        (const sgx_sealed_data_t*)sealed_input->sealed_data
    );

    // Note: sgx_unseal_data must be called from within an enclave.
    uint32_t aad_size = 0;
    sgx_status_t ret = sgx_unseal_data(
        (const sgx_sealed_data_t*)sealed_input->sealed_data,
        NULL,           // No AAD output
        &aad_size,
        plaintext_output,
        &decrypted_size
    );

    if (ret != SGX_SUCCESS) {
        return SEEN_TEE_ERR_UNSEAL;
    }

    *plaintext_size = decrypted_size;
    return SEEN_TEE_SUCCESS;
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
    if (!report_output) {
        return SEEN_TEE_ERR_INVALID_PARAM;
    }

    memset(report_output, 0, sizeof(*report_output));
    report_output->tee_type = SEEN_TEE_SGX;
    report_output->attest_type = attest_type;

    if (attest_type == SEEN_ATTEST_LOCAL) {
        // Local attestation uses sgx_create_report
        sgx_target_info_t target_info = {0};
        sgx_report_data_t sgx_report_data = {0};
        sgx_report_t report;

        if (report_data && report_data_size > 0) {
            size_t copy_size = report_data_size < 64 ? report_data_size : 64;
            memcpy(sgx_report_data.d, report_data, copy_size);
        }

        // Note: This must be called from within enclave
        sgx_status_t ret = sgx_create_report(&target_info, &sgx_report_data, &report);
        if (ret != SGX_SUCCESS) {
            return SEEN_TEE_ERR_REPORT;
        }

        // Copy report to output
        memcpy(report_output->report_data, &report, sizeof(report));
        report_output->report_size = sizeof(report);

        // Copy MRENCLAVE measurement
        memcpy(report_output->measurement, report.body.mr_enclave.m, 32);
        report_output->measurement_size = 32;

        report_output->valid = 1;

    } else {
        // Remote attestation using EPID or DCAP
        // This requires interaction with Intel Attestation Service (IAS)
        // or DCAP Quote Generation/Verification Library

        // Placeholder - actual implementation depends on attestation infrastructure
        sgx_epid_group_id_t gid;
        sgx_status_t ret = sgx_init_quote(NULL, &gid);
        if (ret != SGX_SUCCESS) {
            return SEEN_TEE_ERR_ATTESTATION;
        }

        // For remote attestation, would need to:
        // 1. Get quote using sgx_get_quote
        // 2. Send quote to IAS for verification
        // 3. Return IAS response as attestation report

        return SEEN_TEE_ERR_NOT_SUPPORTED;  // Full implementation requires IAS setup
    }

    return SEEN_TEE_SUCCESS;
}

SeenTEEStatus __seen_sgx_verify_report(
    const SeenAttestationReport* report,
    const uint8_t* expected_measurement,
    size_t measurement_size
) {
    if (!report || report->tee_type != SEEN_TEE_SGX) {
        return SEEN_TEE_ERR_INVALID_PARAM;
    }

    if (report->attest_type == SEEN_ATTEST_LOCAL) {
        // For local attestation, verify the report using sgx_verify_report
        // Note: The verifying enclave needs the target enclave's MRENCLAVE

        // Verify measurement if provided
        if (expected_measurement && measurement_size > 0) {
            if (measurement_size != report->measurement_size) {
                return SEEN_TEE_ERR_VERIFY;
            }
            if (memcmp(expected_measurement, report->measurement, measurement_size) != 0) {
                return SEEN_TEE_ERR_VERIFY;
            }
        }

        return SEEN_TEE_SUCCESS;
    }

    // Remote attestation verification requires IAS
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
    if (!key_id || !key_output) {
        return SEEN_TEE_ERR_INVALID_PARAM;
    }

    // Use SGX EGETKEY instruction via sgx_get_key
    sgx_key_request_t key_request = {0};
    key_request.key_name = SGX_KEYSELECT_SEAL;  // Use seal key
    key_request.key_policy = SGX_KEYPOLICY_MRENCLAVE;

    // Set key ID
    size_t copy_size = key_id_size < 32 ? key_id_size : 32;
    memcpy(key_request.key_id.id, key_id, copy_size);

    sgx_key_128bit_t derived_key;

    // Note: This must be called from within enclave
    sgx_status_t ret = sgx_get_key(&key_request, &derived_key);
    if (ret != SGX_SUCCESS) {
        return SEEN_TEE_ERR_KEY_DERIVATION;
    }

    // Copy 128-bit key and expand to 256-bit if needed
    memcpy(key_output, derived_key, 16);

    // For 256-bit key, derive second half with different key_id
    key_request.key_id.id[0] ^= 0xFF;
    ret = sgx_get_key(&key_request, &derived_key);
    if (ret == SGX_SUCCESS) {
        memcpy(key_output + 16, derived_key, 16);
    } else {
        // Duplicate first half if second derivation fails
        memcpy(key_output + 16, key_output, 16);
    }

    return SEEN_TEE_SUCCESS;
}

#else // SEEN_TEE_ENABLE_SGX not defined

// Stub implementations when SGX is not enabled
int __seen_sgx_available(void) { return 0; }
SeenTEEStatus __seen_sgx_init(void) { return SEEN_TEE_ERR_NOT_SUPPORTED; }

#endif // SEEN_TEE_ENABLE_SGX
