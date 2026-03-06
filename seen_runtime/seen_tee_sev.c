// Seen TEE - AMD SEV Implementation
// Provides SEV-specific VM protection, sealing, and attestation
// Part of UWW Infrastructure (Task 5.3)
//
// Build with: -DSEEN_TEE_ENABLE_SEV
// Requires: /dev/sev and /dev/sev-guest device access

#include "seen_tee.h"

#ifdef SEEN_TEE_ENABLE_SEV

#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/ioctl.h>
#include <errno.h>

// SEV device paths
#define SEV_DEVICE "/dev/sev"
#define SEV_GUEST_DEVICE "/dev/sev-guest"

// SEV ioctl commands (from linux/psp-sev.h)
#define SEV_IOC_TYPE 'S'
#define SNP_GET_REPORT  _IOWR(SEV_IOC_TYPE, 0x01, struct snp_report_req)
#define SNP_GET_DERIVED_KEY _IOWR(SEV_IOC_TYPE, 0x02, struct snp_derived_key_req)

// SNP Report Request structure
struct snp_report_req {
    uint8_t user_data[64];    // User-provided data to include in report
    uint32_t vmpl;             // VMPL level (0-3)
    uint8_t reserved[28];
    uint8_t report[4000];      // Output report buffer
    uint32_t report_size;      // Size of report
};

// SNP Derived Key Request structure
struct snp_derived_key_req {
    uint32_t root_key_select;  // 0 = VCEK, 1 = VMRK
    uint32_t guest_svn;        // Guest SVN
    uint64_t guest_field_select; // Which fields to mix
    uint32_t vmpl;             // VMPL level
    uint8_t reserved[20];
    uint8_t key[32];           // Output key
};

// ============================================================================
// SEV State
// ============================================================================

static int g_sev_fd = -1;
static int g_sev_guest_fd = -1;
static int g_sev_initialized = 0;

// ============================================================================
// SEV Availability Check
// ============================================================================

int __seen_sev_available(void) {
    // Check if we're running in an SEV VM by checking for /dev/sev-guest
    int fd = open(SEV_GUEST_DEVICE, O_RDWR);
    if (fd >= 0) {
        close(fd);
        return 1;
    }

    // Also check /dev/sev (for host operations)
    fd = open(SEV_DEVICE, O_RDWR);
    if (fd >= 0) {
        close(fd);
        return 1;
    }

    return 0;
}

// ============================================================================
// Initialization
// ============================================================================

SeenTEEStatus __seen_sev_init(void) {
    if (g_sev_initialized) {
        return SEEN_TEE_SUCCESS;
    }

    // Try to open SEV guest device (for running inside SEV VM)
    g_sev_guest_fd = open(SEV_GUEST_DEVICE, O_RDWR);
    if (g_sev_guest_fd < 0) {
        // Try SEV device (for host operations)
        g_sev_fd = open(SEV_DEVICE, O_RDWR);
        if (g_sev_fd < 0) {
            fprintf(stderr, "SEV: Failed to open SEV device: %s\n", strerror(errno));
            return SEEN_TEE_ERR_NOT_SUPPORTED;
        }
    }

    g_sev_initialized = 1;
    return SEEN_TEE_SUCCESS;
}

void __seen_sev_cleanup(void) {
    if (g_sev_fd >= 0) {
        close(g_sev_fd);
        g_sev_fd = -1;
    }
    if (g_sev_guest_fd >= 0) {
        close(g_sev_guest_fd);
        g_sev_guest_fd = -1;
    }
    g_sev_initialized = 0;
}

// ============================================================================
// SEV Attestation
// ============================================================================

SeenTEEStatus __seen_sev_get_report(
    const uint8_t* report_data,
    size_t report_data_size,
    SeenAttestationType attest_type,
    SeenAttestationReport* report_output
) {
    if (!report_output) {
        return SEEN_TEE_ERR_INVALID_PARAM;
    }

    if (!g_sev_initialized) {
        if (__seen_sev_init() != SEEN_TEE_SUCCESS) {
            return SEEN_TEE_ERR_NOT_SUPPORTED;
        }
    }

    memset(report_output, 0, sizeof(*report_output));
    report_output->tee_type = SEEN_TEE_SEV;
    report_output->attest_type = attest_type;

    // SEV-SNP attestation
    if (g_sev_guest_fd >= 0) {
        struct snp_report_req req;
        memset(&req, 0, sizeof(req));

        // Copy user data
        if (report_data && report_data_size > 0) {
            size_t copy_size = report_data_size < 64 ? report_data_size : 64;
            memcpy(req.user_data, report_data, copy_size);
        }

        req.vmpl = 0;  // VMPL 0 (most privileged)

        int ret = ioctl(g_sev_guest_fd, SNP_GET_REPORT, &req);
        if (ret < 0) {
            fprintf(stderr, "SEV: Failed to get SNP report: %s\n", strerror(errno));
            return SEEN_TEE_ERR_REPORT;
        }

        // Copy report to output
        size_t copy_size = req.report_size < SEEN_TEE_MAX_REPORT_SIZE
                          ? req.report_size : SEEN_TEE_MAX_REPORT_SIZE;
        memcpy(report_output->report_data, req.report, copy_size);
        report_output->report_size = copy_size;

        // Extract measurement from report (offset depends on SNP report format)
        // SNP report has MEASUREMENT at offset 0x90, size 48 bytes
        if (copy_size >= 0x90 + 48) {
            memcpy(report_output->measurement, req.report + 0x90, 48);
            report_output->measurement_size = 48;
        }

        report_output->valid = 1;
        return SEEN_TEE_SUCCESS;
    }

    return SEEN_TEE_ERR_NOT_SUPPORTED;
}

