#ifndef _WINDOW_H
#define _WINDOW_H

#include <stddef.h>
#include <stdint.h>

#define PIXEL_FORMAT_RGB 0
#define PIXEL_FORMAT_BGR 1
#define PIXEL_FORMAT_BGRA 2

typedef struct
{
    int layer_id;
} component_descriptor;

int remove_component(component_descriptor* cdesc);
component_descriptor* create_component_window(const char* title, size_t x_pos, size_t y_pos, size_t width, size_t height);
component_descriptor* create_component_image(component_descriptor* cdesc, size_t image_width, size_t image_height, uint8_t pixel_format, const void* framebuf);

#endif
