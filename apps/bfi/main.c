#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define MEM_LEN 30000
#define STACK_LEN 32

static uint8_t mem[MEM_LEN] = {0};
static int stack[STACK_LEN] = {0};

int exec_bf(const char* bf_code) {
    int code_len = strlen(bf_code);
    char str[2];
    // instruction pointer
    int ip = 0;
    // memory pointer
    int mp = 0;
    // stack pointer
    int sp = 0;

    while (1) {
        if (code_len <= ip) {
            break;
        }

        switch (bf_code[ip]) {
            // increment pointed value
            case '+':
                if (mem[mp] == UINT8_MAX) {
                    printf("[ERR]Memory overflow\n");
                    return -1;
                }
                mem[mp]++;
                break;

            // decrement pointed value
            case '-':
                if (mem[mp] == 0) {
                    printf("[ERR]Memory underflow\n");
                    return -1;
                }
                mem[mp]--;
                break;

            // output pointed value to ascii
            case '.':
                str[0] = (char)mem[mp];
                str[1] = '\0';
                printf("%s", str);
                break;

            // increment pointer
            case '>':
                // printf(">");
                if (mp == MEM_LEN - 1) {
                    printf("[ERR]Memory pointer overflow\n");
                    return -1;
                }
                mp++;
                break;

            // decrement pointer
            case '<':
                if (mp == 0) {
                    printf("[ERR]Memory pointer underflow\n");
                    return -1;
                }
                mp--;
                break;

            // start loop
            case '[':
                stack[sp++] = ip;
                if (sp >= STACK_LEN) {
                    printf("[ERR]Stack overflow\n");
                    return -1;
                }

                if (mem[mp] == 0) {
                    while (1) {
                        ip++;
                        if (ip >= code_len) {
                            printf("[ERR]Unmatched '['\n");
                            return -1;
                        }
                        if (bf_code[ip] == ']') {
                            sp--;
                            break;
                        } else if (bf_code[ip] == '[') {
                            sp++;
                            break;
                        }
                    }
                }
                break;

            // end loop
            case ']':
                if (sp == 0) {
                    printf("[ERR]Unmatched ']'\n");
                    return -1;
                }

                if (mem[mp] != 0) {
                    ip = stack[sp - 1];
                } else {
                    sp--;
                }
                break;

            // skip
            case ' ':
                break;

            case ',':
                printf("[ERR]Unimplemented instruction\n");
                return -1;

            default:
                printf("[ERR]Invalid instruction\n");
                return -1;
        }

        ip++;
    }

    printf("\n");
    return 0;
}

int main(int argc, char const* argv[]) {
    const char* bf_code = "++ ++ ++ ++[ > ++ ++[ > ++ > ++ + > ++ + > + < < < < -] > + > + >->> +[ < ] < -] >>.> -- -.++ ++ ++ +..++ +.>>.<-.<.++ +.-- -- --.-- -- -- --.>> +.>++.";

    if (argc > 1) {
        bf_code = argv[1];
    }

    printf("Welcome to Brainf**k interpreter!\n");
    printf("code: \"%s\"\n", bf_code);
    if (exec_bf(bf_code) == -1) {
        return -1;
    }

    return 0;
}
