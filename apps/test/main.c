#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <syscalls.h>

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

int main(int argc, const char* argv[]) {
    // return test_tcp_server();
    return test_tcp_client();
}
