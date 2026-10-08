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
#[derive(Clone, Copy, Debug)]
pub(super) struct HostWindowProc {
    previous: isize,
    installed: isize,
}

fn resolve_window_route(
    routes: &HashMap<usize, String>,
    mut hwnd: raw::HWND,
    mut parent_of: impl FnMut(raw::HWND) -> raw::HWND,
) -> Option<(String, bool)> {
    let original = hwnd as usize;
    if let Some(owner) = routes.get(&original) {
        return Some((owner.clone(), true));
    }
    while !hwnd.is_null() {
        if let Some(owner) = routes.get(&(hwnd as usize)) {
            return Some((owner.clone(), false));
        }
        hwnd = parent_of(hwnd);
    }
    None
}

#[cfg(test)]
mod route_tests {
    use super::resolve_window_route;
    use rea_rs_low::raw;
    use std::collections::HashMap;

    fn handle(value: usize) -> raw::HWND {
        value as raw::HWND
    }

    #[test]
    fn route_resolution_prefers_direct_then_walks_ancestors() {
        let mut routes = HashMap::new();
        routes.insert(10, "main".to_owned());
        routes.insert(30, "other".to_owned());
        let parents = HashMap::from([(20, 10), (40, 30)]);

        assert_eq!(
            resolve_window_route(&routes, handle(10), |_| std::ptr::null_mut(
            )),
            Some(("main".to_owned(), true)),
        );
        assert_eq!(
            resolve_window_route(&routes, handle(20), |hwnd| {
                handle(*parents.get(&(hwnd as usize)).unwrap_or(&0))
            }),
            Some(("main".to_owned(), false)),
        );
        assert_eq!(
            resolve_window_route(&routes, handle(99), |_| std::ptr::null_mut(
            )),
            None,
        );
    }

    #[test]
    fn minimum_track_size_does_not_shrink_native_minimums() {
        let mut info = [raw::POINT { x: 0, y: 0 }; 5];
        info[3] = raw::POINT { x: 400, y: 300 };
        info[4] = raw::POINT { x: 1800, y: 1200 };
        super::enforce_min_track_size(&mut info, (320, 240));
        assert_eq!(info[3].x, 400);
        assert_eq!(info[3].y, 300);
        assert_eq!(info[4].x, 1800);
        assert_eq!(info[4].y, 1200);
    }
}

fn dispatch_owned_command_detached(
    key: &str,
    hwnd: raw::HWND,
    command: super::events::WindowCommand,
    control_event: Option<super::events::ControlEvent>,
) -> bool {
    let Some(mut handler) = Reaper::get_mut().windows.remove(key) else {
        return false;
    };
    if !handler.window().is_owned() || handler.window().hwnd() != hwnd {
        Reaper::get_mut().windows.insert(key.to_owned(), handler);
        return false;
    }
    Reaper::get_mut().begin_window_dispatch(key);

    let generation = handler.window().lifecycle_generation.get();
    log::debug!(
        "owned command callback detached: hwnd={hwnd:?} window_id={key:?} command={command:?} generation={generation}",
    );
    if let super::events::WindowCommand::Control { id, notification } = command
    {
        let registered_control = handler.window().control(id);
        if control_event.is_none() {
            log::trace!(
                "owned WM_COMMAND did not decode to a control event: hwnd={hwnd:?} control_id={id:?} notification={notification:?} registered_control={registered_control:?}",
            );
        }
    }
    let _ = super::windows::invoke_handler_callback(
        handler.as_mut(),
        "owned window command",
        |handler| handler.on_command(command),
    );
    if let Some(event) = control_event {
        let result = handler.window().dispatch_control_event(event);
        log::debug!(
            "detached control event dispatch: event={event:?} result={result:?}"
        );
        if matches!(result, super::events::DispatchResult::ForwardToWindow) {
            let _ = super::windows::invoke_handler_callback(
                handler.as_mut(),
                "owned window control event",
                |handler| handler.on_control_event(event),
            );
        }
    }

    if unsafe { Reaper::get().swell().IsWindow(hwnd) }
        && handler.window().hwnd() == hwnd
    {
        let reaper = Reaper::get_mut();
        if let std::collections::hash_map::Entry::Vacant(entry) =
            reaper.windows.entry(key.to_owned())
        {
            entry.insert(handler);
            reaper.finish_window_dispatch(key);
        } else {
            log::error!(
                "owned window handler could not be restored after command callback: window_id={key:?} hwnd={hwnd:?}; preserving existing registration"
            );
            handler.window().relinquish_native_ownership();
            let _ = super::windows::invoke_handler_cleanup_callback(
                handler.as_mut(),
                "owned window destroy",
                |handler| handler.on_destroy(),
            );
            Reaper::get_mut().finish_window_dispatch(key);
        }
    } else {
        log::warn!(
            "owned window destroyed during command callback: window_id={key:?} hwnd={hwnd:?}"
        );
        handler.window().destroy_structural_children();
        handler.window().relinquish_native_ownership();
        let _ = super::windows::invoke_handler_cleanup_callback(
            handler.as_mut(),
            "owned window destroy",
            |handler| handler.on_destroy(),
        );
        Reaper::get_mut().finish_window_dispatch(key);
    }
    true
}

pub(super) static HOST_WINDOW_PROCS: OnceLock<
    Mutex<HashMap<usize, HostWindowProc>>,
> = OnceLock::new();

fn enforce_min_track_size(points: &mut [raw::POINT; 5], minimum: (i32, i32)) {
    let min_track_size = &mut points[3];
    min_track_size.x = min_track_size.x.max(minimum.0);
    min_track_size.y = min_track_size.y.max(minimum.1);
}

