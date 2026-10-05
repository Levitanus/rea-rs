use std::{
    collections::HashMap,
    rc::Rc,
    sync::{Mutex, OnceLock},
};

use int_enum::IntEnum;
use rea_rs_low::raw;

use crate::{
    keys::VKeys,
    swell_gui::{
        drawing::HdcSurface,
        layout::{Axis, Rect},
        windows::{render_with_lice, PaintTransaction},
    },
    PaintInfo, Reaper, ScrollViewEvent, ScrollViewEventSource,
};

pub(super) static CONTAINER_WINDOW_PROCS: OnceLock<
    Mutex<HashMap<usize, isize>>,
> = OnceLock::new();
pub(super) static HOST_WINDOW_PROCS: OnceLock<Mutex<HashMap<usize, isize>>> =
    OnceLock::new();

pub(crate) fn remember_host_proc(hwnd: raw::HWND, previous: isize) {
    HOST_WINDOW_PROCS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("host window procedure registry poisoned")
        .insert(hwnd as usize, previous);
}

pub(crate) fn detach_host_proc(hwnd: raw::HWND) {
    let previous = HOST_WINDOW_PROCS.get().and_then(|registry| {
        registry.lock().ok()?.get(&(hwnd as usize)).copied()
    });
    let Some(previous) = previous else { return };
    if Reaper::is_available()
        && unsafe { Reaper::get().swell().IsWindow(hwnd) }
    {
        let swell = Reaper::get().swell();
        let current = unsafe { swell.GetWindowLong(hwnd, raw::GWL_WNDPROC) };
        if current == window_proc as *const () as usize as isize {
            unsafe { swell.SetWindowLong(hwnd, raw::GWL_WNDPROC, previous) };
        }
    }
    if let Some(registry) = HOST_WINDOW_PROCS.get() {
        if let Ok(mut registry) = registry.lock() {
            registry.remove(&(hwnd as usize));
        }
    }
}

pub(super) unsafe fn call_saved_host_proc(
    hwnd: raw::HWND,
    msg: raw::UINT,
    wparam: raw::WPARAM,
    lparam: raw::LPARAM,
) -> Option<raw::INT_PTR> {
    let previous = HOST_WINDOW_PROCS.get().and_then(|registry| {
        registry.lock().ok()?.get(&(hwnd as usize)).copied()
    })?;
    Some(call_proc(previous, hwnd, msg, wparam, lparam))
}

pub(super) unsafe fn call_proc(
    previous: isize,
    hwnd: raw::HWND,
    msg: raw::UINT,
    wparam: raw::WPARAM,
    lparam: raw::LPARAM,
) -> raw::INT_PTR {
    let previous: unsafe extern "C" fn(
        raw::HWND,
        raw::UINT,
        raw::WPARAM,
        raw::LPARAM,
    ) -> raw::INT_PTR = std::mem::transmute(previous as usize);
    previous(hwnd, msg, wparam, lparam)
}

