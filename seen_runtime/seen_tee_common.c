// Seen TEE Common Implementation
// Shared utilities and stub implementation for TEE operations
// Part of UWW Infrastructure (Task 5.3)

#include "seen_runtime.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <time.h>
#include <ctype.h>

// Forward declarations for platform-specific implementations
extern SeenTEEStatus __seen_sgx_init(void);
extern SeenTEEStatus __seen_sev_init(void);
extern int __seen_sgx_available(void);
extern int __seen_sev_available(void);
extern int64_t __seen_sgx_enclave_create(const char* enclave_path, size_t heap_size, size_t stack_size);
extern SeenTEEStatus __seen_sgx_enclave_destroy(int64_t enclave_id);
extern SeenTEEStatus __seen_sgx_enclave_enter(int64_t enclave_id);
extern SeenTEEStatus __seen_sgx_enclave_exit(void);
extern SeenTEEStatus __seen_sgx_seal_data(const uint8_t* plaintext, size_t plaintext_size, const uint8_t* additional_data, size_t additional_size, SeenSealPolicy policy, SeenSealedData* sealed_output);
extern SeenTEEStatus __seen_sgx_unseal_data(const SeenSealedData* sealed_input, uint8_t* plaintext_output, size_t* plaintext_size);
extern SeenTEEStatus __seen_sgx_get_report(const uint8_t* report_data, size_t report_data_size, SeenAttestationType attest_type, SeenAttestationReport* report_output);
extern SeenTEEStatus __seen_sgx_verify_report(const SeenAttestationReport* report, const uint8_t* expected_measurement, size_t measurement_size);
extern SeenTEEStatus __seen_sgx_derive_key(const uint8_t* key_id, size_t key_id_size, uint8_t* key_output);
extern SeenTEEStatus __seen_sev_get_report(const uint8_t* report_data, size_t report_data_size, SeenAttestationType attest_type, SeenAttestationReport* report_output);
extern SeenTEEStatus __seen_sev_verify_report(const SeenAttestationReport* report, const uint8_t* expected_measurement, size_t measurement_size);
extern SeenTEEStatus __seen_sev_derive_key(const uint8_t* key_id, size_t key_id_size, uint8_t* key_output);
extern SeenTEEStatus __seen_sev_seal_data(const uint8_t* plaintext, size_t plaintext_size, const uint8_t* additional_data, size_t additional_size, SeenSealPolicy policy, SeenSealedData* sealed_output);
extern SeenTEEStatus __seen_sev_unseal_data(const SeenSealedData* sealed_input, uint8_t* plaintext_output, size_t* plaintext_size);

// ============================================================================
// Global State
// ============================================================================

static SeenTEEType g_tee_type = SEEN_TEE_NONE;
static int g_tee_initialized = 0;
static int g_in_enclave = 0;
static int64_t g_current_enclave_id = -1;

// Stub mode state
static int g_stub_mode = 0;
static uint8_t g_stub_key[SEEN_TEE_KEY_SIZE] = {0};

static int seen_tee_stub_requested(void) {
    const char* env = getenv("SEEN_TEE_ALLOW_STUB");
    if (!env) {
        return 0;
    }
    if (strcmp(env, "1") == 0 || strcmp(env, "true") == 0 ||
        strcmp(env, "yes") == 0 || strcmp(env, "on") == 0) {
        return 1;
    }
    return 0;
}

// ============================================================================
// Initialization
// ============================================================================

