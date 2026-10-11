// SPDX-License-Identifier: MPL-2.0
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

static volatile int private_value = 7;

int main(void)
{
    int channel[2];
    if (pipe(channel) != 0) {
        perror("pipe");
        return 1;
    }
    pid_t child = fork();
    if (child < 0) {
        perror("fork");
        return 1;
    }
    if (child == 0) {
        close(channel[0]);
        private_value = 99;
        char message[80];
        int length = snprintf(message, sizeof(message), "child PID %ld: private value = %d",
                              (long)getpid(), private_value);
        ssize_t sent;
        do {
            sent = write(channel[1], message, (size_t)length);
        } while (sent < 0 && errno == EINTR);
        close(channel[1]);
        _exit(sent == length ? 0 : 1);
    }

    close(channel[1]);
    char message[80];
    size_t used = 0;
    while (used < sizeof(message) - 1) {
        ssize_t received = read(channel[0], message + used, sizeof(message) - 1 - used);
        if (received < 0) {
            if (errno == EINTR)
                continue;
            perror("read");
            return 1;
        }
        if (received == 0)
            break;
        used += (size_t)received;
    }
    message[used] = '\0';
    close(channel[0]);
    int status;
    pid_t waited;
    do {
        waited = waitpid(child, &status, 0);
    } while (waited < 0 && errno == EINTR);
    if (waited != child) {
        perror("waitpid");
        return 1;
    }
    printf("Pipe received: %s\n", message);
    printf("Parent PID %ld: private value remains %d.\n", (long)getpid(), private_value);
    if (!WIFEXITED(status) || WEXITSTATUS(status) != 0 || private_value != 7 ||
        strstr(message, "private value = 99") == NULL) {
        fputs("unexpected child result\n", stderr);
        return 1;
    }
    puts("Reaped the child: fork and pipe demo complete.");
    return 0;
}
