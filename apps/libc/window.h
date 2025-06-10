#include <stddef.h>
#include <stdint.h>

#define PIXEL_FORMAT_RGB 0
#define PIXEL_FORMAT_BGR 1
#define PIXEL_FORMAT_BGRA 2

typedef struct
{
    int layer_id;
} ComponentDescriptor;

extern int remove_component(ComponentDescriptor *cdesc);
extern ComponentDescriptor *create_component_window(const char *title, size_t x_pos, size_t y_pos, size_t width, size_t height);
extern ComponentDescriptor *create_component_image(ComponentDescriptor *cdesc, size_t image_width, size_t image_height, uint8_t pixel_format, const void *framebuf);