SeenTEEStatus __seen_tee_init(void) {
    if (g_tee_initialized) {
        return g_tee_type == SEEN_TEE_NONE ? SEEN_TEE_ERR_NOT_SUPPORTED : SEEN_TEE_SUCCESS;
    }

    // Try to detect available TEE
    g_tee_type = SEEN_TEE_NONE;
    g_stub_mode = 0;

#ifdef SEEN_TEE_ENABLE_SGX
    if (__seen_sgx_available()) {
        SeenTEEStatus status = __seen_sgx_init();
        if (status == SEEN_TEE_SUCCESS) {
            g_tee_type = SEEN_TEE_SGX;
            g_tee_initialized = 1;
            return SEEN_TEE_SUCCESS;
        }
    }
#endif

#ifdef SEEN_TEE_ENABLE_SEV
    if (__seen_sev_available()) {
        SeenTEEStatus status = __seen_sev_init();
        if (status == SEEN_TEE_SUCCESS) {
            g_tee_type = SEEN_TEE_SEV;
            g_tee_initialized = 1;
            return SEEN_TEE_SUCCESS;
        }
    }
#endif

    g_tee_initialized = 1;
    if (seen_tee_stub_requested()) {
        g_tee_type = SEEN_TEE_STUB;
        g_stub_mode = 1;

        // Initialize stub key with random-ish data
        srand((unsigned int)time(NULL));
        for (int i = 0; i < SEEN_TEE_KEY_SIZE; i++) {
            g_stub_key[i] = (uint8_t)(rand() & 0xFF);
        }

        return SEEN_TEE_SUCCESS;
    }

    return SEEN_TEE_ERR_NOT_SUPPORTED;
}

SeenTEEType __seen_tee_get_type(void) {
    if (!g_tee_initialized) {
        __seen_tee_init();
    }
    return g_tee_type;
}

int __seen_tee_is_active(void) {
    return g_tee_initialized && g_tee_type != SEEN_TEE_NONE;
}

const char* __seen_tee_type_name(SeenTEEType type) {
    switch (type) {
        case SEEN_TEE_NONE: return "None";
        case SEEN_TEE_SGX:  return "Intel SGX";
        case SEEN_TEE_SEV:  return "AMD SEV";
        case SEEN_TEE_TDX:  return "Intel TDX";
        case SEEN_TEE_STUB: return "Stub (Development Opt-In)";
        default:            return "Unknown";
    }
}

const char* __seen_tee_status_string(SeenTEEStatus status) {
    switch (status) {
        case SEEN_TEE_SUCCESS:           return "Success";
        case SEEN_TEE_ERR_NOT_SUPPORTED: return "TEE not supported";
        case SEEN_TEE_ERR_ENCLAVE_CREATE: return "Enclave creation failed";
        case SEEN_TEE_ERR_ENCLAVE_ENTER: return "Enclave entry failed";
        case SEEN_TEE_ERR_ENCLAVE_EXIT:  return "Enclave exit failed";
        case SEEN_TEE_ERR_SEAL:          return "Sealing failed";
        case SEEN_TEE_ERR_UNSEAL:        return "Unsealing failed";
        case SEEN_TEE_ERR_ATTESTATION:   return "Attestation failed";
        case SEEN_TEE_ERR_INVALID_PARAM: return "Invalid parameter";
        case SEEN_TEE_ERR_MEMORY:        return "Memory allocation failed";
        case SEEN_TEE_ERR_KEY_DERIVATION: return "Key derivation failed";
        case SEEN_TEE_ERR_REPORT:        return "Report generation failed";
        case SEEN_TEE_ERR_VERIFY:        return "Verification failed";
        default:                         return "Unknown error";
    }
}

// ============================================================================
// Enclave Operations (Common/Stub Implementation)
// ============================================================================

int __seen_in_enclave(void) {
    return g_in_enclave;
}

SeenTEEStatus __seen_enclave_enter(int64_t enclave_id) {
    if (!g_tee_initialized) {
        __seen_tee_init();
    }

    if (g_in_enclave) {
        return SEEN_TEE_ERR_ENCLAVE_ENTER;  // Already in enclave
    }

    // Stub mode just sets the flag
    if (g_stub_mode) {
        g_in_enclave = 1;
        g_current_enclave_id = enclave_id;
        return SEEN_TEE_SUCCESS;
    }

    if (g_tee_type == SEEN_TEE_SGX) {
        SeenTEEStatus status = __seen_sgx_enclave_enter(enclave_id);
        if (status == SEEN_TEE_SUCCESS) {
            g_in_enclave = 1;
            g_current_enclave_id = enclave_id;
        }
        return status;
    }

    return SEEN_TEE_ERR_NOT_SUPPORTED;
}