pub(super) fn decode_window_event(
    hwnd: raw::HWND,
    msg: raw::UINT,
    wparam: raw::WPARAM,
    lparam: raw::LPARAM,
) -> Option<super::events::WindowEvent> {
    use super::events::{
        KeyMessage, KeyModifiers, MouseButton, MouseButtons, MouseMessage,
        NativeKey, WindowEvent,
    };
    let point = super::layout::Point {
        x: ((lparam as u32) as u16 as i16 as i32).max(0) as u32,
        y: ((lparam as u32 >> 16) as u16 as i16 as i32).max(0) as u32,
    };
    let mouse_buttons = MouseButtons::from_bits_truncate(wparam as u16);
    match msg {
        raw::WM_MOUSEMOVE => Some(WindowEvent::Mouse {
            message: MouseMessage::Move,
            position: point,
            buttons: mouse_buttons,
        }),
        raw::WM_LBUTTONDOWN
        | raw::WM_RBUTTONDOWN
        | raw::WM_MBUTTONDOWN
        | 0x020B => {
            let button = match msg {
                raw::WM_LBUTTONDOWN => MouseButton::Left,
                raw::WM_RBUTTONDOWN => MouseButton::Right,
                raw::WM_MBUTTONDOWN => MouseButton::Middle,
                _ if (wparam >> 16) as u16 == 1 => MouseButton::X1,
                _ => MouseButton::X2,
            };
            Some(WindowEvent::Mouse {
                message: MouseMessage::Down(button),
                position: point,
                buttons: mouse_buttons,
            })
        }
        raw::WM_LBUTTONUP | raw::WM_RBUTTONUP | raw::WM_MBUTTONUP | 0x020C => {
            let button = match msg {
                raw::WM_LBUTTONUP => MouseButton::Left,
                raw::WM_RBUTTONUP => MouseButton::Right,
                raw::WM_MBUTTONUP => MouseButton::Middle,
                _ if (wparam >> 16) as u16 == 1 => MouseButton::X1,
                _ => MouseButton::X2,
            };
            Some(WindowEvent::Mouse {
                message: MouseMessage::Up(button),
                position: point,
                buttons: mouse_buttons,
            })
        }
        raw::WM_LBUTTONDBLCLK
        | raw::WM_RBUTTONDBLCLK
        | raw::WM_MBUTTONDBLCLK
        | 0x020D => {
            let button = match msg {
                raw::WM_LBUTTONDBLCLK => MouseButton::Left,
                raw::WM_RBUTTONDBLCLK => MouseButton::Right,
                raw::WM_MBUTTONDBLCLK => MouseButton::Middle,
                _ if (wparam >> 16) as u16 == 1 => MouseButton::X1,
                _ => MouseButton::X2,
            };
            Some(WindowEvent::Mouse {
                message: MouseMessage::DoubleClick(button),
                position: point,
                buttons: mouse_buttons,
            })
        }
        raw::WM_MOUSEWHEEL | raw::WM_MOUSEHWHEEL => {
            let mut screen_point = raw::POINT {
                x: ((lparam as u32) as u16 as i16) as i32,
                y: ((lparam as u32 >> 16) as u16 as i16) as i32,
            };
            unsafe {
                Reaper::get()
                    .swell()
                    .ScreenToClient(hwnd, &mut screen_point)
            };
            Some(WindowEvent::Wheel {
                horizontal: msg == raw::WM_MOUSEHWHEEL,
                delta: ((wparam >> 16) & 0xffff) as i16 as i32,
                position: screen_point.into(),
            })
        }
        raw::WM_KEYDOWN
        | raw::WM_KEYUP
        | raw::WM_SYSKEYDOWN
        | raw::WM_SYSKEYUP => {
            let key = wparam as u32;
            Some(WindowEvent::Key {
                message: match msg {
                    raw::WM_KEYDOWN => KeyMessage::Down,
                    raw::WM_KEYUP => KeyMessage::Up,
                    raw::WM_SYSKEYDOWN => KeyMessage::SystemDown,
                    _ => KeyMessage::SystemUp,
                },
                key: VKeys::from_int(key)
                    .map(NativeKey::Known)
                    .unwrap_or(NativeKey::Other(key)),
                stroke: (lparam as isize).into(),
                modifiers: if matches!(
                    msg,
                    raw::WM_SYSKEYDOWN | raw::WM_SYSKEYUP
                ) {
                    KeyModifiers::ALT
                } else {
                    KeyModifiers::empty()
                },
            })
        }
        raw::WM_CHAR | raw::WM_SYSCHAR => {
            char::from_u32(wparam as u32).map(WindowEvent::Text)
        }
        raw::WM_SETFOCUS => Some(WindowEvent::Focus(true)),
        raw::WM_KILLFOCUS => Some(WindowEvent::Focus(false)),
        _ => None,
    }
}

