#include "virtual_controls.hpp"

#include <cstring>
#include <memory>
#include <string>
#include <vector>

#include "../lib/WDL/wingui/virtwnd-controls.h"
#include "../lib/WDL/lice/lice.h"

#ifdef min
#undef min
#endif
#ifdef max
#undef max
#endif

// WDL's built-in control implementations require these application style hooks.
int WDL_STYLE_WantGlobalButtonBorders() { return 0; }
bool WDL_STYLE_WantGlobalButtonBackground(int *) { return false; }
bool WDL_STYLE_GetBackgroundGradient(double *, double *) { return false; }
LICE_IBitmap *WDL_STYLE_GetSliderBitmap2(bool) { return nullptr; }
int WDL_STYLE_AllowSliderMouseWheel(WDL_VWnd *, double *) { return 1; }
bool WDL_STYLE_AllowSliderClickOutsideHandle(WDL_VWnd *) { return true; }
int WDL_STYLE_GetSliderDynamicCenterPos() { return 500; }

struct rea_wdl_control {
  WDL_VWnd *widget;
  int kind;
  std::vector<std::string> strings;
};

static int listbox_item_info(WDL_VirtualListBox *sender, int idx, char *nameout, int namelen, int *, WDL_VirtualWnd_BGCfg **) {
  auto *control = reinterpret_cast<rea_wdl_control *>(sender->GetUserData());
  if (!control) return 0;
  if (idx < 0) return static_cast<int>(control->strings.size());
  if (nameout && namelen > 0) {
    const char *text = idx < static_cast<int>(control->strings.size()) ? control->strings[idx].c_str() : "";
    strncpy(nameout, text, static_cast<size_t>(namelen - 1));
    nameout[namelen - 1] = 0;
  }
  return idx >= 0 && idx < static_cast<int>(control->strings.size());
}

struct rea_wdl_host : WDL_VWnd {
  rea_wdl_command_callback callback;
  void *context;
  std::vector<std::unique_ptr<rea_wdl_control>> controls;
  explicit rea_wdl_host(rea_wdl_command_callback cb, void *ctx) : callback(cb), context(ctx) {}
  INT_PTR SendCommand(int command, INT_PTR p1, INT_PTR p2, WDL_VWnd *source) override {
    if (callback) callback(context, command, p1, p2, source ? source->GetID() : -1);
    return 0;
  }
};