SeenTEEStatus __seen_enclave_exit(void) {
    if (!g_in_enclave) {
        return SEEN_TEE_ERR_ENCLAVE_EXIT;  // Not in enclave
    }

    if (g_stub_mode) {
        g_in_enclave = 0;
        g_current_enclave_id = -1;
        return SEEN_TEE_SUCCESS;
    }

    if (g_tee_type == SEEN_TEE_SGX) {
        SeenTEEStatus status = __seen_sgx_enclave_exit();
        if (status == SEEN_TEE_SUCCESS) {
            g_in_enclave = 0;
            g_current_enclave_id = -1;
        }
        return status;
    }

    return SEEN_TEE_ERR_NOT_SUPPORTED;
}

int64_t __seen_enclave_create(const char* enclave_path, size_t heap_size, size_t stack_size) {
    if (!g_tee_initialized) {
        __seen_tee_init();
    }

    (void)enclave_path;  // Suppress unused warnings
    (void)heap_size;
    (void)stack_size;

    if (g_stub_mode) {
        // Return a fake enclave ID
        static int64_t stub_enclave_counter = 1000;
        return stub_enclave_counter++;
    }

    if (g_tee_type == SEEN_TEE_SGX) {
        return __seen_sgx_enclave_create(enclave_path, heap_size, stack_size);
    }

    return -1;  // Not supported
}

SeenTEEStatus __seen_enclave_destroy(int64_t enclave_id) {
    (void)enclave_id;

    if (g_stub_mode) {
        if (g_current_enclave_id == enclave_id) {
            g_in_enclave = 0;
            g_current_enclave_id = -1;
        }
        return SEEN_TEE_SUCCESS;
    }

    if (g_tee_type == SEEN_TEE_SGX) {
        SeenTEEStatus status = __seen_sgx_enclave_destroy(enclave_id);
        if (status == SEEN_TEE_SUCCESS && g_current_enclave_id == enclave_id) {
            g_in_enclave = 0;
            g_current_enclave_id = -1;
        }
        return status;
    }

    return SEEN_TEE_ERR_NOT_SUPPORTED;
}

// ============================================================================
// Data Sealing (Stub Implementation)
// ============================================================================

// Simple XOR-based stub encryption (NOT cryptographically secure!)
static void stub_xor_encrypt(const uint8_t* input, size_t size,
                              const uint8_t* key, uint8_t* output) {
    for (size_t i = 0; i < size; i++) {
        output[i] = input[i] ^ key[i % SEEN_TEE_KEY_SIZE];
    }
}

SeenTEEStatus __seen_seal_data(
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

    if (plaintext_size > SEEN_TEE_MAX_SEALED_SIZE) {
        return SEEN_TEE_ERR_INVALID_PARAM;
    }

    if (!g_tee_initialized) {
        __seen_tee_init();
    }

    memset(sealed_output, 0, sizeof(*sealed_output));
    sealed_output->tee_type = g_tee_type;
    sealed_output->policy = policy;

    if (g_stub_mode) {
        // Stub mode: use simple XOR (NOT secure!)
        stub_xor_encrypt(plaintext, plaintext_size, g_stub_key, sealed_output->sealed_data);
        sealed_output->sealed_size = plaintext_size;

        if (additional_data && additional_size > 0) {
            size_t copy_size = additional_size < 256 ? additional_size : 256;
            memcpy(sealed_output->additional_data, additional_data, copy_size);
            sealed_output->additional_size = copy_size;
        }

        sealed_output->valid = 1;
        return SEEN_TEE_SUCCESS;
    }

    if (g_tee_type == SEEN_TEE_SGX) {
        return __seen_sgx_seal_data(
            plaintext,
            plaintext_size,
            additional_data,
            additional_size,
            policy,
            sealed_output
        );
    }

    if (g_tee_type == SEEN_TEE_SEV) {
        return __seen_sev_seal_data(
            plaintext,
            plaintext_size,
            additional_data,
            additional_size,
            policy,
            sealed_output
        );
    }

    return SEEN_TEE_ERR_NOT_SUPPORTED;
}