/// The procedure installed on owned windows.
pub(crate) unsafe extern "C" fn window_proc(
    hwnd: raw::HWND,
    msg: raw::UINT,
    wparam: raw::WPARAM,
    lparam: raw::LPARAM,
) -> raw::INT_PTR {
    if !Reaper::is_available() {
        return 0;
    }
    // Window creation synchronously dispatches messages before its HWND is
    // registered. Resolve those with shared access only: taking the mutable
    // global Reaper reference here would alias the create_window caller.
    let (direct_host, key) = {
        let reaper = Reaper::get();
        // Structural ScrollView HWNDs use this procedure too. Resolve their
        // owner through the parent chain so messages reach the registered
        // top-level window.
        let direct_host = reaper.window_routes.get(&(hwnd as usize)).cloned();
        let mut current = hwnd;
        let mut key = direct_host.clone();
        while key.is_none() && !current.is_null() {
            if let Some(owner) = reaper.window_id_for_hwnd(current) {
                key = Some(owner);
                break;
            }
            current = reaper.swell().GetParent(current);
        }
        let Some(key) = key else {
            log::warn!("window message for unregistered HWND {:p}", hwnd);
            return reaper.swell().DefWindowProc(hwnd, msg, wparam, lparam)
                as raw::INT_PTR;
        };
        (direct_host, key)
    };
    let reaper = Reaper::get_mut();
    let is_borrowed_host = direct_host.is_some()
        && reaper
            .windows
            .get(&key)
            .is_some_and(|handler| !handler.window().is_owned());
    if is_borrowed_host {
        let host_message_enabled = reaper
            .windows
            .get(&key)
            .is_some_and(|handler| handler.handle_host_message(msg));
        let host_id = key.clone();
        if msg == raw::WM_DESTROY || msg == raw::WM_NCDESTROY {
            let previous = HOST_WINDOW_PROCS.get().and_then(|registry| {
                registry.lock().ok()?.get(&(hwnd as usize)).copied()
            });
            let handler = reaper.windows.remove(&key);
            reaper.window_routes.remove(&(hwnd as usize));
            detach_host_proc(hwnd);
            if let Some(mut handler) = handler {
                handler.on_destroy();
                drop(handler);
            }
            return previous
                .map(|proc| call_proc(proc, hwnd, msg, wparam, lparam))
                .unwrap_or(0);
        }
        if msg == raw::WM_CLOSE {
            let Some(mut handler) = reaper.windows.remove(&key) else {
                return call_saved_host_proc(hwnd, msg, wparam, lparam)
                    .unwrap_or(0);
            };
            reaper.window_routes.remove(&(hwnd as usize));
            let allow = handler.on_close();
            if unsafe { reaper.swell().IsWindow(hwnd) } {
                reaper.window_routes.insert(hwnd as usize, key.clone());
                reaper.windows.insert(key, handler);
            } else {
                handler.on_destroy();
                drop(handler);
                detach_host_proc(hwnd);
                return 0;
            }
            if allow {
                return call_saved_host_proc(hwnd, msg, wparam, lparam)
                    .unwrap_or(0);
            }
            return 0;
        }
        if msg == raw::WM_PAINT {
            let result =
                call_saved_host_proc(hwnd, msg, wparam, lparam).unwrap_or(0);
            if host_message_enabled {
                let hdc = reaper.swell().GetDC(hwnd);
                if !hdc.is_null() {
                    let mut client = std::mem::zeroed();
                    reaper.swell().GetClientRect(hwnd, &mut client);
                    let info = PaintInfo {
                        damage_rect: Rect::from(client),
                        client_rect: Rect::from(client),
                    };
                    if let Some(handler) =
                        Reaper::get_mut().windows.get_mut(&host_id)
                    {
                        if let Some(mut surface) =
                            HdcSurface::from_paint_hdc_with_swell(
                                hdc,
                                *Reaper::get().swell(),
                            )
                        {
                            render_with_lice(
                                handler.window(),
                                &info,
                                &mut surface,
                                None,
                            );
                        }
                    }
                    Reaper::get().swell().ReleaseDC(hwnd, hdc);
                }
            }
            return result;
        }
        if host_message_enabled {
            let event = decode_window_event(hwnd, msg, wparam, lparam);
            if let Some(event) = event {
                if Reaper::get_mut()
                    .windows
                    .get_mut(&host_id)
                    .is_some_and(|handler| handler.on_event(event))
                {
                    return 0;
                }
            }
        }
        return call_saved_host_proc(hwnd, msg, wparam, lparam).unwrap_or(0);
    }
    if let Some(event) = decode_window_event(hwnd, msg, wparam, lparam) {
        let widget_id = Reaper::get().windows.get(&key).and_then(|handler| {
            handler
                .window()
                .layout
                .borrow()
                .structural
                .iter()
                .find_map(|(id, child)| (*child == hwnd).then_some(*id))
                .or_else(|| {
                    handler.window().scroll_views.borrow().values().find_map(
                        |runtime| {
                            (runtime.content == hwnd).then_some(runtime.id)
                        },
                    )
                })
        });
        if matches!(
            &event,
            super::events::WindowEvent::Mouse {
                message: super::events::MouseMessage::Move
                    | super::events::MouseMessage::Wheel { .. },
                ..
            } | super::events::WindowEvent::Wheel { .. }
        ) {
            log::trace!("window input: hwnd={hwnd:?} widget={widget_id:?} event={event:?}");
        } else {
            log::debug!("window input: hwnd={hwnd:?} widget={widget_id:?} event={event:?}");
        }
        if let Some(handler) = Reaper::get_mut().windows.get_mut(&key) {
            if let Some(id) = widget_id {
                let handled = handler.on_widget_event(id, event.clone());
                if handled || handler.on_event(event) {
                    return 0;
                }
            } else if handler.on_event(event) {
                return 0;
            }
        }
    }
    let swell = reaper.swell() as *const rea_rs_low::Swell;
    match msg {
        raw::WM_CLOSE => {
            let is_top_level = reaper
                .windows
                .get(&key)
                .is_some_and(|handler| handler.window().hwnd() == hwnd);
            if !is_top_level {
                return reaper.swell().DefWindowProc(hwnd, msg, wparam, lparam)
                    as raw::INT_PTR;
            }
            let Some(mut handler) = reaper.windows.remove(&key) else {
                return 1;
            };
            reaper.window_routes.remove(&(hwnd as usize));
            let allow = handler.on_close();
            if !unsafe { reaper.swell().IsWindow(hwnd) } {
                handler.window().relinquish_native_ownership();
                handler.on_destroy();
                drop(handler);
                return 1;
            }
            reaper.window_routes.insert(hwnd as usize, key.clone());
            reaper.windows.insert(key.clone(), handler);
            if allow {
                unsafe { reaper.swell().DestroyWindow(hwnd) };
                // Normally WM_DESTROY has already removed the entry. Handle
                // backends that don't synchronously dispatch it as well.
                if let Some(mut handler) = reaper.windows.remove(&key) {
                    reaper.window_routes.remove(&(hwnd as usize));
                    handler.window().relinquish_native_ownership();
                    handler.on_destroy();
                    drop(handler);
                }
            }
            1
        }
        raw::WM_DESTROY => {
            let is_top_level = reaper
                .windows
                .get(&key)
                .is_some_and(|handler| handler.window().hwnd() == hwnd);
            if !is_top_level {
                return reaper.swell().DefWindowProc(hwnd, msg, wparam, lparam)
                    as raw::INT_PTR;
            }
            let handler = reaper.windows.remove(&key);
            reaper.window_routes.remove(&(hwnd as usize));
            if let Some(mut handler) = handler {
                handler.window().destroy_structural_children();
                handler.window().relinquish_native_ownership();
                handler.on_destroy();
                drop(handler);
            }
            if let Some(low) =
                Reaper::is_available().then(|| Reaper::get().low())
            {
                if low.pointers().DockWindowRemove.is_some() {
                    unsafe {
                        low.DockWindowRemove(hwnd);
                    }
                }
            }
            0
        }
        raw::WM_COMMAND => {
            let Some(handler) = reaper.windows.get_mut(&key) else {
                return 0;
            };
            let id = (wparam as usize & 0xffff) as i32;
            let code = ((wparam as usize >> 16) & 0xffff) as i32;
            if id == raw::IDCANCEL as i32 && lparam == 0 {
                let _ = handler;
                let _ = window_proc(hwnd, raw::WM_CLOSE, wparam, lparam);
                return 1;
            }
            let command = if lparam == 0 {
                super::events::WindowCommand::Menu { id }
            } else {
                super::events::WindowCommand::Control {
                    id,
                    notification: super::events::CommandNotification::from_raw(
                        code,
                    ),
                }
            };
            log::debug!("WM_COMMAND: hwnd={hwnd:?} id={id} code={code} command={command:?}");
            handler.on_command(command);
            if lparam != 0 {
                let control_id = super::widgets::ControlId(id);
                let event =
                    handler.window().control(control_id).and_then(|control| {
                        super::events::decode_control_event(
                            control.kind,
                            control_id,
                            super::events::CommandNotification::from_raw(code),
                        )
                    });
                if let Some(event) = event {
                    let result =
                        handler.window().events.borrow_mut().dispatch(event);
                    log::debug!("control event dispatch: event={event:?} result={result:?}");
                    if matches!(
                        result,
                        super::events::DispatchResult::ForwardToWindow
                    ) {
                        handler.on_control_event(event);
                    }
                }
            }
            0
        }
        raw::WM_HSCROLL | raw::WM_VSCROLL => {
            let Some(handler) = reaper.windows.get_mut(&key) else {
                return 0;
            };
            if let Some(runtime) =
                handler.window().scroll_views.borrow().get(&(hwnd as usize))
            {
                let axis = if msg == raw::WM_HSCROLL {
                    Axis::X
                } else {
                    Axis::Y
                };
                let code = (wparam as usize & 0xffff) as i32;
                let position = ((wparam as usize >> 16) & 0xffff) as u32;
                if let Some(command) =
                    super::scroll::decode_scroll_command(code, position)
                {
                    let mut state = runtime.state.borrow_mut();
                    *state = state.apply(axis, command);
                    let offset = state.offset();
                    unsafe {
                        (*swell).SetWindowPos(
                            runtime.content,
                            std::ptr::null_mut(),
                            -(offset.x as i32),
                            -(offset.y as i32),
                            0,
                            0,
                            (raw::SWP_NOSIZE
                                | raw::SWP_NOZORDER
                                | raw::SWP_NOACTIVATE)
                                as i32,
                        );
                    }
                    let source = match command {
                        super::scroll::ScrollCommand::LineBackward
                        | super::scroll::ScrollCommand::LineForward => {
                            ScrollViewEventSource::Line
                        }
                        super::scroll::ScrollCommand::PageBackward
                        | super::scroll::ScrollCommand::PageForward => {
                            ScrollViewEventSource::Page
                        }
                        super::scroll::ScrollCommand::ThumbTrack(_)
                        | super::scroll::ScrollCommand::ThumbPosition(_) => {
                            ScrollViewEventSource::Thumb
                        }
                        _ => ScrollViewEventSource::Programmatic,
                    };
                    let event = ScrollViewEvent { offset, source };
                    log::trace!(
                        "scroll view moved: id={:?} source={:?} offset={:?}",
                        runtime.id,
                        source,
                        offset,
                    );
                    let _ = handler
                        .window()
                        .emit_scroll_view_event(runtime.id, event);
                }
                return 0;
            }
            let control = handler
                .window()
                .controls
                .borrow()
                .get_by_hwnd(lparam as raw::HWND);
            if let Some(control) = control {
                let event = super::events::ControlEvent::Scroll {
                    control: control.id,
                    code: (wparam as usize & 0xffff) as i32,
                };
                let result =
                    handler.window().events.borrow_mut().dispatch(event);
                log::debug!("control scroll dispatch: event={event:?} result={result:?}");
                if matches!(
                    result,
                    super::events::DispatchResult::ForwardToWindow
                ) {
                    handler.on_control_event(event);
                }
            }
            0
        }
        raw::WM_MOUSEWHEEL | raw::WM_MOUSEHWHEEL => {
            let Some(handler) = reaper.windows.get_mut(&key) else {
                return 0;
            };
            let mut parent = hwnd;
            while !parent.is_null() {
                let runtime = handler
                    .window()
                    .scroll_views
                    .borrow()
                    .get(&(parent as usize))
                    .map(|runtime| {
                        (
                            runtime.id,
                            runtime.content,
                            Rc::clone(&runtime.state),
                        )
                    });
                if let Some((id, content, state)) = runtime {
                    let delta =
                        ((wparam as usize >> 16) & 0xffff) as i16 as i32;
                    let axis = if msg == raw::WM_MOUSEHWHEEL {
                        Axis::X
                    } else {
                        Axis::Y
                    };
                    let mut state = state.borrow_mut();
                    *state = state.scroll_wheel(axis, delta);
                    let offset = state.offset();
                    unsafe {
                        (*swell).SetWindowPos(
                            content,
                            std::ptr::null_mut(),
                            -(offset.x as i32),
                            -(offset.y as i32),
                            0,
                            0,
                            (raw::SWP_NOSIZE
                                | raw::SWP_NOZORDER
                                | raw::SWP_NOACTIVATE)
                                as i32,
                        );
                    }
                    let _ = handler.window().emit_scroll_view_event(
                        id,
                        ScrollViewEvent {
                            offset,
                            source: ScrollViewEventSource::Wheel,
                        },
                    );
                    return 0;
                }
                parent = (*swell).GetParent(parent);
            }
            0
        }
        raw::WM_NOTIFY => {
            let Some(handler) = reaper.windows.get_mut(&key) else {
                return 0;
            };
            if lparam == 0 {
                return 0;
            }
            let header = &*(lparam as *const super::events::NotifyHeader);
            let control_id = super::widgets::ControlId(header.id_from as i32);
            if handler.window().control(control_id).is_some() {
                let event = super::events::ControlEvent::Notified {
                    control: control_id,
                    code: header.code,
                };
                let result =
                    handler.window().events.borrow_mut().dispatch(event);
                log::debug!("control notification dispatch: event={event:?} result={result:?}");
                if matches!(
                    result,
                    super::events::DispatchResult::ForwardToWindow
                ) {
                    handler.on_control_event(event);
                }
            }
            0
        }
        raw::WM_PAINT => {
            let (top_level, child_id) = Reaper::get()
                .windows
                .get(&key)
                .map(|handler| {
                    let window = handler.window();
                    let id = window
                        .layout
                        .borrow()
                        .structural
                        .iter()
                        .find_map(|(id, child)| {
                            (*child == hwnd).then_some(*id)
                        })
                        .or_else(|| {
                            window.scroll_views.borrow().values().find_map(
                                |runtime| {
                                    (runtime.content == hwnd)
                                        .then_some(runtime.id)
                                },
                            )
                        });
                    (window.hwnd() == hwnd, id)
                })
                .unwrap_or((false, None));
            let mut transaction = PaintTransaction::begin(hwnd);
            if let Some((info, mut surface)) =
                transaction.as_mut().and_then(|tx| tx.surface())
            {
                let handler = Reaper::get_mut().windows.get_mut(&key);
                if let Some(handler) = handler {
                    if top_level {
                        render_with_lice(
                            handler.window(),
                            &info,
                            &mut surface,
                            None,
                        );
                    } else if let Some(id) = child_id {
                        render_with_lice(
                            handler.window(),
                            &info,
                            &mut surface,
                            Some(id),
                        );
                    }
                }
            }
            0
        }
        raw::WM_SIZE => {
            let is_top_level = reaper
                .windows
                .get(&key)
                .is_some_and(|handler| handler.window().hwnd() == hwnd);
            if !is_top_level {
                return reaper.swell().DefWindowProc(hwnd, msg, wparam, lparam)
                    as raw::INT_PTR;
            }
            let mut rect = std::mem::zeroed();
            Reaper::get().swell().GetClientRect(hwnd, &mut rect);
            log::trace!(
                "WM_SIZE: hwnd={hwnd:?} client={}x{}",
                rect.right - rect.left,
                rect.bottom - rect.top,
            );
            let Some(handler) = reaper.windows.get_mut(&key) else {
                return 0;
            };
            if let Err(error) = handler.window().apply_default_layout() {
                log::warn!("could not apply default window layout: {error}");
            }
            handler.on_resize(rect.right - rect.left, rect.bottom - rect.top);
            0
        }
        raw::WM_TIMER => {
            if let Some(handler) = reaper.windows.get_mut(&key) {
                handler.on_timer(wparam as usize);
            }
            0
        }
        raw::WM_ACTIVATE => {
            if let Some(handler) = reaper.windows.get_mut(&key) {
                handler.on_activate((wparam as usize & 0xffff) != 0);
            }
            0
        }
        _ => Reaper::get()
            .swell()
            .DefWindowProc(hwnd, msg, wparam, lparam)
            as raw::INT_PTR,
    }
}