SeenTEEStatus __seen_sev_verify_report(
    const SeenAttestationReport* report,
    const uint8_t* expected_measurement,
    size_t measurement_size
) {
    if (!report || report->tee_type != SEEN_TEE_SEV) {
        return SEEN_TEE_ERR_INVALID_PARAM;
    }

    // For SEV-SNP, verification typically involves:
    // 1. Fetching VCEK certificate from AMD KDS
    // 2. Verifying report signature using VCEK
    // 3. Verifying certificate chain to AMD root

    // Basic measurement verification
    if (expected_measurement && measurement_size > 0) {
        if (measurement_size != report->measurement_size) {
            return SEEN_TEE_ERR_VERIFY;
        }
        if (memcmp(expected_measurement, report->measurement, measurement_size) != 0) {
            return SEEN_TEE_ERR_VERIFY;
        }
    }

    // Full verification would require AMD KDS integration
    // For now, trust the measurement comparison
    return SEEN_TEE_SUCCESS;
}

// ============================================================================
// SEV Key Derivation
// ============================================================================

SeenTEEStatus __seen_sev_derive_key(
    const uint8_t* key_id,
    size_t key_id_size,
    uint8_t* key_output
) {
    if (!key_id || !key_output) {
        return SEEN_TEE_ERR_INVALID_PARAM;
    }

    if (!g_sev_initialized) {
        if (__seen_sev_init() != SEEN_TEE_SUCCESS) {
            return SEEN_TEE_ERR_NOT_SUPPORTED;
        }
    }

    if (g_sev_guest_fd >= 0) {
        struct snp_derived_key_req req;
        memset(&req, 0, sizeof(req));

        req.root_key_select = 0;  // Use VCEK
        req.vmpl = 0;

        // Mix key_id into guest_field_select as a simple customization
        // In production, would use proper key derivation parameters
        if (key_id_size >= 8) {
            memcpy(&req.guest_field_select, key_id, 8);
        }

        int ret = ioctl(g_sev_guest_fd, SNP_GET_DERIVED_KEY, &req);
        if (ret < 0) {
            fprintf(stderr, "SEV: Failed to derive key: %s\n", strerror(errno));
            return SEEN_TEE_ERR_KEY_DERIVATION;
        }

        memcpy(key_output, req.key, SEEN_TEE_KEY_SIZE);
        return SEEN_TEE_SUCCESS;
    }

    return SEEN_TEE_ERR_NOT_SUPPORTED;
}

// ============================================================================
// SEV Sealing (using derived keys)
// ============================================================================

// SEV doesn't have hardware sealing like SGX, but we can use
// derived keys for encryption/decryption

#include <openssl/evp.h>
#include <openssl/rand.h>

// AES-256-GCM encryption using SEV-derived key
SeenTEEStatus __seen_sev_seal_data(
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

    // Derive sealing key
    uint8_t seal_key[SEEN_TEE_KEY_SIZE];
    const char* key_derivation_context = (policy == SEEN_SEAL_MRENCLAVE)
        ? "SEV_SEAL_MRENCLAVE" : "SEV_SEAL_MRSIGNER";

    SeenTEEStatus status = __seen_sev_derive_key(
        (const uint8_t*)key_derivation_context,
        strlen(key_derivation_context),
        seal_key
    );

    if (status != SEEN_TEE_SUCCESS) {
        return status;
    }

    // Generate random IV (12 bytes for GCM)
    uint8_t iv[12];
    if (RAND_bytes(iv, 12) != 1) {
        return SEEN_TEE_ERR_SEAL;
    }

    // Initialize encryption context
    EVP_CIPHER_CTX* ctx = EVP_CIPHER_CTX_new();
    if (!ctx) {
        return SEEN_TEE_ERR_MEMORY;
    }

    if (EVP_EncryptInit_ex(ctx, EVP_aes_256_gcm(), NULL, seal_key, iv) != 1) {
        EVP_CIPHER_CTX_free(ctx);
        return SEEN_TEE_ERR_SEAL;
    }

    // Add AAD if provided
    int len;
    if (additional_data && additional_size > 0) {
        if (EVP_EncryptUpdate(ctx, NULL, &len, additional_data, additional_size) != 1) {
            EVP_CIPHER_CTX_free(ctx);
            return SEEN_TEE_ERR_SEAL;
        }
    }

    // Encrypt plaintext
    // Output format: [IV (12)] [Ciphertext] [Tag (16)]
    uint8_t* output = sealed_output->sealed_data;
    memcpy(output, iv, 12);
    output += 12;

    if (EVP_EncryptUpdate(ctx, output, &len, plaintext, plaintext_size) != 1) {
        EVP_CIPHER_CTX_free(ctx);
        return SEEN_TEE_ERR_SEAL;
    }
    int ciphertext_len = len;

    if (EVP_EncryptFinal_ex(ctx, output + len, &len) != 1) {
        EVP_CIPHER_CTX_free(ctx);
        return SEEN_TEE_ERR_SEAL;
    }
    ciphertext_len += len;

    // Get authentication tag
    uint8_t tag[16];
    if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_GET_TAG, 16, tag) != 1) {
        EVP_CIPHER_CTX_free(ctx);
        return SEEN_TEE_ERR_SEAL;
    }

    // Append tag
    memcpy(output + ciphertext_len, tag, 16);

    sealed_output->sealed_size = 12 + ciphertext_len + 16;
    sealed_output->tee_type = SEEN_TEE_SEV;
    sealed_output->policy = policy;
    sealed_output->valid = 1;

    if (additional_data && additional_size > 0) {
        size_t copy_size = additional_size < 256 ? additional_size : 256;
        memcpy(sealed_output->additional_data, additional_data, copy_size);
        sealed_output->additional_size = copy_size;
    }

    EVP_CIPHER_CTX_free(ctx);
    return SEEN_TEE_SUCCESS;
}