SeenTEEStatus __seen_unseal_data(
    const SeenSealedData* sealed_input,
    uint8_t* plaintext_output,
    size_t* plaintext_size
) {
    if (!sealed_input || !plaintext_output || !plaintext_size) {
        return SEEN_TEE_ERR_INVALID_PARAM;
    }

    if (!sealed_input->valid) {
        return SEEN_TEE_ERR_UNSEAL;
    }

    if (!g_tee_initialized) {
        __seen_tee_init();
    }

    if (g_stub_mode) {
        // Stub mode: reverse XOR
        stub_xor_encrypt(sealed_input->sealed_data, sealed_input->sealed_size,
                        g_stub_key, plaintext_output);
        *plaintext_size = sealed_input->sealed_size;
        return SEEN_TEE_SUCCESS;
    }

    if (sealed_input->tee_type == SEEN_TEE_SGX) {
        return __seen_sgx_unseal_data(sealed_input, plaintext_output, plaintext_size);
    }

    if (sealed_input->tee_type == SEEN_TEE_SEV) {
        return __seen_sev_unseal_data(sealed_input, plaintext_output, plaintext_size);
    }

    return SEEN_TEE_ERR_NOT_SUPPORTED;
}

// ============================================================================
// Attestation (Stub Implementation)
// ============================================================================

SeenTEEStatus __seen_get_attestation_report(
    const uint8_t* report_data,
    size_t report_data_size,
    SeenAttestationType attest_type,
    SeenAttestationReport* report_output
) {
    if (!report_output) {
        return SEEN_TEE_ERR_INVALID_PARAM;
    }

    if (!g_tee_initialized) {
        __seen_tee_init();
    }

    memset(report_output, 0, sizeof(*report_output));
    report_output->tee_type = g_tee_type;
    report_output->attest_type = attest_type;
    report_output->timestamp = (int64_t)time(NULL);

    if (g_stub_mode) {
        // Stub mode: generate fake report
        if (report_data && report_data_size > 0) {
            size_t copy_size = report_data_size < SEEN_TEE_MAX_REPORT_SIZE
                              ? report_data_size : SEEN_TEE_MAX_REPORT_SIZE;
            memcpy(report_output->report_data, report_data, copy_size);
            report_output->report_size = copy_size;
        }

        // Fake measurement (hash of "STUB_MEASUREMENT")
        const char* stub_measurement = "STUB_MEASUREMENT_00000000000000";
        size_t meas_len = strlen(stub_measurement);
        memcpy(report_output->measurement, stub_measurement, meas_len);
        report_output->measurement_size = meas_len;

        report_output->valid = 1;
        return SEEN_TEE_SUCCESS;
    }

    if (g_tee_type == SEEN_TEE_SGX) {
        SeenTEEStatus status = __seen_sgx_get_report(
            report_data,
            report_data_size,
            attest_type,
            report_output
        );
        if (status == SEEN_TEE_SUCCESS && report_output->timestamp == 0) {
            report_output->timestamp = (int64_t)time(NULL);
        }
        return status;
    }

    if (g_tee_type == SEEN_TEE_SEV) {
        SeenTEEStatus status = __seen_sev_get_report(
            report_data,
            report_data_size,
            attest_type,
            report_output
        );
        if (status == SEEN_TEE_SUCCESS && report_output->timestamp == 0) {
            report_output->timestamp = (int64_t)time(NULL);
        }
        return status;
    }

    return SEEN_TEE_ERR_NOT_SUPPORTED;
}

