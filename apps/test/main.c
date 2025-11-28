#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <syscalls.h>

int main(int argc, const char* argv[]) {
    printf("UDP test - sending to host\n");

    int sockfd = sys_socket(SOCKET_DOMAIN_AF_INET, SOCKET_TYPE_SOCK_DGRAM, SOCKET_PROTO_UDP);
    if (sockfd < 0) {
        printf("Failed to create socket\n");
        return -1;
    }
    printf("Socket created: fd=%d\n", sockfd);

    // bind at ephemeral port (port 0)
    struct sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = SOCKET_DOMAIN_AF_INET;
    addr.sin_port = 0;  // auto-assign
    addr.sin_addr.s_addr = 0;

    if (sys_bind(sockfd, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
        printf("Failed to bind socket\n");
        return -1;
    }
    printf("Socket bound to auto-assigned port\n");

    // test data
    const char* test_msg = "Hello from myOS UDP socket!";

    // destination: host machine (192.168.100.1:1234)
    struct sockaddr_in dest_addr;
    memset(&dest_addr, 0, sizeof(dest_addr));
    dest_addr.sin_family = SOCKET_DOMAIN_AF_INET;
    dest_addr.sin_port = 1234;
    dest_addr.sin_addr.s_addr = (192 << 24) | (168 << 16) | (100 << 8) | 1;

    printf("Sending to host (192.168.100.1:1234): %s\n", test_msg);
    int ret = sys_sendto(sockfd, test_msg, strlen(test_msg) + 1, 0,
                         (struct sockaddr*)&dest_addr, sizeof(dest_addr));
    if (ret < 0) {
        printf("Failed to sendto\n");
        return -1;
    }
    printf("Sent %d bytes\n", ret);

    return 0;
}
