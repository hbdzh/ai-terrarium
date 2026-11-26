#![cfg(target_os = "windows")]

use std::cell::Cell;
use std::sync::OnceLock;
use tauri::window::{Effect, EffectsBuilder};
use tauri::{Emitter, WebviewWindow};
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{BeginPaint, ClientToScreen, EndPaint, HBRUSH, PAINTSTRUCT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    TrackMouseEvent, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT,
};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetClientRect, GetCursorPos, GetWindowLongPtrW,
    GetWindowRect, IsZoomed, RegisterClassExW, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    GWL_STYLE, GWLP_USERDATA, HCURSOR, HICON, HTMAXBUTTON, HWND_TOP, SM_CYFRAME, STYLESTRUCT,
    SWP_ASYNCWINDOWPOS, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER,
    SWP_NOSIZE, SWP_NOZORDER, SW_MAXIMIZE, SW_RESTORE, WM_DPICHANGED, WM_ERASEBKGND,
    WM_NCHITTEST, WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE, WM_NCMOUSEMOVE, WM_NCDESTROY,
    WM_PAINT, WM_SHOWWINDOW, WM_SIZE, WM_STYLECHANGING, WM_WINDOWPOSCHANGED, WNDCLASSEXW,
    WNDCLASS_STYLES, WS_CAPTION, WS_CHILD, WS_CLIPSIBLINGS, WS_EX_NOREDIRECTIONBITMAP,
    WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME, WS_VISIBLE,
};
use windows_core::{w, Interface, PCWSTR};

const MAIN_SUBCLASS_ID: usize = 0x4149_5401;
const MAX_BTN_CLASS_NAME: PCWSTR = w!("AI_TERRARIUM_MAX_BTN_CLASS");
const MAX_BTN_WINDOW_NAME: PCWSTR = w!("AI_TERRARIUM_MAX_BTN");

const CAPTION_BTN_WIDTH_LOGICAL: f32 = 46.0;
const CAPTION_BTN_HEIGHT_LOGICAL: f32 = 36.0;

/// Window styles required by Windows 11 DWM / Explorer to enable Snap Layouts on HTMAXBUTTON.
/// `tao` keeps `WM_NCCALCSIZE -> 0` based on its internal `MARKER_DECORATIONS` flag, so keeping these
/// styles on `GWL_STYLE` does not paint native OS chrome while restoring native Win11 snap support.
const SNAP_REQUIRED_STYLES: u32 =
    WS_CAPTION.0 | WS_THICKFRAME.0 | WS_MAXIMIZEBOX.0 | WS_MINIMIZEBOX.0 | WS_SYSMENU.0;

static REGISTER_CLASS_ONCE: OnceLock<()> = OnceLock::new();

struct MainSubclassState {
    win: WebviewWindow,
    btn_hwnd: HWND,
    last_btn_rect: Cell<Option<(i32, i32, i32, i32)>>,
}

struct MaxBtnState {
    main_hwnd: HWND,
    win: WebviewWindow,
    hovered: bool,
}

#[inline]
fn get_x_lparam(lparam: LPARAM) -> i32 {
    ((lparam.0 & 0xFFFF) as i16) as i32
}

#[inline]
fn get_y_lparam(lparam: LPARAM) -> i32 {
    (((lparam.0 >> 16) & 0xFFFF) as i16) as i32
}

unsafe fn maximize_button_client_rect(main_hwnd: HWND) -> Option<(i32, i32, i32, i32)> {
    let mut client = RECT::default();
    if GetClientRect(main_hwnd, &mut client).is_err() {
        return None;
    }
    let dpi = GetDpiForWindow(main_hwnd);
    let scale = if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 };
    let btn_w = (CAPTION_BTN_WIDTH_LOGICAL * scale).round() as i32;
    let btn_h = (CAPTION_BTN_HEIGHT_LOGICAL * scale).round() as i32;

    // Leave top few pixels for Tauri's top-edge resize strip when unmaximized
    let top_inset = if IsZoomed(main_hwnd).as_bool() {
        0
    } else {
        GetSystemMetricsForDpi(SM_CYFRAME, dpi).clamp(2, 6)
    };

    let x = client.right - btn_w * 2;
    let y = top_inset;
    let w = btn_w;
    let h = (btn_h - top_inset).max(1);
    Some((x, y, w, h))
}

unsafe fn in_maximize_button_rect(main_hwnd: HWND, sx: i32, sy: i32) -> bool {
    let Some((x, y, w, h)) = maximize_button_client_rect(main_hwnd) else {
        return false;
    };
    let mut top_left = POINT { x: 0, y: 0 };
    if !ClientToScreen(main_hwnd, &mut top_left).as_bool() {
        return false;
    }
    let left = top_left.x + x;
    let top = top_left.y + y;
    sx >= left && sx < left + w && sy >= top && sy < top + h
}