SeenTEEStatus __seen_verify_attestation_report(
    const SeenAttestationReport* report,
    const uint8_t* expected_measurement,
    size_t measurement_size
) {
    if (!report) {
        return SEEN_TEE_ERR_INVALID_PARAM;
    }

    if (!report->valid) {
        return SEEN_TEE_ERR_VERIFY;
    }

    if (g_stub_mode) {
        // Stub mode: verify measurement if provided
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

    if (report->tee_type == SEEN_TEE_SGX) {
        return __seen_sgx_verify_report(report, expected_measurement, measurement_size);
    }

    if (report->tee_type == SEEN_TEE_SEV) {
        return __seen_sev_verify_report(report, expected_measurement, measurement_size);
    }

    return SEEN_TEE_ERR_NOT_SUPPORTED;
}

// ============================================================================
// Key Derivation (Stub Implementation)
// ============================================================================

SeenTEEStatus __seen_derive_key(
    const uint8_t* key_id,
    size_t key_id_size,
    uint8_t* key_output
) {
    if (!key_id || !key_output) {
        return SEEN_TEE_ERR_INVALID_PARAM;
    }

    if (!g_tee_initialized) {
        __seen_tee_init();
    }

    if (g_stub_mode) {
        // Stub mode: derive key by XORing stub key with key_id
        for (size_t i = 0; i < SEEN_TEE_KEY_SIZE; i++) {
            key_output[i] = g_stub_key[i];
            if (i < key_id_size) {
                key_output[i] ^= key_id[i];
            }
        }
        return SEEN_TEE_SUCCESS;
    }

    if (g_tee_type == SEEN_TEE_SGX) {
        return __seen_sgx_derive_key(key_id, key_id_size, key_output);
    }

    if (g_tee_type == SEEN_TEE_SEV) {
        return __seen_sev_derive_key(key_id, key_id_size, key_output);
    }

    return SEEN_TEE_ERR_NOT_SUPPORTED;
}

// ============================================================================
// String-based API Implementation
// ============================================================================

// Convert bytes to hex string
static char* bytes_to_hex(const uint8_t* bytes, size_t len) {
    char* hex = (char*)malloc(len * 2 + 1);
    if (!hex) return NULL;

    for (size_t i = 0; i < len; i++) {
        sprintf(hex + i * 2, "%02x", bytes[i]);
    }
    hex[len * 2] = '\0';
    return hex;
}

// Convert hex string to bytes
static size_t hex_to_bytes(const char* hex, uint8_t* bytes, size_t max_len) {
    size_t hex_len = strlen(hex);
    size_t byte_len = hex_len / 2;
    if (byte_len > max_len) byte_len = max_len;

    for (size_t i = 0; i < byte_len; i++) {
        unsigned int val;
        sscanf(hex + i * 2, "%2x", &val);
        bytes[i] = (uint8_t)val;
    }
    return byte_len;
}

static const char* skip_json_whitespace(const char* cursor) {
    while (cursor && *cursor && isspace((unsigned char)*cursor)) {
        cursor++;
    }
    return cursor;
}

static int extract_json_bool_field(const char* json, const char* key, int* value) {
    if (!json || !key || !value) {
        return 0;
    }
    char needle[64];
    snprintf(needle, sizeof(needle), "\"%s\"", key);
    const char* key_pos = strstr(json, needle);
    if (!key_pos) {
        return 0;
    }
    const char* colon = strchr(key_pos, ':');
    if (!colon) {
        return 0;
    }
    const char* field = skip_json_whitespace(colon + 1);
    if (!field) {
        return 0;
    }
    if (strncmp(field, "true", 4) == 0) {
        *value = 1;
        return 1;
    }
    if (strncmp(field, "false", 5) == 0) {
        *value = 0;
        return 1;
    }
    return 0;
}

static int extract_json_string_field(const char* json, const char* key, char* output, size_t output_size) {
    if (!json || !key || !output || output_size == 0) {
        return 0;
    }
    char needle[64];
    snprintf(needle, sizeof(needle), "\"%s\"", key);
    const char* key_pos = strstr(json, needle);
    if (!key_pos) {
        output[0] = '\0';
        return 0;
    }
    const char* colon = strchr(key_pos, ':');
    if (!colon) {
        output[0] = '\0';
        return 0;
    }
    const char* field = skip_json_whitespace(colon + 1);
    if (!field || *field != '"') {
        output[0] = '\0';
        return 0;
    }
    field++;
    const char* end = strchr(field, '"');
    if (!end) {
        output[0] = '\0';
        return 0;
    }
    size_t copy_size = (size_t)(end - field);
    if (copy_size >= output_size) {
        copy_size = output_size - 1;
    }
    memcpy(output, field, copy_size);
    output[copy_size] = '\0';
    return 1;
}

SeenString __seen_seal_string(SeenString plaintext, SeenString additional_data, int64_t policy) {
    if (!plaintext.data) {
        return seen_cstr_to_str("");
    }

    const uint8_t* aad = additional_data.data ? (const uint8_t*)additional_data.data : NULL;
    size_t aad_len = additional_data.data ? (size_t)additional_data.len : 0;

    SeenSealedData sealed;
    SeenTEEStatus status = __seen_seal_data(
        (const uint8_t*)plaintext.data, (size_t)plaintext.len,
        aad, aad_len,
        (SeenSealPolicy)policy,
        &sealed
    );

    if (status != SEEN_TEE_SUCCESS) {
        return seen_cstr_to_str("");
    }

    char* hex = bytes_to_hex(sealed.sealed_data, sealed.sealed_size);
    if (!hex) {
        return seen_cstr_to_str("");
    }
    return seen_cstr_to_str(hex);
}

SeenString __seen_unseal_string_ex(SeenString sealed_hex, SeenString additional_data, int64_t tee_type, int64_t policy) {
    if (!sealed_hex.data || sealed_hex.len == 0) {
        return seen_cstr_to_str("");
    }

    char* sealed_hex_cstr = seen_str_to_cstr(sealed_hex);
    if (!sealed_hex_cstr) {
        return seen_cstr_to_str("");
    }

    SeenSealedData sealed;
    memset(&sealed, 0, sizeof(sealed));
    sealed.sealed_size = hex_to_bytes(sealed_hex_cstr, sealed.sealed_data, SEEN_TEE_MAX_SEALED_SIZE);
    free(sealed_hex_cstr);
    sealed.valid = 1;
    sealed.tee_type = (SeenTEEType)tee_type;
    sealed.policy = (SeenSealPolicy)policy;

    if (additional_data.data && additional_data.len > 0) {
        size_t aad_len = (size_t)additional_data.len;
        if (aad_len > 256) aad_len = 256;
        memcpy(sealed.additional_data, additional_data.data, aad_len);
        sealed.additional_size = aad_len;
    }

    uint8_t plaintext[SEEN_TEE_MAX_SEALED_SIZE];
    size_t plain_len;

    SeenTEEStatus status = __seen_unseal_data(&sealed, plaintext, &plain_len);
    if (status != SEEN_TEE_SUCCESS) {
        return seen_cstr_to_str("");
    }

    char* result = (char*)malloc(plain_len + 1);
    if (!result) {
        return seen_cstr_to_str("");
    }

    memcpy(result, plaintext, plain_len);
    result[plain_len] = '\0';
    return seen_cstr_to_str(result);
}

SeenString __seen_unseal_string(SeenString sealed_hex, SeenString additional_data) {
    return __seen_unseal_string_ex(sealed_hex, additional_data, g_tee_type, SEEN_SEAL_MRENCLAVE);
}

SeenString __seen_get_attestation_json(SeenString report_data, int64_t attest_type) {
    SeenAttestationReport report;
    SeenTEEStatus status = __seen_get_attestation_report(
        report_data.data ? (const uint8_t*)report_data.data : NULL,
        report_data.data ? (size_t)report_data.len : 0,
        (SeenAttestationType)attest_type,
        &report
    );

    if (status != SEEN_TEE_SUCCESS) {
        return seen_cstr_to_str("");
    }

    // Build JSON manually (no JSON library dependency)
    char* json = (char*)malloc(4096);
    if (!json) {
        return seen_cstr_to_str("");
    }

    char* measurement_hex = bytes_to_hex(report.measurement, report.measurement_size);

    snprintf(json, 4096,
        "{\n"
        "  \"tee_type\": \"%s\",\n"
        "  \"attest_type\": \"%s\",\n"
        "  \"timestamp\": %ld,\n"
        "  \"measurement\": \"%s\",\n"
        "  \"valid\": %s\n"
        "}",
        __seen_tee_type_name(report.tee_type),
        report.attest_type == SEEN_ATTEST_LOCAL ? "local" : "remote",
        (long)report.timestamp,
        measurement_hex ? measurement_hex : "",
        report.valid ? "true" : "false"
    );

    free(measurement_hex);
    return seen_cstr_to_str(json);
}

SeenString __seen_attestation_measurement_json(SeenString report_json) {
    if (!report_json.data) {
        return seen_cstr_to_str("");
    }

    char* report_json_cstr = seen_str_to_cstr(report_json);
    if (!report_json_cstr) {
        return seen_cstr_to_str("");
    }

    char measurement_hex[SEEN_TEE_MAX_REPORT_SIZE * 2 + 1];
    int found = extract_json_string_field(report_json_cstr, "measurement", measurement_hex, sizeof(measurement_hex));
    free(report_json_cstr);
    if (!found) {
        return seen_cstr_to_str("");
    }

    return seen_str_copy(measurement_hex);
}

int64_t __seen_verify_attestation_json(SeenString report_json, SeenString expected_measurement) {
    if (!report_json.data) {
        return 0;
    }

    char* report_json_cstr = seen_str_to_cstr(report_json);
    if (!report_json_cstr) {
        return 0;
    }

    char* expected_measurement_cstr = NULL;
    if (expected_measurement.data) {
        expected_measurement_cstr = seen_str_to_cstr(expected_measurement);
        if (!expected_measurement_cstr) {
            free(report_json_cstr);
            return 0;
        }
    }

    int report_valid = 0;
    if (!extract_json_bool_field(report_json_cstr, "valid", &report_valid) || !report_valid) {
        free(report_json_cstr);
        free(expected_measurement_cstr);
        return 0;
    }

    if (expected_measurement_cstr && expected_measurement_cstr[0] != '\0') {
        char measurement_hex[SEEN_TEE_MAX_REPORT_SIZE * 2 + 1];
        if (!extract_json_string_field(report_json_cstr, "measurement", measurement_hex, sizeof(measurement_hex))) {
            free(report_json_cstr);
            free(expected_measurement_cstr);
            return 0;
        }
        if (strcmp(measurement_hex, expected_measurement_cstr) != 0) {
            free(report_json_cstr);
            free(expected_measurement_cstr);
            return 0;
        }
    }

    free(report_json_cstr);
    free(expected_measurement_cstr);
    return 1;
}

// ============================================================================
// Diagnostics
// ============================================================================

uint64_t __seen_tee_get_capabilities(void) {
    if (!g_tee_initialized) {
        __seen_tee_init();
    }

    uint64_t caps = 0;

    if (g_stub_mode) {
        // Stub mode supports everything (for testing)
        caps = SEEN_TEE_CAP_SEAL | SEEN_TEE_CAP_ATTEST_LOCAL |
               SEEN_TEE_CAP_DERIVE_KEY | SEEN_TEE_CAP_ENCLAVE;
    } else if (g_tee_type == SEEN_TEE_SGX) {
        caps = SEEN_TEE_CAP_ENCLAVE;
    } else if (g_tee_type == SEEN_TEE_SEV) {
        caps = SEEN_TEE_CAP_SEAL | SEEN_TEE_CAP_ATTEST_LOCAL | SEEN_TEE_CAP_DERIVE_KEY;
    }

    return caps;
}

void __seen_tee_print_info(void) {
    if (!g_tee_initialized) {
        __seen_tee_init();
    }

    fprintf(stderr, "\n=== SEEN TEE INFO ===\n");
    fprintf(stderr, "TEE Type: %s\n", __seen_tee_type_name(g_tee_type));
    fprintf(stderr, "Initialized: %s\n", g_tee_initialized ? "Yes" : "No");
    fprintf(stderr, "In Enclave: %s\n", g_in_enclave ? "Yes" : "No");
    fprintf(stderr, "Stub Mode: %s\n", g_stub_mode ? "Yes (SEEN_TEE_ALLOW_STUB=1)" : "No");

    uint64_t caps = __seen_tee_get_capabilities();
    fprintf(stderr, "Capabilities:\n");
    fprintf(stderr, "  - Sealing: %s\n", (caps & SEEN_TEE_CAP_SEAL) ? "Yes" : "No");
    fprintf(stderr, "  - Local Attestation: %s\n", (caps & SEEN_TEE_CAP_ATTEST_LOCAL) ? "Yes" : "No");
    fprintf(stderr, "  - Remote Attestation: %s\n", (caps & SEEN_TEE_CAP_ATTEST_REMOTE) ? "Yes" : "No");
    fprintf(stderr, "  - Key Derivation: %s\n", (caps & SEEN_TEE_CAP_DERIVE_KEY) ? "Yes" : "No");
    fprintf(stderr, "  - Enclave Execution: %s\n", (caps & SEEN_TEE_CAP_ENCLAVE) ? "Yes" : "No");
    fprintf(stderr, "=====================\n\n");
}
