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
    const char* test_msg = "Hello from myOS UDP socket!";

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

int main(int argc, const char* argv[]) {
    return test_udp();
}