unsafe fn ensure_snap_styles(hwnd: HWND) {
    let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
    if (style & WS_POPUP.0) != 0 {
        return;
    }
    if (style & SNAP_REQUIRED_STYLES) != SNAP_REQUIRED_STYLES {
        SetWindowLongPtrW(hwnd, GWL_STYLE, (style | SNAP_REQUIRED_STYLES) as isize);
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

unsafe fn sync_max_btn_overlay(main_hwnd: HWND, btn_hwnd: HWND) {
    if btn_hwnd.0.is_null() {
        return;
    }
    if let Some((x, y, w, h)) = maximize_button_client_rect(main_hwnd) {
        let _ = SetWindowPos(
            btn_hwnd,
            Some(HWND_TOP),
            x,
            y,
            w,
            h,
            SWP_ASYNCWINDOWPOS | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
        );
    }
}

unsafe extern "system" fn max_btn_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_ERASEBKGND => return LRESULT(1),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let _ = BeginPaint(hwnd, &mut ps);
            let _ = EndPaint(hwnd, &ps);
            return LRESULT(0);
        }
        WM_NCHITTEST => {
            return LRESULT(HTMAXBUTTON as isize);
        }
        WM_NCMOUSEMOVE => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MaxBtnState;
            if !ptr.is_null() {
                let state = &mut *ptr;
                if !state.hovered {
                    state.hovered = true;
                    let _ = state.win.emit("win-max-hover", true);
                }
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_NONCLIENT | TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                let _ = TrackMouseEvent(&mut tme);
            }
            return LRESULT(0);
        }
        WM_NCMOUSELEAVE => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MaxBtnState;
            if !ptr.is_null() {
                let state = &mut *ptr;
                if state.hovered {
                    state.hovered = false;
                    let _ = state.win.emit("win-max-hover", false);
                }
            }
            return LRESULT(0);
        }
        WM_NCLBUTTONDOWN => {
            if wparam.0 == HTMAXBUTTON as usize {
                return LRESULT(0);
            }
        }
        WM_NCLBUTTONUP => {
            if wparam.0 == HTMAXBUTTON as usize {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MaxBtnState;
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    state.hovered = false;
                    let _ = state.win.emit("win-max-hover", false);
                    let mut pt = POINT::default();
                    let mut rect = RECT::default();
                    if GetCursorPos(&mut pt).is_ok()
                        && GetWindowRect(hwnd, &mut rect).is_ok()
                        && pt.x >= rect.left
                        && pt.x < rect.right
                        && pt.y >= rect.top
                        && pt.y < rect.bottom
                    {
                        let cmd = if IsZoomed(state.main_hwnd).as_bool() {
                            SW_RESTORE
                        } else {
                            SW_MAXIMIZE
                        };
                        let _ = ShowWindow(state.main_hwnd, cmd);
                    }
                }
                return LRESULT(0);
            }
        }
        WM_NCDESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MaxBtnState;
            if !ptr.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                drop(Box::from_raw(ptr));
            }
        }
        _ => {}
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe extern "system" fn main_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid_subclass: usize,
    dw_ref_data: usize,
) -> LRESULT {
    match msg {
        WM_STYLECHANGING => {
            if (wparam.0 as i32) == GWL_STYLE.0 && lparam.0 != 0 {
                let ss = &mut *(lparam.0 as *mut STYLESTRUCT);
                if (ss.styleNew & WS_POPUP.0) == 0 {
                    ss.styleNew |= SNAP_REQUIRED_STYLES;
                }
            }
        }
        WM_NCHITTEST => {
            if dw_ref_data != 0 {
                let state = &*(dw_ref_data as *const MainSubclassState);
                if !state.btn_hwnd.0.is_null() {
                    let sx = get_x_lparam(lparam);
                    let sy = get_y_lparam(lparam);
                    let mut rect = RECT::default();
                    if GetWindowRect(state.btn_hwnd, &mut rect).is_ok()
                        && sx >= rect.left
                        && sx < rect.right
                        && sy >= rect.top
                        && sy < rect.bottom
                    {
                        return LRESULT(HTMAXBUTTON as isize);
                    }
                    return DefSubclassProc(hwnd, msg, wparam, lparam);
                }
            }
            let sx = get_x_lparam(lparam);
            let sy = get_y_lparam(lparam);
            if in_maximize_button_rect(hwnd, sx, sy) {
                return LRESULT(HTMAXBUTTON as isize);
            }
        }
        WM_SIZE | WM_WINDOWPOSCHANGED | WM_DPICHANGED | WM_SHOWWINDOW => {
            if dw_ref_data != 0 {
                let state = &*(dw_ref_data as *const MainSubclassState);
                if let Some(rect) = maximize_button_client_rect(hwnd) {
                    if state.last_btn_rect.get() != Some(rect) && !state.btn_hwnd.0.is_null() {
                        state.last_btn_rect.set(Some(rect));
                        let (x, y, w, h) = rect;
                        let _ = SetWindowPos(
                            state.btn_hwnd,
                            Some(HWND_TOP),
                            x,
                            y,
                            w,
                            h,
                            SWP_ASYNCWINDOWPOS | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
                        );
                    }
                }
            }
        }
        WM_NCLBUTTONDOWN => {
            if wparam.0 == HTMAXBUTTON as usize {
                return LRESULT(0);
            }
        }
        WM_NCLBUTTONUP => {
            if wparam.0 == HTMAXBUTTON as usize {
                let cmd = if IsZoomed(hwnd).as_bool() {
                    SW_RESTORE
                } else {
                    SW_MAXIMIZE
                };
                let _ = ShowWindow(hwnd, cmd);
                return LRESULT(0);
            }
        }
        WM_NCDESTROY => {
            if dw_ref_data != 0 {
                let state = Box::from_raw(dw_ref_data as *mut MainSubclassState);
                let _ = &state.win;
                drop(state);
            }
        }
        _ => {}
    }

    DefSubclassProc(hwnd, msg, wparam, lparam)
}

