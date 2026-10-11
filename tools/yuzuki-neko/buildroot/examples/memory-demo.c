// SPDX-License-Identifier: MPL-2.0
#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <sys/mman.h>
#include <unistd.h>

int main(void)
{
    long page_size = sysconf(_SC_PAGESIZE);
    if (page_size <= 0) {
        fputs("cannot determine page size\n", stderr);
        return 1;
    }
    size_t length = (size_t)page_size * 16;
    unsigned char *memory = mmap(NULL, length, PROT_READ | PROT_WRITE,
                                 MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (memory == MAP_FAILED) {
        perror("mmap");
        return 1;
    }
    for (size_t offset = 0; offset < length; ++offset)
        memory[offset] = (unsigned char)offset;
    printf("Mapped and wrote %zu bytes across 16 pages.\n", length);

    if (mprotect(memory, length, PROT_READ) != 0) {
        perror("mprotect");
        return 1;
    }
    for (size_t offset = 0; offset < length; ++offset) {
        if (memory[offset] != (unsigned char)offset) {
            fputs("mapped data mismatch\n", stderr);
            return 1;
        }
    }
    puts("Changed mapping to read-only and verified its contents.");
    if (munmap(memory, length) != 0) {
        perror("munmap");
        return 1;
    }
    puts("Released the mapping: memory demo complete.");
    return 0;
}