extern "C" {
rea_wdl_host *rea_wdl_host_create(rea_wdl_command_callback callback, void *context) {
  return new rea_wdl_host(callback, context);
}
void rea_wdl_host_destroy(rea_wdl_host *host) { delete host; }
void rea_wdl_host_set_real_parent(rea_wdl_host *host, void *hwnd) {
  if (host) host->SetRealParent(static_cast<HWND>(hwnd));
}
rea_wdl_control *rea_wdl_control_create(rea_wdl_host *host, int kind, int id) {
  if (!host || kind < REA_WDL_ICON_BUTTON || kind > REA_WDL_LIST_BOX) return nullptr;
  std::unique_ptr<rea_wdl_control> control(new rea_wdl_control{nullptr, kind, {}});
  switch (kind) {
    case REA_WDL_ICON_BUTTON: control->widget = new WDL_VirtualIconButton(); break;
    case REA_WDL_STATIC_TEXT: control->widget = new WDL_VirtualStaticText(); break;
    case REA_WDL_COMBO_BOX: control->widget = new WDL_VirtualComboBox(); break;
    case REA_WDL_SLIDER: control->widget = new WDL_VirtualSlider(); break;
    case REA_WDL_LIST_BOX: control->widget = new WDL_VirtualListBox(); break;
  }
  control->widget->SetID(id);
  if (kind == REA_WDL_LIST_BOX) {
    control->widget->SetUserData(reinterpret_cast<INT_PTR>(control.get()));
    static_cast<WDL_VirtualListBox *>(control->widget)->m_GetItemInfo = listbox_item_info;
    static_cast<WDL_VirtualListBox *>(control->widget)->SetClickedMessage(WM_USER + 101);
    static_cast<WDL_VirtualListBox *>(control->widget)->SetDroppedMessage(WM_USER + 102);
  }
  auto *result = control.get();
  host->AddChild(control->widget);
  host->controls.emplace_back(std::move(control));
  return result;
}
void rea_wdl_control_set_rect(rea_wdl_control *control, int x, int y, int width, int height) {
  if (!control || !control->widget) return;
  RECT rect{x, y, x + width, y + height}; control->widget->SetPosition(&rect);
}
void rea_wdl_control_set_visible(rea_wdl_control *control, int visible) { if (control) control->widget->SetVisible(visible != 0); }
void rea_wdl_control_set_text(rea_wdl_control *control, const char *text) {
  if (!control || !text) return;
  if (control->kind == REA_WDL_ICON_BUTTON) static_cast<WDL_VirtualIconButton *>(control->widget)->SetTextLabel(text);
  else if (control->kind == REA_WDL_STATIC_TEXT) static_cast<WDL_VirtualStaticText *>(control->widget)->SetText(text);
}
void rea_wdl_control_set_enabled(rea_wdl_control *control, int enabled) {
  if (!control) return;
  if (control->kind == REA_WDL_ICON_BUTTON) static_cast<WDL_VirtualIconButton *>(control->widget)->SetEnabled(enabled != 0);
  else if (control->kind == REA_WDL_SLIDER) static_cast<WDL_VirtualSlider *>(control->widget)->SetGrayed(enabled == 0);
  else if (control->kind == REA_WDL_LIST_BOX) static_cast<WDL_VirtualListBox *>(control->widget)->SetGrayed(enabled == 0);
}
void rea_wdl_control_set_check_state(rea_wdl_control *control, int checked) {
  if (control && control->kind == REA_WDL_ICON_BUTTON) static_cast<WDL_VirtualIconButton *>(control->widget)->SetCheckState(checked ? 1 : 0);
}
void rea_wdl_control_set_range(rea_wdl_control *control, int minimum, int maximum, int center) {
  if (control && control->kind == REA_WDL_SLIDER) static_cast<WDL_VirtualSlider *>(control->widget)->SetRange(minimum, maximum, center);
}
void rea_wdl_control_set_value(rea_wdl_control *control, int value) {
  if (control && control->kind == REA_WDL_SLIDER) static_cast<WDL_VirtualSlider *>(control->widget)->SetSliderPosition(value);
}
int rea_wdl_control_get_value(rea_wdl_control *control) {
  return control && control->kind == REA_WDL_SLIDER ? static_cast<WDL_VirtualSlider *>(control->widget)->GetSliderPosition() : 0;
}
int rea_wdl_control_id(rea_wdl_control *control) {
  return control && control->widget ? control->widget->GetID() : -1;
}
int rea_wdl_control_add_item(rea_wdl_control *control, const char *text) {
  if (!control || !text) return -1;
  control->strings.emplace_back(text);
  if (control->kind == REA_WDL_COMBO_BOX) return static_cast<WDL_VirtualComboBox *>(control->widget)->AddItem(text);
  if (control->kind == REA_WDL_LIST_BOX) return static_cast<int>(control->strings.size()) - 1;
  return -1;
}
void rea_wdl_control_set_selection(rea_wdl_control *control, int index) {
  if (!control) return;
  if (control->kind == REA_WDL_COMBO_BOX) static_cast<WDL_VirtualComboBox *>(control->widget)->SetCurSel(index);
}
int rea_wdl_control_get_selection(rea_wdl_control *control) {
  return control && control->kind == REA_WDL_COMBO_BOX ? static_cast<WDL_VirtualComboBox *>(control->widget)->GetCurSel() : -1;
}
int rea_wdl_host_mouse_down(rea_wdl_host *host, int x, int y) { return host ? host->OnMouseDown(x, y) : 0; }
void rea_wdl_host_mouse_move(rea_wdl_host *host, int x, int y) { if (host) host->OnMouseMove(x, y); }
void rea_wdl_host_mouse_up(rea_wdl_host *host, int x, int y) { if (host) host->OnMouseUp(x, y); }
int rea_wdl_host_mouse_double_click(rea_wdl_host *host, int x, int y) { return host && host->OnMouseDblClick(x, y); }
int rea_wdl_host_mouse_wheel(rea_wdl_host *host, int x, int y, int delta) { return host && host->OnMouseWheel(x, y, delta); }
void rea_wdl_host_capture_lost(rea_wdl_host *host) { if (host) host->OnCaptureLost(); }
void rea_wdl_host_paint(rea_wdl_host *host, void *bitmap, int width, int height, int left, int top, int right, int bottom) {
  if (!host || !bitmap || width <= 0 || height <= 0) return;
  RECT bounds{0, 0, width, height}, clip{left, top, right, bottom};
  host->OnPaint(static_cast<LICE_IBitmap *>(bitmap), 0, 0, &clip, WDL_VWND_SCALEBASE);
  host->OnPaintOver(static_cast<LICE_IBitmap *>(bitmap), 0, 0, &clip, WDL_VWND_SCALEBASE);
}
}
