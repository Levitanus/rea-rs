#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct rea_wdl_host rea_wdl_host;
typedef struct rea_wdl_control rea_wdl_control;
typedef void (*rea_wdl_command_callback)(void *context, int command, intptr_t p1, intptr_t p2, int source_id);

enum rea_wdl_control_kind {
  REA_WDL_ICON_BUTTON = 0,
  REA_WDL_STATIC_TEXT = 1,
  REA_WDL_COMBO_BOX = 2,
  REA_WDL_SLIDER = 3,
  REA_WDL_LIST_BOX = 4
};

rea_wdl_host *rea_wdl_host_create(rea_wdl_command_callback callback, void *context);
void rea_wdl_host_destroy(rea_wdl_host *host);
void *rea_wdl_host_context(rea_wdl_host *host);
void rea_wdl_host_set_real_parent(rea_wdl_host *host, void *hwnd);
rea_wdl_control *rea_wdl_control_create(rea_wdl_host *host, int kind, int id);
void rea_wdl_control_set_icon_skin(rea_wdl_control *control, void *frame_strip, void *overlay);
void rea_wdl_control_set_slider_skin(rea_wdl_control *control, void *horizontal_image, void *vertical_image);
void rea_wdl_control_set_rect(rea_wdl_control *control, int x, int y, int width, int height);
void rea_wdl_control_set_visible(rea_wdl_control *control, int visible);
void rea_wdl_control_set_text(rea_wdl_control *control, const char *text);
void rea_wdl_control_set_enabled(rea_wdl_control *control, int enabled);
void rea_wdl_control_set_check_state(rea_wdl_control *control, int checked);
void rea_wdl_control_set_range(rea_wdl_control *control, int minimum, int maximum, int center);
void rea_wdl_control_set_value(rea_wdl_control *control, int value);
int rea_wdl_control_get_value(rea_wdl_control *control);
void rea_wdl_control_get_range(rea_wdl_control *control, int *minimum, int *maximum, int *center);
void rea_wdl_control_set_text_align(rea_wdl_control *control, int align);
void rea_wdl_control_set_button_style(rea_wdl_control *control, int is_button, int immediate);
void rea_wdl_control_set_list_clicked_message(rea_wdl_control *control, int command);
void rea_wdl_control_set_list_double_click_message(rea_wdl_control *control, int command);
int rea_wdl_control_add_item(rea_wdl_control *control, const char *text);
void rea_wdl_control_set_selection(rea_wdl_control *control, int index);
int rea_wdl_control_get_selection(rea_wdl_control *control);
int rea_wdl_control_get_item(rea_wdl_control *control, int index, char *buffer, int capacity);
int rea_wdl_control_set_list_row_height(rea_wdl_control *control, int height);
int rea_wdl_control_id(rea_wdl_control *control);
int rea_wdl_host_mouse_down(rea_wdl_host *host, int x, int y);
void rea_wdl_host_mouse_move(rea_wdl_host *host, int x, int y);
void rea_wdl_host_mouse_up(rea_wdl_host *host, int x, int y);
int rea_wdl_host_mouse_double_click(rea_wdl_host *host, int x, int y);
int rea_wdl_host_mouse_wheel(rea_wdl_host *host, int x, int y, int delta);
void rea_wdl_host_capture_lost(rea_wdl_host *host);
void rea_wdl_host_paint(rea_wdl_host *host, void *lice_bitmap, int width, int height, int clip_left, int clip_top, int clip_right, int clip_bottom);

#ifdef __cplusplus
}
#endif
