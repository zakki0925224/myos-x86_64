#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <syscalls.h>
#include <input.h>

#define FIFO_PATH "/fifo-test"

int test_udp() {
    int sockfd = sys_socket(SOCKET_DOMAIN_AF_INET, SOCKET_TYPE_SOCK_DGRAM, SOCKET_PROTO_UDP);
    if (sockfd < 0) {
        printf("Failed to create socket\n");
        return -1;
    }

    struct sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = SOCKET_DOMAIN_AF_INET;
    addr.sin_port = 0;  // auto-assign
    addr.sin_addr.s_addr = 0;

    if (sys_bind(sockfd, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
        printf("Failed to bind socket\n");
        return -1;
    }

    // test data
    const char* test_msg = "Hello from myos UDP socket!";

    struct sockaddr_in dest_addr;
    memset(&dest_addr, 0, sizeof(dest_addr));
    dest_addr.sin_family = SOCKET_DOMAIN_AF_INET;
    dest_addr.sin_port = 1234;
    dest_addr.sin_addr.s_addr = (192 << 24) | (168 << 16) | (100 << 8) | 1;

    int ret = sys_sendto(sockfd, test_msg, strlen(test_msg) + 1, 0,
                         (struct sockaddr*)&dest_addr, sizeof(dest_addr));
    if (ret < 0) {
        printf("Failed to sendto\n");
        return -1;
    }

    char recv_buf[256];
    memset(recv_buf, 0, sizeof(recv_buf));
    struct sockaddr_in src_addr;
    memset(&src_addr, 0, sizeof(src_addr));
    int recv_len = 0;
    // wait
    while (recv_len <= 0) {
        recv_len = sys_recvfrom(sockfd, recv_buf, sizeof(recv_buf), 0,
                                (struct sockaddr*)&src_addr, sizeof(src_addr));
    }
    printf("Received %d bytes from host: %s\n", recv_len, recv_buf);

    return 0;
}

int test_tcp_server() {
    printf("=== TCP Server Test ===\n");

    int sockfd = sys_socket(SOCKET_DOMAIN_AF_INET, SOCKET_TYPE_SOCK_STREAM, 0);
    if (sockfd < 0) {
        printf("Failed to create socket\n");
        return -1;
    }
    printf("TCP socket created: fd=%d\n", sockfd);

    struct sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = SOCKET_DOMAIN_AF_INET;
    addr.sin_port = 5000;
    addr.sin_addr.s_addr = 0;  // INADDR_ANY

    printf("Binding to port 5000...\n");
    if (sys_bind(sockfd, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
        printf("Failed to bind\n");
        return -1;
    }
    printf("Bound!\n");

    printf("Listening...\n");
    if (sys_listen(sockfd, 1) < 0) {
        printf("Failed to listen\n");
        return -1;
    }
    printf("Listening on port 5000\n");

    struct sockaddr_in client_addr;
    memset(&client_addr, 0, sizeof(client_addr));
    size_t client_addr_len = sizeof(client_addr);

    printf("Waiting for connection...\n");
    int client_fd = sys_accept(sockfd, (struct sockaddr*)&client_addr, &client_addr_len);
    if (client_fd < 0) {
        printf("Failed to accept\n");
        return -1;
    }
    printf("Connection accepted! client_fd=%d\n", client_fd);

    char recv_buf[256];
    memset(recv_buf, 0, sizeof(recv_buf));
    printf("Waiting for data...\n");
    int recv_len = 0;
    while (recv_len <= 0) {
        recv_len = sys_recv(client_fd, recv_buf, sizeof(recv_buf), 0);
    }
    printf("Received %d bytes: %s\n", recv_len, recv_buf);

    const char* response = "Hello from TCP server!";
    printf("Sending response: %s\n", response);
    int sent = sys_send(client_fd, response, strlen(response), 0);
    if (sent < 0) {
        printf("Failed to send\n");
        return -1;
    }
    printf("Sent %d bytes\n", sent);

    return 0;
}

int test_tcp_client() {
    printf("=== TCP Client Test ===\n");

    int sockfd = sys_socket(SOCKET_DOMAIN_AF_INET, SOCKET_TYPE_SOCK_STREAM, 0);
    if (sockfd < 0) {
        printf("Failed to create socket\n");
        return -1;
    }
    printf("TCP socket created: fd=%d\n", sockfd);

    struct sockaddr_in dest_addr;
    memset(&dest_addr, 0, sizeof(dest_addr));
    dest_addr.sin_family = SOCKET_DOMAIN_AF_INET;
    dest_addr.sin_port = 12345;
    // 192.168.100.1
    dest_addr.sin_addr.s_addr = (192 << 24) | (168 << 16) | (100 << 8) | 1;

    printf("Connecting to 192.168.100.1:12345...\n");
    if (sys_connect(sockfd, (struct sockaddr*)&dest_addr, sizeof(dest_addr)) < 0) {
        printf("Failed to connect\n");
        sys_close(sockfd);
        return -1;
    }
    printf("Connected!\n");

    const char* msg = "Hello from myos TCP client!";
    printf("Sending: %s\n", msg);
    int sent = sys_send(sockfd, msg, strlen(msg), 0);
    if (sent < 0) {
        printf("Failed to send\n");
        sys_close(sockfd);
        return -1;
    }
    printf("Sent %d bytes\n", sent);

    char recv_buf[256];
    memset(recv_buf, 0, sizeof(recv_buf));
    printf("Waiting for response...\n");
    int recv_len = 0;
    while (recv_len == 0) {
        recv_len = sys_recv(sockfd, recv_buf, sizeof(recv_buf), 0);
    }
    if (recv_len < 0) {
        printf("Failed to recv\n");
        sys_close(sockfd);
        return -1;
    }
    printf("Received %d bytes: %s\n", recv_len, recv_buf);

    sys_close(sockfd);
    return 0;
}

int test_pipe() {
    printf("=== Pipe Test ===\n");

    int pipefd[2];
    if (sys_pipe(pipefd) < 0) {
        printf("FAIL: sys_pipe\n");
        return -1;
    }
    printf("pipe created: read_fd=%d, write_fd=%d\n", pipefd[0], pipefd[1]);

    const char* msg = "hello pipe";
    int written = sys_write(pipefd[1], msg, strlen(msg));
    if (written < 0) {
        printf("FAIL: write\n");
        return -1;
    }
    printf("written: %d bytes\n", written);

    char buf[64];
    memset(buf, 0, sizeof(buf));
    int read_len = sys_read(pipefd[0], buf, sizeof(buf));
    if (read_len < 0) {
        printf("FAIL: read\n");
        return -1;
    }
    printf("read: %d bytes, data=\"%s\"\n", read_len, buf);

    if (strcmp(buf, msg) == 0) {
        printf("OK\n");
    } else {
        printf("FAIL: data mismatch\n");
        return -1;
    }

    sys_close(pipefd[0]);
    sys_close(pipefd[1]);
    return 0;
}

int test_crash() {
    printf("=== Crash Test ===\n");

    int* p = (int*)0x0;
    *p = 42;  // null pointer dereference -> page fault

    return 0;
}

int global_counter = 100;

int test_fork() {
    printf("=== Fork Test ===\n");

    global_counter = 100;
    int local_var = 1;

    pid_t pid = sys_fork();
    if (pid < 0) {
        printf("FAIL: sys_fork\n");
        return -1;
    }

    if (pid == 0) {
        // child
        local_var++;
        global_counter++;
        printf("child: pid=%d, local_var=%d, global_counter=%d\n", sys_getpid(), local_var, global_counter);
        sys_exit(local_var == 2 && global_counter == 101 ? 0 : 1);
    }

    // parent
    printf("parent: forked child pid=%d\n", pid);
    printf("parent: local_var=%d, global_counter=%d (should be unchanged)\n", local_var, global_counter);

    int status = sys_wait(pid);
    printf("parent: child exited with status=%d\n", status);

    if (status != 0) {
        printf("FAIL: child reported bad state\n");
        return -1;
    }
    if (local_var != 1 || global_counter != 100) {
        printf("FAIL: parent's memory was mutated by child (no address space isolation)\n");
        return -1;
    }

    printf("OK\n");
    return 0;
}

int test_sleep() {
    const uint64_t cases[] = {10, 20, 55, 100, 1000, 10000};

    for (int i = 0; i < 6; i++) {
        uint64_t ms = cases[i];
        uint64_t start = sys_uptime();
        sys_sleep(ms);
        uint64_t elapsed = sys_uptime() - start;

        printf("sleep(%d): elapsed=%d\n", (int)ms, (int)elapsed);

        if (elapsed < ms || elapsed > ms + 20) {
            return 1;
        }
    }

    return 0;
}

int test_input() {
    int mfd = sys_open("/dev/mouse", OPEN_FLAG_NONE);
    int kfd = sys_open("/dev/keyboard", OPEN_FLAG_NONE);

    if (mfd < 0 || kfd < 0) {
        printf("failed to open input devices\n");
        return 1;
    }

    int running = 1;
    while (running) {
        mouse_event me[16];
        int mlen = sys_read(mfd, me, sizeof(me));
        for (int i = 0; i < mlen / (int)sizeof(mouse_event); i++) {
            printf("mouse: buttons=%d abs=%d x=%d y=%d\n",
                    me[i].buttons, me[i].is_abs, me[i].x, me[i].y);
        }

        key_event ke[16];
        int klen = sys_read(kfd, ke, sizeof(ke));
        for (int i = 0; i < klen / (int)sizeof(key_event); i++) {
            printf("key: code=%d pressed=%d c=%d\n",
                    ke[i].code, ke[i].pressed, (int)ke[i].c);

            if (ke[i].c == 'q' && ke[i].pressed) {
                running = 0;
            }
        }

        sys_sleep(10);
    }

    sys_close(mfd);
    sys_close(kfd);
    return 0;
}

int test_fifo_reader() {
    sys_mkfifo(FIFO_PATH);

    int fd = sys_open(FIFO_PATH, OPEN_FLAG_READ);
    if (fd < 0) {
        printf("reader: open failed\n");
        return 1;
    }

    for (int i = 0; i < 1000; i++) {
        char buf[64];
        int len = sys_read(fd, buf, sizeof(buf) - 1);
        if (len > 0) {
            buf[len] = '\0';
            printf("reader: %s\n", buf);
        }
        sys_sleep(10);
    }

    sys_close(fd);
    sys_unlink(FIFO_PATH);
    return 0;
}

int test_fifo_writer(const char* msg) {
    int fd = sys_open(FIFO_PATH, OPEN_FLAG_WRITE);
    if (fd < 0) {
        printf("writer: open failed\n");
        return 1;
    }

    sys_write(fd, msg, strlen(msg));
    sys_close(fd);
    return 0;
}

int main(int argc, const char* argv[]) {
    // return test_tcp_server();
    // return test_tcp_client();
    // return test_pipe();
    // return test_crash();
    // return test_fork();
    // return test_sleep();
    // return test_input();

    if (argc > 2 && strcmp(argv[1], "fifo-write") == 0) {
        return test_fifo_writer(argv[2]);
    }
        if (argc > 1 && strcmp(argv[1], "fifo-read") == 0) {
        return test_fifo_reader();
    }

    return test_input();
}
