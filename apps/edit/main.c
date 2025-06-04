#include <stdio.h>
#include <syscalls.h>

#define SCREEN_WIDTH 80
#define SCREEN_HEIGHT 24

#define BOTTOM_BAR_HEIGHT 3

char input_char;

int main(int argc, char *argv[]) {
    // fill screen
    printf("\e[2J");

    // draw top bar
    printf("\e[1;1H");
    printf("\e[7m");
    for (int i = 0; i < SCREEN_WIDTH; i++)
        printf(" ");
    printf("\e[1;1H");
    printf("\t\tEdit - This is not microsoft/edit");

    // draw bottom bar
    printf("\e[%d;1H", SCREEN_HEIGHT - BOTTOM_BAR_HEIGHT + 1);
    for (int i = 0; i < BOTTOM_BAR_HEIGHT; i++) {
        for (int j = 0; j < SCREEN_WIDTH; j++) {
            printf(" ");
        }
        printf("\n");
    }

    printf("\e[2;1H\e[8m");

    for (;;) {
        sys_read(FDN_STDIN, &input_char, 1);
        if (input_char >= ' ')
            printf("\e[0m%c\e[8m", input_char);
        else if (input_char == 0x03) {
            printf("\e[0m");
            return 0;
        }
    }

    printf("\e[0m");
    return 0;
}