fn restore_borrowed_handler(
    key: &str,
    hwnd: raw::HWND,
    mut handler: Box<dyn super::windows::WindowHandler>,
) {
    if unsafe { Reaper::get().swell().IsWindow(hwnd) }
        && handler.window().hwnd() == hwnd
    {
        let reaper = Reaper::get_mut();
        if let std::collections::hash_map::Entry::Vacant(entry) =
            reaper.windows.entry(key.to_owned())
        {
            entry.insert(handler);
        } else {
            let _ = super::windows::invoke_handler_cleanup_callback(
                handler.as_mut(),
                "borrowed window destroy",
                |handler| handler.on_destroy(),
            );
        }
        reaper.finish_window_dispatch(key);
        return;
    }
    let _ = super::windows::invoke_handler_cleanup_callback(
        handler.as_mut(),
        "borrowed window destroy",
        |handler| handler.on_destroy(),
    );
    Reaper::get_mut().finish_window_dispatch(key);
}

pub(crate) fn remember_host_proc(hwnd: raw::HWND, previous: isize) {
    // SWELL's SetWindowLong is allowed to install its own dispatcher instead
    // of leaving our callback as the literal HWND procedure. Preserve the
    // procedure returned here for fallback/chaining, but don't assume this is
    // necessarily what a later GetWindowLong will report.
    let installed = if Reaper::is_available()
        && unsafe { Reaper::get().swell().IsWindow(hwnd) }
    {
        Some(unsafe {
            Reaper::get().swell().GetWindowLong(hwnd, raw::GWL_WNDPROC)
        })
    } else {
        None
    };
    let installed =
        installed.unwrap_or(window_proc as *const () as usize as isize);
    HOST_WINDOW_PROCS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("host window procedure registry poisoned")
        .insert(
            hwnd as usize,
            HostWindowProc {
                previous,
                installed,
            },
        );
    log::debug!(
        "host WndProc bound: hwnd={hwnd:p} parent={:p} saved_previous={previous:#x} installed_after_bind={installed:#x} wrapper={:#x}",
        unsafe { Reaper::get().swell().GetParent(hwnd) },
        window_proc as *const () as usize,
    );
}

pub(crate) fn detach_host_proc(hwnd: raw::HWND) {
    let procedures = HOST_WINDOW_PROCS
        .get()
        .and_then(|registry| registry.lock().ok()?.remove(&(hwnd as usize)));
    let Some(HostWindowProc {
        previous,
        installed,
    }) = procedures
    else {
        log::debug!(
            "host WndProc detach skipped: hwnd={hwnd:p} no saved procedure"
        );
        return;
    };
    let mut current = None;
    let mut restored = false;
    let mut previous_after_set = None;
    if Reaper::is_available()
        && unsafe { Reaper::get().swell().IsWindow(hwnd) }
    {
        let swell = Reaper::get().swell();
        let proc = unsafe { swell.GetWindowLong(hwnd, raw::GWL_WNDPROC) };
        current = Some(proc);
        let wrapper = window_proc as *const () as usize as isize;
        let owned_by_us = proc == wrapper;
        // Under SWELL, SetWindowLong may install a platform dispatcher which
        // calls our registered callback, while GetWindowLong reports that
        // dispatcher rather than `window_proc`. Compare against the exact
        // procedure observed immediately after installation, not the callback
        // function pointer. Never clobber a later third-party subclass.
        if proc == previous {
            // The host already restored its original procedure.
            restored = true;
        } else if owned_by_us || proc == installed {
            previous_after_set = Some(unsafe {
                swell.SetWindowLong(hwnd, raw::GWL_WNDPROC, previous)
            });
            let restored_proc =
                unsafe { swell.GetWindowLong(hwnd, raw::GWL_WNDPROC) };
            restored = restored_proc == previous;
            if !restored {
                log::warn!(
                    "host WndProc restore mismatch: hwnd={hwnd:p} expected={previous:#x} actual={restored_proc:#x} SetWindowLong_previous={previous_after_set:?}"
                );
            }
        } else {
            log::warn!(
                "host WndProc changed by another owner; leaving current proc intact: hwnd={hwnd:p} saved={previous:#x} installed={installed:#x} current={proc:#x} wrapper={wrapper:#x}"
            );
        }
    }
    log::debug!(
        "host WndProc detached: hwnd={hwnd:p} parent={:p} saved={previous:#x} installed={installed:#x} current={current:?} SetWindowLong_previous={previous_after_set:?} restored={restored}",
        if Reaper::is_available() {
            unsafe { Reaper::get().swell().GetParent(hwnd) }
        } else {
            std::ptr::null_mut()
        },
    );
}

