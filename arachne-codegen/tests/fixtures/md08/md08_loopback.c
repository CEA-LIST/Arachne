/* MD.08 baseline-only adapter: keep unchanged legacy process tests on loopback.
 * No descriptor, operation, HTTP response or assertion is changed.
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <netinet/in.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

int bind(int fd, const struct sockaddr *address, socklen_t length) {
    int (*real_bind)(int, const struct sockaddr *, socklen_t) = dlsym(RTLD_NEXT, "bind");
    struct sockaddr_in local;
    if (!real_bind) { errno = ENOSYS; return -1; }
    if (address && address->sa_family == AF_INET && length >= sizeof(local)) {
        memcpy(&local, address, sizeof(local));
        unsigned port = ntohs(local.sin_port);
        if (port == 7447 || port == 7000 || port == 7200 || port == 8081 ||
            port == 8082 || port == 8090 || (port >= 8181 && port <= 8183)) {
            errno = EACCES; return -1;
        }
        if (local.sin_addr.s_addr != htonl(INADDR_ANY) &&
            local.sin_addr.s_addr != htonl(INADDR_LOOPBACK)) {
            errno = EACCES; return -1;
        }
        local.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
        int result = real_bind(fd, (const struct sockaddr *)&local, sizeof(local));
        if (result == 0) fprintf(stderr, "MD08 bind pid=%ld 127.0.0.1:%u\n", (long)getpid(), port);
        return result;
    }
    if (address && address->sa_family == AF_INET6) { errno = EAFNOSUPPORT; return -1; }
    return real_bind(fd, address, length);
}
