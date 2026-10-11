// SPDX-License-Identifier: MPL-2.0
#include <pthread.h>
#include <sched.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

enum { WORKERS = 2, INCREMENTS = 1000, STACK_BYTES = 64 * 1024 };
static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
static unsigned counter;

static void check(int error, const char *operation)
{
    if (error != 0) {
        fprintf(stderr, "%s: %s\n", operation, strerror(error));
        exit(1);
    }
}

static void *count(void *argument)
{
    unsigned id = *(unsigned *)argument;
    for (unsigned step = 0; step < INCREMENTS; ++step) {
        check(pthread_mutex_lock(&lock), "pthread_mutex_lock");
        ++counter;
        check(pthread_mutex_unlock(&lock), "pthread_mutex_unlock");
        if (step % 64 == 0 && sched_yield() != 0) {
            perror("sched_yield");
            exit(1);
        }
    }
    printf("Worker %u completed %u increments.\n", id, INCREMENTS);
    return NULL;
}

int main(void)
{
    pthread_attr_t attributes;
    pthread_t threads[WORKERS];
    unsigned ids[WORKERS];
    check(pthread_attr_init(&attributes), "pthread_attr_init");
    check(pthread_attr_setstacksize(&attributes, STACK_BYTES), "pthread_attr_setstacksize");
    for (unsigned index = 0; index < WORKERS; ++index) {
        ids[index] = index + 1;
        check(pthread_create(&threads[index], &attributes, count, &ids[index]), "pthread_create");
    }
    check(pthread_attr_destroy(&attributes), "pthread_attr_destroy");
    for (unsigned index = 0; index < WORKERS; ++index)
        check(pthread_join(threads[index], NULL), "pthread_join");
    check(pthread_mutex_destroy(&lock), "pthread_mutex_destroy");
    printf("Joined both workers: counter = %u (expected %u).\n", counter, WORKERS * INCREMENTS);
    return counter == WORKERS * INCREMENTS ? 0 : 1;
}
