#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/personality.h>
#include <sys/prctl.h>
#include <sys/resource.h>

static pthread_barrier_t barrier;
static pthread_mutex_t mutex = PTHREAD_MUTEX_INITIALIZER;
static volatile int value;
static int deliberately_racy;

static void require_zero(int status) {
    if (status != 0) {
        fprintf(stderr, "native thread control failure: %d\n", status);
        abort();
    }
}

static void *worker(void *unused) {
    (void)unused;
    int status = pthread_barrier_wait(&barrier);
    if (status != 0 && status != PTHREAD_BARRIER_SERIAL_THREAD) {
        abort();
    }
    if (!deliberately_racy) {
        require_zero(pthread_mutex_lock(&mutex));
    }
    ++value;
    if (!deliberately_racy) {
        require_zero(pthread_mutex_unlock(&mutex));
    }
    return NULL;
}

int main(int argc, char **argv) {
    if (prctl(PR_SET_DUMPABLE, 0L, 0L, 0L, 0L) != 0) {
        return 5;
    }
    struct rlimit limit;
    if (getrlimit(RLIMIT_AS, &limit) != 0) {
        return 2;
    }
    int personality_flags = personality(0xffffffffUL);
    if (personality_flags < 0 || (personality_flags & ADDR_NO_RANDOMIZE) != 0) {
        fprintf(stderr, "native control requires unchanged enabled ASLR\n");
        return 3;
    }
    deliberately_racy = argc == 2 && strcmp(argv[1], "race") == 0;
    require_zero(pthread_barrier_init(&barrier, NULL, 2));
    pthread_t threads[2];
    require_zero(pthread_create(&threads[0], NULL, worker, NULL));
    require_zero(pthread_create(&threads[1], NULL, worker, NULL));
    require_zero(pthread_join(threads[0], NULL));
    require_zero(pthread_join(threads[1], NULL));
    require_zero(pthread_barrier_destroy(&barrier));
    require_zero(pthread_mutex_destroy(&mutex));
    printf("TSAN_CONTROL value=%d soft=%llu hard=%llu aslr_disabled=0\n", value,
           (unsigned long long)limit.rlim_cur, (unsigned long long)limit.rlim_max);
    return value == 2 ? 0 : 4;
}
