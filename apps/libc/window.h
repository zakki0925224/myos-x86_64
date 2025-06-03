#include <stddef.h>
#include <stdint.h>

#define PIXEL_FORMAT_RGB 0
#define PIXEL_FORMAT_BGR 1
#define PIXEL_FORMAT_BGRA 2

typedef struct
{
    int layer_id;
} WindowDescriptor;

extern WindowDescriptor *create_window(const char *title, size_t x_pos, size_t y_pos, size_t width, size_t height);
extern int destroy_window(WindowDescriptor *wdesc);
extern int add_image_to_window(WindowDescriptor *wdesc, size_t image_width, size_t image_height, uint8_t pixel_format, const void *framebuf);
