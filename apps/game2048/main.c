#include <stdio.h>

#define SIZE 4
#define CELL_WIDTH 16
#define CELL_HEIGHT 8

void draw_board(int board[SIZE][SIZE]) {
    // fill screen
    printf("\e[2J");
    printf("\e[1;1H");

    for (int i = 0; i < SIZE; i++) {
        for (int j = 0; j < SIZE; j++) {
            int y = (i + 1) * CELL_HEIGHT;
            int x = (j + 1) * CELL_WIDTH;

            // draw cell border
            for (int k = y; k < y + CELL_HEIGHT; k++) {
                for (int l = x; l < x + CELL_WIDTH; l++) {
                    if (k == y || k == y + CELL_HEIGHT - 1 || l == x || l == x + CELL_WIDTH - 1)
                        printf("\e[7m\e[%d;%dH+\e[0m", k, l);
                    else
                        printf("\e[%d;%dH ", k, l);
                }
            }

            printf("\e[%d;%dH%d", y + CELL_HEIGHT / 2, x + CELL_WIDTH / 2, board[i][j]);
        }
    }
    printf("\n\n\n\n\n\n\n");
}

void update_board(char key, int board[SIZE][SIZE]) {
    if (key == 'w') {
        for (int j = 0; j < SIZE; j++) {
            for (int i = 1; i < SIZE; i++) {
                if (board[i][j] == 0) continue;
                int k = i;
                while (k > 0 && board[k - 1][j] == 0) {
                    board[k - 1][j] = board[k][j];
                    board[k][j] = 0;
                    k--;
                }
                while (k > 0 && board[k - 1][j] == board[k][j]) {
                    board[k - 1][j] *= 2;
                    board[k][j] = 0;
                    k--;
                }
            }
        }
    } else if (key == 's') {
        for (int j = 0; j < SIZE; j++) {
            for (int i = SIZE - 2; i >= 0; i--) {
                if (board[i][j] == 0) continue;
                int k = i;
                while (k < SIZE - 1 && board[k + 1][j] == 0) {
                    board[k + 1][j] = board[k][j];
                    board[k][j] = 0;
                    k++;
                }
                while (k < SIZE - 1 && board[k + 1][j] == board[k][j]) {
                    board[k + 1][j] *= 2;
                    board[k][j] = 0;
                    k++;
                }
            }
        }
    } else if (key == 'a') {
        for (int i = 0; i < SIZE; i++) {
            for (int j = 1; j < SIZE; j++) {
                if (board[i][j] == 0) continue;
                int k = j;
                while (k > 0 && board[i][k - 1] == 0) {
                    board[i][k - 1] = board[i][k];
                    board[i][k] = 0;
                    k--;
                }
                while (k > 0 && board[i][k - 1] == board[i][k]) {
                    board[i][k - 1] *= 2;
                    board[i][k] = 0;
                    k--;
                }
            }
        }
    } else if (key == 'd') {
        for (int i = 0; i < SIZE; i++) {
            for (int j = SIZE - 2; j >= 0; j--) {
                if (board[i][j] == 0) continue;
                int k = j;
                while (k < SIZE - 1 && board[i][k + 1] == 0) {
                    board[i][k + 1] = board[i][k];
                    board[i][k] = 0;
                    k++;
                }
                while (k < SIZE - 1 && board[i][k + 1] == board[i][k]) {
                    board[i][k + 1] *= 2;
                    board[i][k] = 0;
                    k++;
                }
            }
        }
    }

    // check if the board is filled
    int zero_count = 0;
    for (int i = 0; i < SIZE; i++) {
        for (int j = 0; j < SIZE; j++) {
            if (board[i][j] == 0) {
                zero_count++;
            }
        }
    }

    if (zero_count == 0) {
        printf("Game Over\n");
        exit(0);
    }

    // add a new number to a empty cell
    for (int i = 0; i < SIZE; i++) {
        for (int j = 0; j < SIZE; j++) {
            if (board[i][j] == 0) {
                board[i][j] = 2;
                return;
            }
        }
    }
}

void update(int turn, int board[SIZE][SIZE]) {
    draw_board(board);
    printf("Turn: %d\n", turn);

    // user input
    printf("w/a/s/d to move, q to quit: ");
    printf("\e[8m");  // hide input character
    char input = '\0';
    while (input == '\0') {
        input = getchar();
    }
    printf("\e[0m");  // show input character

    switch (input) {
        case 'w':
        case 'a':
        case 's':
        case 'd':
            update_board(input, board);
            break;

        case 'q':
        case 0x03:  // Ctrl-C
            printf("Exiting game.\n");
            exit(0);

        default:
            update(turn, board);
            break;
    }
}

int main(int argc, char *argv[]) {
    int turn = 1;
    int board[SIZE][SIZE] = {
        {0, 2, 0, 0},
        {0, 0, 4, 0},
        {0, 0, 0, 0},
        {2, 0, 0, 0}};

    for (;;) {
        update(turn, board);
        turn++;
    }

    return 0;
}