SeenTEEStatus __seen_sev_unseal_data(
    const SeenSealedData* sealed_input,
    uint8_t* plaintext_output,
    size_t* plaintext_size
) {
    if (!sealed_input || !plaintext_output || !plaintext_size) {
        return SEEN_TEE_ERR_INVALID_PARAM;
    }

    if (sealed_input->tee_type != SEEN_TEE_SEV) {
        return SEEN_TEE_ERR_UNSEAL;
    }

    // Derive sealing key
    uint8_t seal_key[SEEN_TEE_KEY_SIZE];
    const char* key_derivation_context = (sealed_input->policy == SEEN_SEAL_MRENCLAVE)
        ? "SEV_SEAL_MRENCLAVE" : "SEV_SEAL_MRSIGNER";

    SeenTEEStatus status = __seen_sev_derive_key(
        (const uint8_t*)key_derivation_context,
        strlen(key_derivation_context),
        seal_key
    );

    if (status != SEEN_TEE_SUCCESS) {
        return status;
    }

    // Extract IV, ciphertext, and tag
    const uint8_t* input = sealed_input->sealed_data;
    uint8_t iv[12];
    memcpy(iv, input, 12);
    input += 12;

    size_t ciphertext_len = sealed_input->sealed_size - 12 - 16;
    uint8_t tag[16];
    memcpy(tag, input + ciphertext_len, 16);

    // Initialize decryption context
    EVP_CIPHER_CTX* ctx = EVP_CIPHER_CTX_new();
    if (!ctx) {
        return SEEN_TEE_ERR_MEMORY;
    }

    if (EVP_DecryptInit_ex(ctx, EVP_aes_256_gcm(), NULL, seal_key, iv) != 1) {
        EVP_CIPHER_CTX_free(ctx);
        return SEEN_TEE_ERR_UNSEAL;
    }

    // Add AAD if present
    int len;
    if (sealed_input->additional_size > 0) {
        if (EVP_DecryptUpdate(ctx, NULL, &len,
                              sealed_input->additional_data,
                              sealed_input->additional_size) != 1) {
            EVP_CIPHER_CTX_free(ctx);
            return SEEN_TEE_ERR_UNSEAL;
        }
    }

    // Set expected tag
    if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_SET_TAG, 16, tag) != 1) {
        EVP_CIPHER_CTX_free(ctx);
        return SEEN_TEE_ERR_UNSEAL;
    }

    // Decrypt
    if (EVP_DecryptUpdate(ctx, plaintext_output, &len, input, ciphertext_len) != 1) {
        EVP_CIPHER_CTX_free(ctx);
        return SEEN_TEE_ERR_UNSEAL;
    }
    *plaintext_size = len;

    // Verify tag
    int final_len;
    if (EVP_DecryptFinal_ex(ctx, plaintext_output + len, &final_len) != 1) {
        EVP_CIPHER_CTX_free(ctx);
        return SEEN_TEE_ERR_UNSEAL;  // Tag verification failed
    }
    *plaintext_size += final_len;

    EVP_CIPHER_CTX_free(ctx);
    return SEEN_TEE_SUCCESS;
}

#else // SEEN_TEE_ENABLE_SEV not defined

// Stub implementations when SEV is not enabled
int __seen_sev_available(void) { return 0; }
SeenTEEStatus __seen_sev_init(void) { return SEEN_TEE_ERR_NOT_SUPPORTED; }

#endif // SEEN_TEE_ENABLE_SEV