unsafe fn create_max_btn_overlay(main_hwnd: HWND, win: &WebviewWindow) -> HWND {
    let hinst = GetModuleHandleW(PCWSTR::null())
        .map(|h| HINSTANCE(h.0))
        .unwrap_or_default();

    REGISTER_CLASS_ONCE.get_or_init(|| {
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: WNDCLASS_STYLES::default(),
            lpfnWndProc: Some(max_btn_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: HICON::default(),
            hCursor: HCURSOR::default(),
            hbrBackground: HBRUSH::default(),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: MAX_BTN_CLASS_NAME,
            hIconSm: HICON::default(),
        };
        RegisterClassExW(&wc);
    });

    let (x, y, w, h) = maximize_button_client_rect(main_hwnd).unwrap_or((0, 0, 46, 36));
    let btn_hwnd = CreateWindowExW(
        WS_EX_NOREDIRECTIONBITMAP,
        MAX_BTN_CLASS_NAME,
        MAX_BTN_WINDOW_NAME,
        WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
        x,
        y,
        w,
        h,
        Some(main_hwnd),
        None,
        Some(hinst),
        None,
    )
    .unwrap_or_default();

    if !btn_hwnd.0.is_null() {
        let state_ptr = Box::into_raw(Box::new(MaxBtnState {
            main_hwnd,
            win: win.clone(),
            hovered: false,
        }));
        SetWindowLongPtrW(btn_hwnd, GWLP_USERDATA, state_ptr as isize);
        sync_max_btn_overlay(main_hwnd, btn_hwnd);
    }

    btn_hwnd
}

pub fn apply_windows_theme(win: &WebviewWindow, theme_pref: &str) {
    let (effect, tauri_theme) = match theme_pref {
        "dark" => (Effect::MicaDark, Some(tauri::Theme::Dark)),
        "light" => (Effect::MicaLight, Some(tauri::Theme::Light)),
        _ => (Effect::Mica, None),
    };
    let _ = win.set_theme(tauri_theme);
    let _ = win.set_effects(EffectsBuilder::new().effect(effect).build());
}

pub fn setup_windows_chrome(win: &WebviewWindow, theme_pref: &str) {
    let _ = win.set_decorations(false);
    let _ = win.set_shadow(true);
    apply_windows_theme(win, theme_pref);

    let btn_hwnd = if let Ok(hwnd) = win.hwnd() {
        unsafe {
            let btn = create_max_btn_overlay(hwnd, win);
            let boxed_state = Box::into_raw(Box::new(MainSubclassState {
                win: win.clone(),
                btn_hwnd: btn,
                last_btn_rect: Cell::new(None),
            }));
            let _ = SetWindowSubclass(
                hwnd,
                Some(main_subclass_proc),
                MAIN_SUBCLASS_ID,
                boxed_state as usize,
            );
            ensure_snap_styles(hwnd);
            btn
        }
    } else {
        HWND::default()
    };

    let btn_raw = btn_hwnd.0 as usize;
    let win_for_wv = win.clone();
    let _ = win.with_webview(move |wv| unsafe {
        if let Ok(core) = wv.controller().CoreWebView2() {
            if let Ok(settings) = core.Settings() {
                let _ = settings.SetAreDefaultContextMenusEnabled(false);
                if let Ok(settings3) = settings.cast::<ICoreWebView2Settings3>() {
                    let _ = settings3.SetAreBrowserAcceleratorKeysEnabled(false);
                }
            }
        }
        if let Ok(main_hwnd) = win_for_wv.hwnd() {
            ensure_snap_styles(main_hwnd);
            sync_max_btn_overlay(main_hwnd, HWND(btn_raw as *mut _));
        }
    });

    // Keep overlay above WebView2 & ensure styles stay intact after async tao/WebView2 init
    let win_retry = win.clone();
    tauri::async_runtime::spawn(async move {
        for delay_ms in [150_u64, 500_u64, 1200_u64] {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            let w = win_retry.clone();
            let _ = win_retry.run_on_main_thread(move || {
                if let Ok(main_hwnd) = w.hwnd() {
                    unsafe {
                        ensure_snap_styles(main_hwnd);
                        sync_max_btn_overlay(main_hwnd, HWND(btn_raw as *mut _));
                    }
                }
            });
        }
    });
}
