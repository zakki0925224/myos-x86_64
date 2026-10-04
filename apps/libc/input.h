#ifndef _INPUT_H
#define _INPUT_H

#include <stdint.h>

#define MOUSE_BUTTON_LEFT 0x1
#define MOUSE_BUTTON_RIGHT 0x2
#define MOUSE_BUTTON_MIDDLE 0x4

typedef struct {
    uint8_t buttons;
    uint8_t is_abs;
    uint8_t _reserved[2];
    int32_t x;
    int32_t y;
} mouse_event;

typedef struct {
    uint16_t code;
    uint8_t pressed;
    uint8_t _reserved;
    uint32_t c;
} key_event;

#endif