/// Forwards commands from controls whose immediate parent is a native
/// container (currently GroupBox) to the owning top-level window procedure.
/// Native child controls send `WM_COMMAND` to that immediate parent, so the
/// top-level window procedure cannot observe them unless the container is
/// subclassed.
pub(super) unsafe extern "C" fn container_event_proc(
    hwnd: raw::HWND,
    msg: raw::UINT,
    wparam: raw::WPARAM,
    lparam: raw::LPARAM,
) -> raw::INT_PTR {
    if msg == raw::WM_COMMAND && Reaper::is_available() {
        let mut parent = Reaper::get().swell().GetParent(hwnd);
        while !parent.is_null() {
            if Reaper::get().window_id_for_hwnd(parent).is_some() {
                return window_proc(parent, msg, wparam, lparam);
            }
            parent = Reaper::get().swell().GetParent(parent);
        }
    }

    let previous = CONTAINER_WINDOW_PROCS.get().and_then(|registry| {
        registry.lock().ok()?.get(&(hwnd as usize)).copied()
    });
    if let Some(previous) = previous {
        let previous: unsafe extern "C" fn(
            raw::HWND,
            raw::UINT,
            raw::WPARAM,
            raw::LPARAM,
        ) -> raw::INT_PTR = std::mem::transmute(previous as usize);
        return previous(hwnd, msg, wparam, lparam);
    }

    Reaper::get()
        .swell()
        .DefWindowProc(hwnd, msg, wparam, lparam) as raw::INT_PTR
}