pub(super) unsafe fn call_saved_host_proc(
    hwnd: raw::HWND,
    msg: raw::UINT,
    wparam: raw::WPARAM,
    lparam: raw::LPARAM,
) -> Option<raw::INT_PTR> {
    let previous = HOST_WINDOW_PROCS.get().and_then(|registry| {
        registry
            .lock()
            .ok()?
            .get(&(hwnd as usize))
            .map(|entry| entry.previous)
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
    if matches!(msg, raw::WM_CREATE | raw::WM_DESTROY | raw::WM_NCDESTROY) {
        log::trace!(
            "window_proc lifecycle callback: hwnd={hwnd:p} message={msg:#x} wparam={wparam:#x} lparam={lparam:#x}"
        );
    }
    if matches!(msg, raw::WM_SIZE | raw::WM_MOVE | raw::WM_SHOWWINDOW) {
        log::trace!(
            "window_proc geometry/visibility callback: hwnd={hwnd:p} message={msg:#x} wparam={wparam:#x} lparam={lparam:#x}"
        );
    }
    if !Reaper::is_available() {
        log::trace!(
            "window_proc ignored lifecycle callback because Reaper is unavailable: hwnd={hwnd:p} message={msg:#x}"
        );
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
        let key = if let Some(key) = direct_host.as_ref() {
            if matches!(
                msg,
                raw::WM_CREATE | raw::WM_DESTROY | raw::WM_NCDESTROY
            ) {
                log::trace!(
                    "window_proc resolved direct route: id={key:?} hwnd={hwnd:p} message={msg:#x}"
                );
            }
            key.clone()
        } else {
            let route =
                resolve_window_route(&reaper.window_routes, hwnd, |window| {
                    reaper.swell().GetParent(window)
                });
            let Some((key, _)) = route else {
                if matches!(
                    msg,
                    raw::WM_CREATE | raw::WM_DESTROY | raw::WM_NCDESTROY
                ) {
                    log::trace!(
                        "window_proc has no registered route for lifecycle callback: hwnd={hwnd:p} message={msg:#x}"
                    );
                }
                return reaper.swell().DefWindowProc(hwnd, msg, wparam, lparam)
                    as raw::INT_PTR;
            };
            if matches!(
                msg,
                raw::WM_CREATE | raw::WM_DESTROY | raw::WM_NCDESTROY
            ) {
                log::trace!(
                    "window_proc resolved ancestor route: id={key:?} hwnd={hwnd:p} message={msg:#x}"
                );
            }
            key
        };
        (direct_host, key)
    };
    // Process posted popup requests before taking mutable access to the
    // global REAPER instance. TrackPopupMenu runs a nested message loop, so
    // the popup must own an independent menu and must not retain a RefCell
    // borrow of the window's attached menu.
    if msg == super::windows::SHOW_MENU_POPUP_MESSAGE {
        let request_id = wparam;
        log::debug!(
            "popup request received: window_id={key:?} hwnd={hwnd:?} request_id={request_id} lifecycle_generation={:?}",
            Reaper::get()
                .windows
                .get(&key)
                .map(|handler| handler.window().lifecycle_generation.get()),
        );
        let request = Reaper::get()
            .windows
            .get(&key)
            .filter(|handler| {
                handler.window().hwnd() == hwnd
                    && unsafe { Reaper::get().swell().IsWindow(hwnd) }
            })
            .and_then(|handler| handler.window().popup_request.replace(None))
            .filter(|request| request.0 == request_id);
        if let Some((_, x, y, generation)) = request {
            log::debug!(
                "popup request validated: window_id={key:?} hwnd={hwnd:?} request_id={request_id} position=({x}, {y}) requested_generation={generation}",
            );
            let popup = Reaper::get().windows.get(&key).and_then(|handler| {
                if handler.window().lifecycle_generation.get() != generation {
                    log::warn!(
                        "popup request cancelled after window lifecycle change: window_id={key:?} hwnd={hwnd:?} request_generation={generation} current_generation={}",
                        handler.window().lifecycle_generation.get(),
                    );
                    return None;
                }
                handler.window().menu.borrow().as_ref()?.duplicate().ok()
            });
            if let Some(popup) = popup {
                let popup_owner = Reaper::get()
                    .low()
                    .pointers()
                    .GetMainHwnd
                    .as_ref()
                    .map(|get_main_hwnd| get_main_hwnd())
                    .filter(|owner| {
                        !owner.is_null()
                            && unsafe {
                                Reaper::get().swell().IsWindow(*owner)
                            }
                    });
                let Some(popup_owner) = popup_owner else {
                    log::warn!(
                        "popup request cancelled: no valid REAPER main window; window_id={key:?} hwnd={hwnd:?}",
                    );
                    return 0;
                };
                log::debug!(
                    "tracking popup menu: window_id={key:?} hwnd={hwnd:?} owner={popup_owner:?} position=({x}, {y}) generation={generation}",
                );
                if let Some(super::widgets::SwellId(id)) =
                    popup.popup_at(popup_owner, x, y)
                {
                    let still_current = Reaper::get()
                        .windows
                        .get(&key)
                        .is_some_and(|handler| {
                            handler.window().hwnd() == hwnd
                                && handler.window().lifecycle_generation.get()
                                    == generation
                                && unsafe {
                                    Reaper::get().swell().IsWindow(hwnd)
                                }
                        });
                    if still_current {
                        log::debug!(
                            "popup command selected: window_id={key:?} hwnd={hwnd:?} command_id={id} generation={generation}; dispatching",
                        );
                        dispatch_owned_command_detached(
                            &key,
                            hwnd,
                            super::events::WindowCommand::Menu {
                                id: super::widgets::SwellId(id),
                            },
                            None,
                        );
                    } else {
                        log::warn!(
                            "popup command discarded after window lifecycle change: window_id={key:?} hwnd={hwnd:?} command_id={id} generation={generation}",
                        );
                    }
                } else {
                    log::debug!(
                        "popup dismissed without command: window_id={key:?} hwnd={hwnd:?} generation={generation}",
                    );
                }
            } else {
                log::warn!(
                    "popup request discarded: attached menu unavailable or request stale; window_id={key:?} hwnd={hwnd:?} generation={generation}",
                );
            }
        } else {
            log::warn!(
                "popup request discarded: mismatched, missing, or invalid request; window_id={key:?} hwnd={hwnd:?} request_id={request_id}",
            );
        }
        return 0;
    }
    // User callbacks can call host APIs such as DockWindowAddEx and
    // DockWindowRemove. Those APIs synchronously send messages back through
    // this WndProc. Detach the owned handler and end the global mutable Reaper
    // borrow before invoking callbacks so nested WndProc calls cannot alias
    // the same mutable registry or handler entry.
    let owned_command = msg == raw::WM_COMMAND
        && (wparam as usize & 0xffff) != raw::IDCANCEL as usize
        && Reaper::get()
            .windows
            .get(&key)
            .is_some_and(|handler| handler.window().is_owned());
    if owned_command {
        let id = (wparam as usize & 0xffff) as i32;
        let code = ((wparam as usize >> 16) & 0xffff) as i32;
        let command = if lparam == 0 {
            super::events::WindowCommand::Menu {
                id: super::widgets::SwellId(id as u32),
            }
        } else {
            super::events::WindowCommand::Control {
                id: super::widgets::SwellId(id as u32),
                notification: super::events::CommandNotification::from_raw(
                    code,
                ),
            }
        };
        let control_event = if lparam != 0 {
            Reaper::get().windows.get(&key).and_then(|handler| {
                let control_id = super::widgets::SwellId(id as u32);
                handler.window().control(control_id).and_then(|control| {
                    super::events::decode_control_event(
                        control.kind,
                        control_id,
                        super::events::CommandNotification::from_raw(code),
                    )
                })
            })
        } else {
            None
        };
        dispatch_owned_command_detached(&key, hwnd, command, control_event);
        return 0;
    }
    let is_borrowed_host = direct_host.is_some()
        && Reaper::get()
            .windows
            .get(&key)
            .is_some_and(|handler| !handler.window().is_owned());
    if is_borrowed_host {
        let Some(mut handler) = Reaper::get_mut().windows.remove(&key) else {
            return call_saved_host_proc(hwnd, msg, wparam, lparam)
                .unwrap_or(0);
        };
        Reaper::get_mut().begin_window_dispatch(&key);
        let host_id = key.clone();
        if msg == raw::WM_DESTROY || msg == raw::WM_NCDESTROY {
            let previous = HOST_WINDOW_PROCS.get().and_then(|registry| {
                registry
                    .lock()
                    .ok()?
                    .get(&(hwnd as usize))
                    .map(|entry| entry.previous)
            });
            Reaper::get_mut().window_routes.remove(&(hwnd as usize));
            detach_host_proc(hwnd);
            let _ = super::windows::invoke_handler_cleanup_callback(
                handler.as_mut(),
                "borrowed window destroy",
                |handler| handler.on_destroy(),
            );
            drop(handler);
            Reaper::get_mut().finish_window_dispatch(&host_id);
            return previous
                .map(|proc| call_proc(proc, hwnd, msg, wparam, lparam))
                .unwrap_or(0);
        }
        if msg == raw::WM_CLOSE {
            Reaper::get_mut().window_routes.remove(&(hwnd as usize));
            let allow = super::windows::invoke_handler_close_callback(
                handler.as_mut(),
                "borrowed window close",
            );
            if unsafe { Reaper::get().swell().IsWindow(hwnd) } {
                Reaper::get_mut()
                    .window_routes
                    .insert(hwnd as usize, key.clone());
            } else {
                let _ = super::windows::invoke_handler_cleanup_callback(
                    handler.as_mut(),
                    "borrowed window destroy",
                    |handler| handler.on_destroy(),
                );
                drop(handler);
                detach_host_proc(hwnd);
                Reaper::get_mut().finish_window_dispatch(&key);
                return 0;
            }
            if allow {
                let result = call_saved_host_proc(hwnd, msg, wparam, lparam)
                    .unwrap_or(0);
                restore_borrowed_handler(&host_id, hwnd, handler);
                return result;
            }
            restore_borrowed_handler(&host_id, hwnd, handler);
            return 0;
        }
        let host_message_enabled = super::windows::invoke_handler_callback(
            handler.as_mut(),
            "borrowed window host message",
            |handler| handler.handle_host_message(msg),
        )
        .unwrap_or(false);
        if msg == raw::WM_PAINT {
            let result =
                call_saved_host_proc(hwnd, msg, wparam, lparam).unwrap_or(0);
            if host_message_enabled {
                let hdc = Reaper::get().swell().GetDC(hwnd);
                if !hdc.is_null() {
                    let mut client = std::mem::zeroed();
                    Reaper::get().swell().GetClientRect(hwnd, &mut client);
                    let info = PaintInfo {
                        damage_rect: Rect::from(client),
                        client_rect: Rect::from(client),
                    };
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
                    Reaper::get().swell().ReleaseDC(hwnd, hdc);
                }
            }
            restore_borrowed_handler(&host_id, hwnd, handler);
            return result;
        } else if host_message_enabled {
            let event = decode_window_event(hwnd, msg, wparam, lparam);
            if let Some(event) = event {
                if super::windows::invoke_handler_callback(
                    handler.as_mut(),
                    "borrowed window event",
                    |handler| handler.on_event(event),
                )
                .unwrap_or(false)
                {
                    restore_borrowed_handler(&host_id, hwnd, handler);
                    return 0;
                }
            }
        }
        let result =
            call_saved_host_proc(hwnd, msg, wparam, lparam).unwrap_or(0);
        restore_borrowed_handler(&host_id, hwnd, handler);
        return result;
    }
    let reaper = Reaper::get_mut();
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
                        (runtime.content == hwnd || runtime.clip == hwnd)
                            .then_some(runtime.id)
                    },
                )
            })
    });
    if msg == raw::WM_CAPTURECHANGED {
        let canvas_id = Reaper::get().windows.get(&key).and_then(|handler| {
            handler
                .window()
                .layout
                .borrow()
                .structural
                .iter()
                .find_map(|(id, child)| (*child == hwnd).then_some(*id))
        });
        if let (Some(id), Some(handler)) =
            (canvas_id, Reaper::get_mut().windows.get_mut(&key))
        {
            if let Ok(mut hosts) =
                handler.window().virtual_hosts.try_borrow_mut()
            {
                if let Some(host) = hosts.get_mut(&id) {
                    host.capture_lost();
                }
            }
            return 0;
        }
    }

    if let Some(super::events::WindowEvent::Focus(false)) =
        decode_window_event(hwnd, msg, wparam, lparam)
    {
        let canvas_id = Reaper::get().windows.get(&key).and_then(|handler| {
            handler
                .window()
                .layout
                .borrow()
                .structural
                .iter()
                .find_map(|(id, child)| (*child == hwnd).then_some(*id))
        });
        if let (Some(id), Some(handler)) =
            (canvas_id, Reaper::get_mut().windows.get_mut(&key))
        {
            if let Ok(mut hosts) =
                handler.window().virtual_hosts.try_borrow_mut()
            {
                if let Some(host) = hosts.get_mut(&id) {
                    host.capture_lost();
                }
            }
        }
    }

    if let Some(event) = decode_window_event(hwnd, msg, wparam, lparam) {
        if let Some(id) = widget_id {
            let canvas_id =
                Reaper::get().windows.get(&key).and_then(|handler| {
                    let window = handler.window();
                    if window
                        .virtual_hosts
                        .try_borrow()
                        .is_ok_and(|hosts| hosts.contains_key(&id))
                    {
                        Some(id)
                    } else {
                        window
                            .layout
                            .borrow()
                            .virtual_parents
                            .get(&id)
                            .copied()
                    }
                });
            if let Some(canvas_id) = canvas_id {
                let release_capture = matches!(
                    event,
                    super::events::WindowEvent::Mouse {
                        message: super::events::MouseMessage::Up(
                            super::events::MouseButton::Left
                        ),
                        ..
                    }
                );
                let point = match &event {
                    super::events::WindowEvent::Mouse { position, .. }
                    | super::events::WindowEvent::Wheel { position, .. } => {
                        Some(*position)
                    }
                    _ => None,
                };
                let handled = if let Some(handler) =
                    Reaper::get_mut().windows.get_mut(&key)
                {
                    let window = handler.window();
                    if let (Ok(mut hosts), Some(point)) =
                        (window.virtual_hosts.try_borrow_mut(), point)
                    {
                        let host = hosts.get_mut(&canvas_id);
                        if let Some(host) = host {
                            match &event {
                                super::events::WindowEvent::Mouse {
                                    message:
                                        super::events::MouseMessage::Down(
                                            super::events::MouseButton::Left,
                                        ),
                                    ..
                                } => host.mouse_down(
                                    point.x as i32,
                                    point.y as i32,
                                ),
                                super::events::WindowEvent::Mouse {
                                    message: super::events::MouseMessage::Move,
                                    ..
                                } => {
                                    host.mouse_move(
                                        point.x as i32,
                                        point.y as i32,
                                    );
                                    true
                                }
                                super::events::WindowEvent::Mouse {
                                    message:
                                        super::events::MouseMessage::Up(
                                            super::events::MouseButton::Left,
                                        ),
                                    ..
                                } => {
                                    host.mouse_up(
                                        point.x as i32,
                                        point.y as i32,
                                    );
                                    true
                                }
                                super::events::WindowEvent::Mouse {
                                    message:
                                        super::events::MouseMessage::DoubleClick(
                                            super::events::MouseButton::Left,
                                        ),
                                    ..
                                } => host.mouse_double_click(
                                    point.x as i32,
                                    point.y as i32,
                                ),
                                super::events::WindowEvent::Wheel {
                                    delta,
                                    ..
                                } => host.mouse_wheel(
                                    point.x as i32,
                                    point.y as i32,
                                    *delta,
                                ),
                                _ => false,
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };
                let acquire_capture = handled
                    && matches!(
                        event,
                        super::events::WindowEvent::Mouse {
                            message: super::events::MouseMessage::Down(
                                super::events::MouseButton::Left
                            ),
                            ..
                        }
                    );
                if acquire_capture {
                    Reaper::get().swell().SetCapture(hwnd);
                }
                if release_capture {
                    Reaper::get().swell().ReleaseCapture();
                }
                let commands = Reaper::get()
                    .windows
                    .get(&key)
                    .map(|handler| {
                        handler.window().dispatch_virtual_commands()
                    })
                    .unwrap_or_default();
                if let Some(handler) = Reaper::get_mut().windows.get_mut(&key)
                {
                    for command in commands {
                        let result =
                            handler.window().dispatch_control_event(command);
                        if matches!(
                            result,
                            super::events::DispatchResult::ForwardToWindow
                        ) {
                            let _ = super::windows::invoke_handler_callback(
                                handler.as_mut(),
                                "window virtual control event",
                                |handler| handler.on_control_event(command),
                            );
                        }
                    }
                }
                if handled {
                    return 0;
                }
            }
        }
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
            let virtual_events = handler.window().dispatch_virtual_commands();
            for virtual_event in virtual_events {
                let result =
                    handler.window().dispatch_control_event(virtual_event);
                if matches!(
                    result,
                    super::events::DispatchResult::ForwardToWindow
                ) {
                    let _ = super::windows::invoke_handler_callback(
                        handler.as_mut(),
                        "window virtual control event",
                        |handler| handler.on_control_event(virtual_event),
                    );
                }
            }
        }
        let Some(mut handler) = Reaper::get_mut().windows.remove(&key) else {
            return Reaper::get()
                .swell()
                .DefWindowProc(hwnd, msg, wparam, lparam)
                as raw::INT_PTR;
        };
        Reaper::get_mut().begin_window_dispatch(&key);
        let window_event_response = super::events::dispatch_window_event(
            handler.window(),
            event.clone(),
        );
        let handled = if window_event_response
            == super::events::EventResponse::Handled
        {
            true
        } else if let Some(id) = widget_id {
            super::windows::invoke_handler_callback(
                handler.as_mut(),
                "window widget event",
                |handler| handler.on_widget_event(id, event.clone()),
            )
            .unwrap_or(false)
                || super::windows::invoke_handler_callback(
                    handler.as_mut(),
                    "window event",
                    |handler| handler.on_event(event),
                )
                .unwrap_or(false)
        } else {
            super::windows::invoke_handler_callback(
                handler.as_mut(),
                "window event",
                |handler| handler.on_event(event),
            )
            .unwrap_or(false)
        };
        // The routed HWND may be a child widget (for example, SWELL sends a
        // focus event to the control that receives focus when the parent is
        // shown). Keep the owned top-level handler as long as its own HWND is
        // still alive; comparing it with the event's child HWND incorrectly
        // treats normal child input as destruction of the entire window.
        let owner_hwnd = handler.window().hwnd();
        if unsafe { Reaper::get().swell().IsWindow(owner_hwnd) } {
            if let std::collections::hash_map::Entry::Vacant(entry) =
                Reaper::get_mut().windows.entry(key.clone())
            {
                entry.insert(handler);
            } else {
                log::warn!(
                    "window handler could not be restored after event dispatch: window_id={key:?} event_hwnd={hwnd:?} owner_hwnd={owner_hwnd:?}; preserving existing registration and relinquishing duplicate ownership"
                );
                // A nested native callback may have installed a handler for
                // this logical ID while this handler was detached. Dropping
                // this stale handler must not destroy the HWND still used by
                // the surviving registration.
                handler.window().relinquish_native_ownership();
                let _ = super::windows::invoke_handler_cleanup_callback(
                    handler.as_mut(),
                    "window destroy after event dispatch",
                    |handler| handler.on_destroy(),
                );
            }
        } else {
            let _ = super::windows::invoke_handler_cleanup_callback(
                handler.as_mut(),
                "window destroy after event dispatch",
                |handler| handler.on_destroy(),
            );
        }
        Reaper::get_mut().finish_window_dispatch(&key);
        if handled {
            return 0;
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
            reaper.begin_window_dispatch(&key);
            reaper.window_routes.remove(&(hwnd as usize));
            let virtual_events = handler.window().dispatch_virtual_commands();
            for virtual_event in virtual_events {
                let result =
                    handler.window().dispatch_control_event(virtual_event);
                if matches!(
                    result,
                    super::events::DispatchResult::ForwardToWindow
                ) {
                    let _ = super::windows::invoke_handler_callback(
                        handler.as_mut(),
                        "window virtual control event",
                        |handler| handler.on_control_event(virtual_event),
                    );
                }
            }
            let allow = super::windows::invoke_handler_close_callback(
                handler.as_mut(),
                "window close",
            );
            if !unsafe { reaper.swell().IsWindow(hwnd) } {
                handler.window().relinquish_native_ownership();
                let _ = super::windows::invoke_handler_cleanup_callback(
                    handler.as_mut(),
                    "window destroy",
                    |handler| handler.on_destroy(),
                );
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
                    let _ = super::windows::invoke_handler_cleanup_callback(
                        handler.as_mut(),
                        "window destroy",
                        |handler| handler.on_destroy(),
                    );
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
            if cfg!(target_os = "linux") {
                // Generic SWELL destroys an attached menu after WM_DESTROY.
                // Relinquish the wrapper handle instead of calling
                // SetMenu(NULL) here, which could resize/re-enter this proc.
                if let Some(handler) = reaper.windows.get_mut(&key) {
                    if handler.window().is_owned() {
                        if let Ok(mut menu) =
                            handler.window().menu.try_borrow_mut()
                        {
                            if let Some(menu) = menu.as_mut() {
                                menu.relinquish_native_ownership();
                            }
                        }
                    }
                }
            } else if let Some(handler) = reaper.windows.get(&key) {
                if handler.window().is_owned() {
                    unsafe {
                        reaper.swell().SetMenu(hwnd, std::ptr::null_mut());
                    }
                }
            }
            let handler = reaper.windows.remove(&key);
            reaper.window_routes.remove(&(hwnd as usize));
            if let Some(mut handler) = handler {
                handler.window().destroy_structural_children();
                handler.window().relinquish_native_ownership();
                let _ = super::windows::invoke_handler_cleanup_callback(
                    handler.as_mut(),
                    "window destroy",
                    |handler| handler.on_destroy(),
                );
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
        // Detach the routed handler around commands above: callbacks may
        // synchronously redock, unregister, or destroy their own window.
        raw::WM_COMMAND => {
            let Some(mut handler) = reaper.windows.remove(&key) else {
                return 0;
            };
            reaper.begin_window_dispatch(&key);
            let id = (wparam as usize & 0xffff) as i32;
            let code = ((wparam as usize >> 16) & 0xffff) as i32;
            if id == raw::IDCANCEL as i32 && lparam == 0 {
                reaper.windows.insert(key.clone(), handler);
                reaper.finish_window_dispatch(&key);
                let _ = window_proc(hwnd, raw::WM_CLOSE, wparam, lparam);
                return 1;
            }
            let command = if lparam == 0 {
                super::events::WindowCommand::Menu {
                    id: super::widgets::SwellId(id as u32),
                }
            } else {
                super::events::WindowCommand::Control {
                    id: super::widgets::SwellId(id as u32),
                    notification: super::events::CommandNotification::from_raw(
                        code,
                    ),
                }
            };
            log::debug!("WM_COMMAND: hwnd={hwnd:?} id={id} code={code} command={command:?}");
            let registered_control = if lparam != 0 {
                handler.window().control(super::widgets::SwellId(id as u32))
            } else {
                None
            };
            if lparam != 0 && registered_control.is_none() {
                log::trace!(
                    "WM_COMMAND source is not registered as a control: hwnd={hwnd:?} id={id} code={code} source_hwnd={lparam:?} parent={:?}",
                    unsafe { reaper.swell().GetParent(lparam as raw::HWND) },
                );
            }
            let _ = super::windows::invoke_handler_callback(
                handler.as_mut(),
                "window command",
                |handler| handler.on_command(command),
            );
            if lparam != 0 {
                let control_id = super::widgets::SwellId(id as u32);
                let event = registered_control.and_then(|control| {
                    super::events::decode_control_event(
                        control.kind,
                        control_id,
                        super::events::CommandNotification::from_raw(code),
                    )
                });
                if let Some(event) = event {
                    let result =
                        handler.window().dispatch_control_event(event);
                    log::debug!("control event dispatch: event={event:?} result={result:?}");
                    if matches!(
                        result,
                        super::events::DispatchResult::ForwardToWindow
                    ) {
                        let _ = super::windows::invoke_handler_callback(
                            handler.as_mut(),
                            "window control event",
                            |handler| handler.on_control_event(event),
                        );
                    }
                } else {
                    log::trace!(
                        "WM_COMMAND did not decode to a control event: hwnd={hwnd:?} control_id={control_id:?} registered_control={registered_control:?} code={code}",
                    );
                }
            }
            if unsafe { reaper.swell().IsWindow(hwnd) }
                && handler.window().hwnd() == hwnd
            {
                if let std::collections::hash_map::Entry::Vacant(entry) =
                    reaper.windows.entry(key.clone())
                {
                    entry.insert(handler);
                    Reaper::get_mut().finish_window_dispatch(&key);
                } else {
                    handler.window().relinquish_native_ownership();
                    let _ = super::windows::invoke_handler_cleanup_callback(
                        handler.as_mut(),
                        "window destroy after reentrant command",
                        |handler| handler.on_destroy(),
                    );
                    Reaper::get_mut().finish_window_dispatch(&key);
                }
            } else {
                handler.window().destroy_structural_children();
                handler.window().relinquish_native_ownership();
                let _ = super::windows::invoke_handler_cleanup_callback(
                    handler.as_mut(),
                    "window destroy after command",
                    |handler| handler.on_destroy(),
                );
                Reaper::get_mut().finish_window_dispatch(&key);
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
                let before = runtime.state.borrow().offset();
                log::trace!(
                    "scrollbar input: hwnd={hwnd:?} view={:?} content={:?} id={:?} axis={axis:?} code={code} thumb_position={position} offset_before={before:?}",
                    runtime.view,
                    runtime.content,
                    runtime.id,
                );
                if let Some(command) =
                    super::scroll::decode_scroll_command(code, position)
                {
                    let mut state = runtime.state.borrow_mut();
                    *state = state.apply(axis, command);
                    let offset = state.offset();
                    let position =
                        if axis == Axis::X { offset.x } else { offset.y };
                    let page = if axis == Axis::X {
                        state.viewport().x
                    } else {
                        state.viewport().y
                    };
                    let max = if axis == Axis::X {
                        state.content().x
                    } else {
                        state.content().y
                    };
                    let visible = max > page;
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
                        if runtime.renderer
                            == super::scroll::ScrollbarRenderer::Native
                        {
                            let bar = if axis == Axis::X {
                                raw::SB_HORZ
                            } else {
                                raw::SB_VERT
                            };
                            let mut info = raw::SCROLLINFO {
                                cbSize: std::mem::size_of::<raw::SCROLLINFO>()
                                    as u32,
                                fMask: (raw::SIF_RANGE
                                    | raw::SIF_PAGE
                                    | raw::SIF_POS)
                                    as u32,
                                nMin: 0,
                                nMax: max.saturating_sub(1) as i32,
                                nPage: page,
                                nPos: position as i32,
                                nTrackPos: 0,
                            };
                            let _ = (*swell).set_native_scrollbar(
                                runtime.view,
                                bar as i32,
                                &mut info,
                                visible,
                            );
                        } else if Reaper::get()
                            .low()
                            .supports_cool_scrollbars()
                        {
                            let bar = if axis == Axis::X {
                                raw::SB_HORZ
                            } else {
                                raw::SB_VERT
                            };
                            let mut info = raw::SCROLLINFO {
                                cbSize: std::mem::size_of::<raw::SCROLLINFO>()
                                    as u32,
                                fMask: (raw::SIF_RANGE
                                    | raw::SIF_PAGE
                                    | raw::SIF_POS)
                                    as u32,
                                nMin: 0,
                                nMax: super::scroll::scrollbar_range_max(max)
                                    as i32,
                                nPage: page,
                                nPos: position as i32,
                                nTrackPos: 0,
                            };
                            Reaper::get().low().CoolSB_SetScrollInfo(
                                runtime.view,
                                bar as i32,
                                &mut info,
                                1,
                            );
                            Reaper::get().low().CoolSB_ShowScrollBar(
                                runtime.view,
                                bar as i32,
                                visible as i8,
                            );
                        }
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
                        "scroll view moved: hwnd={hwnd:?} view={:?} content={:?} id={:?} command={command:?} source={:?} offset={:?}",
                        runtime.view,
                        runtime.content,
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
                let event =
                    if control.kind == super::widgets::ControlKind::Trackbar {
                        super::events::ControlEvent::TrackbarChanged {
                            control: control.id,
                        }
                    } else {
                        super::events::ControlEvent::Scroll {
                            control: control.id,
                            code: (wparam as usize & 0xffff) as i32,
                        }
                    };
                let result = handler.window().dispatch_control_event(event);
                log::debug!("control scroll dispatch: event={event:?} result={result:?}");
                if matches!(
                    result,
                    super::events::DispatchResult::ForwardToWindow
                ) {
                    let _ = super::windows::invoke_handler_callback(
                        handler.as_mut(),
                        "window control event",
                        |handler| handler.on_control_event(event),
                    );
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
                    let before = state.offset();
                    *state = state.scroll_wheel(axis, delta);
                    let offset = state.offset();
                    log::trace!(
                        "scroll wheel input: hwnd={hwnd:?} view={parent:?} content={content:?} id={id:?} axis={axis:?} delta={delta} offset_before={before:?} offset_after={offset:?} viewport={:?} content_size={:?}",
                        state.viewport(),
                        state.content(),
                    );
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
                    drop(state);
                    // CoolSB is painted in the viewport's non-client area.
                    // Moving the oversized content child can cover those
                    // bars on SWELL, so force a scrollbar refresh afterward.
                    handler.window().sync_scroll_view(parent);
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
            let control_id = super::widgets::SwellId(header.id_from as u32);
            if let Some(control) = handler.window().control(control_id) {
                let event = super::events::decode_notify_event(
                    control.kind,
                    control_id,
                    header.code,
                );
                let result = handler.window().dispatch_control_event(event);
                log::debug!("control notification dispatch: event={event:?} result={result:?}");
                if matches!(
                    result,
                    super::events::DispatchResult::ForwardToWindow
                ) {
                    let _ = super::windows::invoke_handler_callback(
                        handler.as_mut(),
                        "window control event",
                        |handler| handler.on_control_event(event),
                    );
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
                                    (runtime.content == hwnd
                                        || runtime.clip == hwnd)
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
            let _ = super::windows::invoke_handler_callback(
                handler.as_mut(),
                "window resize",
                |handler| {
                    handler.on_resize(
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                    )
                },
            );
            0
        }
        raw::WM_GETMINMAXINFO => {
            let Some(handler) = reaper.windows.get(&key) else {
                return reaper.swell().DefWindowProc(hwnd, msg, wparam, lparam)
                    as raw::INT_PTR;
            };
            if handler.window().hwnd() != hwnd {
                return reaper.swell().DefWindowProc(hwnd, msg, wparam, lparam)
                    as raw::INT_PTR;
            }
            let Some((min_width, min_height)) = handler.window().min_size
            else {
                return reaper.swell().DefWindowProc(hwnd, msg, wparam, lparam)
                    as raw::INT_PTR;
            };
            if lparam == 0 {
                return reaper.swell().DefWindowProc(hwnd, msg, wparam, lparam)
                    as raw::INT_PTR;
            }
            let result =
                reaper.swell().DefWindowProc(hwnd, msg, wparam, lparam)
                    as raw::INT_PTR;
            // Win32/SWELL's MINMAXINFO is five POINTs. Only ptMinTrackSize is
            // changed, preserving any native maximum-size or position values.
            enforce_min_track_size(
                &mut *(lparam as *mut [raw::POINT; 5]),
                (min_width, min_height),
            );
            result
        }
        raw::WM_TIMER => {
            if let Some(handler) = reaper.windows.get_mut(&key) {
                let _ = super::windows::invoke_handler_callback(
                    handler.as_mut(),
                    "window timer",
                    |handler| {
                        handler
                            .on_timer(super::widgets::SwellId(wparam as u32))
                    },
                );
            }
            0
        }
        raw::WM_ACTIVATE => {
            if let Some(handler) = reaper.windows.get_mut(&key) {
                let _ = super::windows::invoke_handler_callback(
                    handler.as_mut(),
                    "window activation",
                    |handler| {
                        handler.on_activate((wparam as usize & 0xffff) != 0)
                    },
                );
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
    if matches!(
        msg,
        raw::WM_COMMAND | raw::WM_HSCROLL | raw::WM_VSCROLL | raw::WM_NOTIFY
    ) && Reaper::is_available()
    {
        let mut parent = Reaper::get().swell().GetParent(hwnd);
        while !parent.is_null() {
            if Reaper::get().window_id_for_hwnd(parent).is_some() {
                log::trace!(
                    "container native message forwarded: source={hwnd:?} parent={parent:?} message={msg:#x} wparam={wparam:#x} lparam={lparam:#x} id_or_code={} notification_or_position={}",
                    (wparam as usize & 0xffff),
                    ((wparam as usize >> 16) & 0xffff),
                );
                return window_proc(parent, msg, wparam, lparam);
            }
            parent = Reaper::get().swell().GetParent(parent);
        }
        log::trace!(
            "container native message has no registered window ancestor: source={hwnd:?} message={msg:#x} wparam={wparam:#x} lparam={lparam:#x}",
        );
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

#[cfg(test)]
mod sizing_tests {
    use super::enforce_min_track_size;
    use rea_rs_low::raw;

    #[test]
    fn minimum_window_size_only_raises_minimum_tracking_dimensions() {
        let mut info = [raw::POINT { x: 1, y: 2 }; 5];
        info[3] = raw::POINT { x: 320, y: 180 };
        info[4] = raw::POINT { x: 1600, y: 900 };

        enforce_min_track_size(&mut info, (400, 120));

        assert_eq!(info[3].x, 400);
        assert_eq!(info[3].y, 180);
        assert_eq!(info[4].x, 1600);
        assert_eq!(info[4].y, 900);
    }
}
