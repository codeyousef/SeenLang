#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>

// Type definitions
typedef enum {
    STATUS_TAG_ACTIVE,
    STATUS_TAG_INACTIVE,
    STATUS_TAG_PENDING,
} Status_tag;

typedef struct {
    Status_tag tag;
    union {
    } data;
} Status;

typedef enum {
    RESULT_TAG_SUCCESS,
    RESULT_TAG_FAILURE,
} Result_tag;

typedef struct {
    Result_tag tag;
    union {
        int64_t success;
        char* failure;
    } data;
} Result;

Status Status__Active() {
    Status result;
    result.tag = STATUS_TAG_ACTIVE;
    return result;
}

Status Status__Inactive() {
    Status result;
    result.tag = STATUS_TAG_INACTIVE;
    return result;
}

Status Status__Pending() {
    Status result;
    result.tag = STATUS_TAG_PENDING;
    return result;
}

Result Result__Success(int64_t arg0) {
    Result result;
    result.tag = RESULT_TAG_SUCCESS;
    result.data.success = arg0;
    return result;
}

Result Result__Failure(char* arg0) {
    Result result;
    result.tag = RESULT_TAG_FAILURE;
    result.data.failure = arg0;
    return result;
}

// Module: main
int64_t seen_main() {
    return 0;
}

int main(int argc, char* argv[]) {
    return seen_main();
}
