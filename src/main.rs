#![windows_subsystem = "windows"]
#![allow(unsafe_op_in_unsafe_fn)]

mod overlay_renderer;

use std::ffi::c_void;
use std::iter::once;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::{
    BOOL, COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
};
use windows::Win32::Graphics::Dwm::{
    DWM_SYSTEMBACKDROP_TYPE, DWM_WINDOW_CORNER_PREFERENCE, DWMSBT_NONE, DWMWA_BORDER_COLOR,
    DWMWA_COLOR_NONE, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_HOSTBACKDROPBRUSH,
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, COLOR_WINDOW, CreateFontW, DEFAULT_CHARSET,
    DEFAULT_PITCH, DEFAULT_QUALITY, DT_LEFT, DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK, DeleteObject,
    DrawTextW, EndPaint, FF_DONTCARE, FW_MEDIUM, FW_SEMIBOLD, FillRect, GetMonitorInfoW,
    GetSysColorBrush, HBRUSH, HFONT, HGDIOBJ, InvalidateRect, MONITOR_DEFAULTTONEAREST,
    MONITORINFO, MonitorFromWindow, OUT_DEFAULT_PRECIS, PAINTSTRUCT, SelectObject, SetBkMode,
    SetTextColor, TRANSPARENT, UpdateWindow,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, KEY_WOW64_64KEY, REG_OPTION_NON_VOLATILE,
    REG_SZ, REG_VALUE_TYPE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegCloseKey, RegCreateKeyExW,
    RegDeleteValueW, RegGetValueW, RegOpenKeyExW, RegSetValueExW,
};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::UI::Controls::{BST_CHECKED, BST_UNCHECKED};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForSystem, GetDpiForWindow,
    SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_ALT, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey,
};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIN_SELECT, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AW_BLEND, AW_HIDE, AnimateWindow, AppendMenuW, BM_GETCHECK, BM_SETCHECK, BN_CLICKED,
    BS_AUTOCHECKBOX, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu,
    DestroyWindow, DispatchMessageW, DrawMenuBar, GA_ROOT, GetAncestor, GetClassNameW,
    GetClientRect, GetCursorPos, GetForegroundWindow, GetMessageW, GetSystemMetrics, GetWindowRect,
    GetWindowTextLengthW, GetWindowTextW, HICON, HMENU, IDC_ARROW, IDI_APPLICATION, IMAGE_ICON,
    IsIconic, IsWindow, IsWindowVisible, KillTimer, LR_DEFAULTSIZE, LoadCursorW, LoadIconW,
    LoadImageW, MB_ICONWARNING, MB_OK, MF_CHECKED, MF_GRAYED, MF_SEPARATOR, MF_STRING,
    MF_UNCHECKED, MSG, MessageBoxW, PostMessageW, PostQuitMessage, RegisterClassW, SM_CXSCREEN,
    SM_CYSCREEN, SW_HIDE, SW_MINIMIZE, SW_RESTORE, SW_SHOW, SW_SHOWNOACTIVATE, SWP_NOACTIVATE,
    SWP_SHOWWINDOW, SendMessageW, SetForegroundWindow, SetTimer, SetWindowPos, ShowWindow,
    TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
    TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_CLOSE, WM_COMMAND, WM_CONTEXTMENU,
    WM_CREATE, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_HOTKEY, WM_LBUTTONDBLCLK, WM_NULL,
    WM_PAINT, WM_RBUTTONUP, WM_SETTINGCHANGE, WM_SIZE, WM_THEMECHANGED, WM_TIMER, WNDCLASSW,
    WS_CHILD, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_MINIMIZEBOX, WS_OVERLAPPED,
    WS_POPUP, WS_SYSMENU, WS_VISIBLE,
};
use windows::core::{PCWSTR, Result, w};

use overlay_renderer::{OverlayTheme, OverlayVisual, discard_overlay_renderer, draw_overlay};

const APP_NAME: &str = "WinMac";
const MAIN_CLASS: PCWSTR = w!("WinMacMainWindow");
const OVERLAY_CLASS: PCWSTR = w!("WinMacOverlayWindow");
const WM_TRAYICON: u32 = WM_APP + 1;

const HOTKEY_HIDE_ID: i32 = 1;
const HOTKEY_QUIT_ID: i32 = 2;

const ID_CHECK_AUTOSTART: isize = 1001;
const ID_TRAY_TOGGLE_WINDOW: usize = 2001;
const ID_TRAY_RESTORE_LAST: usize = 2002;
const ID_TRAY_RESTORE_ALL: usize = 2003;
const ID_TRAY_AUTOSTART: usize = 2004;
const ID_TRAY_EXIT: usize = 2005;

const TIMER_OVERLAY_FADE_IN: usize = 3001;
const TIMER_OVERLAY_DELAY_HIDE: usize = 3002;
const TIMER_MINIMIZE_ANCHOR: usize = 3003;

const QUIT_CONFIRM_MS: u64 = 1600;
const MINIMIZE_ANCHOR_CAPTURE_MS: u64 = 40;
const OVERLAY_WIDTH: i32 = 336;
const OVERLAY_HEIGHT: i32 = 118;
const OVERLAY_LIFT: i32 = 8;
const OVERLAY_BOTTOM_MARGIN: i32 = 56;
const OVERLAY_TOP_MARGIN: i32 = 16;
const OVERLAY_VERTICAL_ANCHOR_PERCENT: i32 = 74;
const WINDOW_WIDTH: i32 = 420;
const WINDOW_HEIGHT: i32 = 250;
const RUN_KEY_PATH: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const PERSONALIZE_KEY_PATH: PCWSTR =
    w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
const RUN_VALUE_NAME: PCWSTR = w!("WinMac");
const LIGHT_THEME_VALUE_NAME: PCWSTR = w!("AppsUseLightTheme");

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ThemeMode {
    #[default]
    Light,
    Dark,
}

#[derive(Default)]
struct AppState {
    main_hwnd: HWND,
    overlay_hwnd: HWND,
    checkbox_hwnd: HWND,
    app_icon: HICON,
    overlay_icon: HICON,
    font_dpi: u32,
    overlay_dpi: u32,
    title_font: HFONT,
    body_font: HFONT,
    overlay_font: HFONT,
    overlay_x: i32,
    overlay_y: i32,
    overlay_target_y: i32,
    overlay_title: String,
    autostart_enabled: bool,
    theme: ThemeMode,
    exiting: bool,
    quit_target: HWND,
    quit_deadline: u64,
    hidden_windows: Vec<HWND>,
    last_minimized_window: HWND,
    last_minimize_anchor: HWND,
    pending_minimized_window: HWND,
    pending_minimize_deadline: u64,
}

fn main() -> Result<()> {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        {
            let mut state = app_state().lock().unwrap();
            state.theme = detect_theme();
            state.autostart_enabled = is_autostart_enabled();
        }

        let hmodule = GetModuleHandleW(None)?;
        let hinstance = HINSTANCE(hmodule.0);
        register_window_classes(hinstance)?;
        ensure_fonts(current_system_dpi());

        let main_hwnd = create_main_window(hinstance)?;
        let overlay_hwnd = create_overlay_window(hinstance)?;

        {
            let mut state = app_state().lock().unwrap();
            state.main_hwnd = main_hwnd;
            state.overlay_hwnd = overlay_hwnd;
            state.app_icon = load_app_icon(hinstance, 48);
            state.overlay_icon = state.app_icon;
        }

        refresh_theme();
        sync_autostart_checkbox();
        add_tray_icon(main_hwnd)?;
        register_hotkeys(main_hwnd);

        ShowWindow(main_hwnd, SW_SHOW);
        let _ = UpdateWindow(main_hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}

fn app_state() -> &'static Mutex<AppState> {
    static STATE: OnceLock<Mutex<AppState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(AppState::default()))
}

unsafe fn register_window_classes(hinstance: HINSTANCE) -> Result<()> {
    let cursor = LoadCursorW(None, IDC_ARROW)?;
    let icon = load_app_icon(hinstance, 48);

    let main_class = WNDCLASSW {
        hCursor: cursor,
        hIcon: icon,
        hInstance: hinstance,
        lpszClassName: MAIN_CLASS,
        lpfnWndProc: Some(main_wndproc),
        hbrBackground: GetSysColorBrush(COLOR_WINDOW),
        ..Default::default()
    };

    let overlay_class = WNDCLASSW {
        hCursor: cursor,
        hInstance: hinstance,
        lpszClassName: OVERLAY_CLASS,
        lpfnWndProc: Some(overlay_wndproc),
        hbrBackground: HBRUSH(0),
        ..Default::default()
    };

    RegisterClassW(&main_class);
    RegisterClassW(&overlay_class);
    Ok(())
}

unsafe fn ensure_fonts(dpi: u32) {
    let mut state = app_state().lock().unwrap();
    let dpi = dpi.max(96);
    if state.title_font.0 != 0 && state.font_dpi == dpi {
        return;
    }

    for font in [state.title_font, state.body_font, state.overlay_font] {
        if font.0 != 0 {
            DeleteObject(font);
        }
    }

    let face = to_wide("Microsoft YaHei UI");
    state.title_font = CreateFontW(
        scale_px(26, dpi),
        0,
        0,
        0,
        FW_SEMIBOLD.0 as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET.0 as u32,
        OUT_DEFAULT_PRECIS.0.into(),
        CLIP_DEFAULT_PRECIS.0.into(),
        CLEARTYPE_QUALITY.0.into(),
        (DEFAULT_PITCH.0 as u32) | (FF_DONTCARE.0 as u32),
        PCWSTR(face.as_ptr()),
    );
    state.body_font = CreateFontW(
        scale_px(18, dpi),
        0,
        0,
        0,
        FW_MEDIUM.0 as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET.0 as u32,
        OUT_DEFAULT_PRECIS.0.into(),
        CLIP_DEFAULT_PRECIS.0.into(),
        DEFAULT_QUALITY.0.into(),
        (DEFAULT_PITCH.0 as u32) | (FF_DONTCARE.0 as u32),
        PCWSTR(face.as_ptr()),
    );
    state.overlay_font = CreateFontW(
        scale_px(20, dpi),
        0,
        0,
        0,
        FW_SEMIBOLD.0 as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET.0 as u32,
        OUT_DEFAULT_PRECIS.0.into(),
        CLIP_DEFAULT_PRECIS.0.into(),
        CLEARTYPE_QUALITY.0.into(),
        (DEFAULT_PITCH.0 as u32) | (FF_DONTCARE.0 as u32),
        PCWSTR(face.as_ptr()),
    );
    state.font_dpi = dpi;
}

unsafe fn create_main_window(hinstance: HINSTANCE) -> Result<HWND> {
    let dpi = current_system_dpi();
    let width = scale_px(WINDOW_WIDTH, dpi);
    let height = scale_px(WINDOW_HEIGHT, dpi);
    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        MAIN_CLASS,
        w!("WinMac"),
        WS_OVERLAPPED | WS_SYSMENU | WS_MINIMIZEBOX | WS_VISIBLE,
        centered_x(width),
        centered_y(height),
        width,
        height,
        None,
        None,
        hinstance,
        None,
    );

    if hwnd.0 == 0 {
        Err(windows::core::Error::from_win32())
    } else {
        Ok(hwnd)
    }
}

unsafe fn create_overlay_window(hinstance: HINSTANCE) -> Result<HWND> {
    let hwnd = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
        OVERLAY_CLASS,
        w!("WinMac"),
        WS_POPUP,
        0,
        0,
        OVERLAY_WIDTH,
        OVERLAY_HEIGHT,
        None,
        None,
        hinstance,
        None,
    );

    if hwnd.0 == 0 {
        return Err(windows::core::Error::from_win32());
    }

    Ok(hwnd)
}

unsafe extern "system" fn main_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            create_main_controls(hwnd);
            LRESULT(0)
        }
        WM_PAINT => {
            paint_main_window(hwnd);
            LRESULT(0)
        }
        WM_COMMAND => {
            let command_id = loword(wparam.0 as usize) as isize;
            let notify_code = hiword(wparam.0 as usize);
            if command_id == ID_CHECK_AUTOSTART && notify_code == BN_CLICKED as u16 {
                let checked = SendMessageW(hwnd_checkbox(), BM_GETCHECK, WPARAM(0), LPARAM(0)).0
                    == BST_CHECKED.0 as isize;
                set_autostart(checked);
                sync_autostart_checkbox();
            }
            LRESULT(0)
        }
        WM_HOTKEY => {
            match wparam.0 as i32 {
                HOTKEY_HIDE_ID => hide_active_window(),
                HOTKEY_QUIT_ID => confirm_or_quit_active_window(),
                _ => {}
            }
            LRESULT(0)
        }
        WM_TIMER => {
            handle_main_timer(hwnd, wparam.0);
            LRESULT(0)
        }
        WM_SETTINGCHANGE | WM_THEMECHANGED | WM_DPICHANGED => {
            ensure_fonts(GetDpiForWindow(hwnd));
            apply_control_font();
            refresh_theme();
            InvalidateRect(hwnd, None, true);
            LRESULT(0)
        }
        WM_SIZE => {
            if wparam.0 as u32 == 1 {
                ShowWindow(hwnd, SW_HIDE);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            if should_exit() {
                let _ = DestroyWindow(hwnd);
            } else {
                ShowWindow(hwnd, SW_HIDE);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            remove_tray_icon(hwnd);
            let _ = UnregisterHotKey(hwnd, HOTKEY_HIDE_ID);
            let _ = UnregisterHotKey(hwnd, HOTKEY_QUIT_ID);
            destroy_overlay_window();
            delete_fonts();
            destroy_loaded_icons();
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_TRAYICON => {
            match lparam.0 as u32 {
                WM_CONTEXTMENU | WM_RBUTTONUP => show_tray_menu(hwnd),
                WM_LBUTTONDBLCLK | NIN_SELECT => toggle_main_window_visibility(),
                _ => {}
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn overlay_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            paint_overlay_minimal(hwnd);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_SETTINGCHANGE | WM_THEMECHANGED | WM_DPICHANGED => {
            discard_overlay_renderer();
            refresh_theme();
            InvalidateRect(hwnd, None, true);
            LRESULT(0)
        }
        WM_SIZE => {
            discard_overlay_renderer();
            LRESULT(0)
        }
        WM_TIMER => {
            handle_overlay_timer(hwnd, wparam.0);
            LRESULT(0)
        }
        WM_DESTROY => {
            discard_overlay_renderer();
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn create_main_controls(hwnd: HWND) {
    let dpi = current_dpi_for_window(hwnd);
    let checkbox = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("BUTTON"),
        w!("开机自启动"),
        WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | BS_AUTOCHECKBOX as u32),
        scale_px(24, dpi),
        scale_px(176, dpi),
        scale_px(180, dpi),
        scale_px(28, dpi),
        hwnd,
        HMENU(ID_CHECK_AUTOSTART as _),
        None,
        None,
    );

    {
        let mut state = app_state().lock().unwrap();
        state.checkbox_hwnd = checkbox;
    }

    let font = app_state().lock().unwrap().body_font;
    if checkbox.0 != 0 && font.0 != 0 {
        SendMessageW(checkbox, 0x0030, WPARAM(font.0 as usize), LPARAM(1));
    }
}

unsafe fn apply_control_font() {
    let (checkbox, font) = {
        let state = app_state().lock().unwrap();
        (state.checkbox_hwnd, state.body_font)
    };

    if checkbox.0 != 0 && font.0 != 0 {
        SendMessageW(checkbox, 0x0030, WPARAM(font.0 as usize), LPARAM(1));
    }
}

unsafe fn paint_main_window(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);
    let mut rect = RECT::default();
    let _ = GetClientRect(hwnd, &mut rect);
    FillRect(hdc, &rect, GetSysColorBrush(COLOR_WINDOW));
    SetBkMode(hdc, TRANSPARENT);
    SetTextColor(hdc, colorref_from_rgb(34, 34, 36));

    let (title_font, body_font) = {
        let state = app_state().lock().unwrap();
        (state.title_font, state.body_font)
    };

    let old_title = SelectObject(hdc, HGDIOBJ(title_font.0));
    let mut title = to_text_wide("WinMac");
    let mut title_rect = RECT {
        left: 24,
        top: 18,
        right: rect.right - 24,
        bottom: 60,
    };
    DrawTextW(
        hdc,
        &mut title,
        &mut title_rect,
        DT_LEFT | DT_SINGLELINE | DT_VCENTER,
    );

    SelectObject(hdc, HGDIOBJ(body_font.0));
    let mut line1 = to_text_wide("Alt + W 最小化当前活动窗口");
    let mut line1_rect = RECT {
        left: 24,
        top: 78,
        right: rect.right - 24,
        bottom: 108,
    };
    DrawTextW(
        hdc,
        &mut line1,
        &mut line1_rect,
        DT_LEFT | DT_SINGLELINE | DT_VCENTER,
    );

    let mut line2 = to_text_wide("Alt + Q 连按两次退出当前活动窗口");
    let mut line2_rect = RECT {
        left: 24,
        top: 110,
        right: rect.right - 24,
        bottom: 140,
    };
    DrawTextW(
        hdc,
        &mut line2,
        &mut line2_rect,
        DT_LEFT | DT_SINGLELINE | DT_VCENTER,
    );

    let mut line3 =
        to_text_wide("托盘菜单可恢复隐藏窗口，并切换开机自启动。点击右上角关闭只会隐藏到托盘。");
    let mut line3_rect = RECT {
        left: 24,
        top: 142,
        right: rect.right - 24,
        bottom: 206,
    };
    DrawTextW(hdc, &mut line3, &mut line3_rect, DT_LEFT | DT_WORDBREAK);

    SelectObject(hdc, old_title);
    EndPaint(hwnd, &ps);
}

#[allow(dead_code)]
unsafe fn paint_overlay(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let _ = BeginPaint(hwnd, &mut ps);
    let (theme, overlay_title, dpi) = {
        let state = app_state().lock().unwrap();
        (
            state.theme,
            state.overlay_title.clone(),
            state.overlay_dpi.max(96),
        )
    };
    let visual = OverlayVisual {
        theme: match theme {
            ThemeMode::Light => OverlayTheme::Light,
            ThemeMode::Dark => OverlayTheme::Dark,
        },
        title: if overlay_title.is_empty() {
            "当前窗口".to_string()
        } else {
            overlay_title
        },
        subtitle: "再次按 Alt + Q 关闭当前窗口".to_string(),
        hint: "再次按下以退出".to_string(),
        badge: "WinMac Quick Quit".to_string(),
    };
    let _ = draw_overlay(hwnd, &visual, dpi);
    EndPaint(hwnd, &ps);
}

unsafe fn paint_overlay_minimal(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let _ = BeginPaint(hwnd, &mut ps);
    let (theme, overlay_title, dpi) = {
        let state = app_state().lock().unwrap();
        (
            state.theme,
            state.overlay_title.clone(),
            state.overlay_dpi.max(96),
        )
    };
    let visual = OverlayVisual {
        theme: match theme {
            ThemeMode::Light => OverlayTheme::Light,
            ThemeMode::Dark => OverlayTheme::Dark,
        },
        title: if overlay_title.is_empty() {
            "Current window".to_string()
        } else {
            overlay_title
        },
        subtitle: "Press Alt + Q again to close".to_string(),
        hint: "Press again to quit".to_string(),
        badge: "WinMac".to_string(),
    };
    let _ = draw_overlay(hwnd, &visual, dpi);
    EndPaint(hwnd, &ps);
}

unsafe fn handle_main_timer(hwnd: HWND, timer_id: usize) {
    if timer_id == TIMER_MINIMIZE_ANCHOR && resolve_pending_minimize_anchor() {
        let _ = KillTimer(hwnd, TIMER_MINIMIZE_ANCHOR);
    }
}

unsafe fn handle_overlay_timer(hwnd: HWND, timer_id: usize) {
    match timer_id {
        TIMER_OVERLAY_FADE_IN => {
            let (x, y, target_y, dpi) = {
                let mut state = app_state().lock().unwrap();
                if state.overlay_y > state.overlay_target_y {
                    state.overlay_y = (state.overlay_y - scale_px(2, state.overlay_dpi.max(96)))
                        .max(state.overlay_target_y);
                }
                (
                    state.overlay_x,
                    state.overlay_y,
                    state.overlay_target_y,
                    state.overlay_dpi.max(96),
                )
            };
            let overlay_width = scale_px(OVERLAY_WIDTH, dpi);
            let overlay_height = scale_px(OVERLAY_HEIGHT, dpi);
            let _ = SetWindowPos(
                hwnd,
                HWND(-1),
                x,
                y,
                overlay_width,
                overlay_height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            if y <= target_y {
                let _ = KillTimer(hwnd, TIMER_OVERLAY_FADE_IN);
            }
        }
        TIMER_OVERLAY_DELAY_HIDE => {
            let _ = KillTimer(hwnd, TIMER_OVERLAY_DELAY_HIDE);
            if AnimateWindow(hwnd, 120, AW_BLEND | AW_HIDE).is_err() {
                ShowWindow(hwnd, SW_HIDE);
            }
        }
        _ => {}
    }
}

unsafe fn add_tray_icon(hwnd: HWND) -> Result<()> {
    let tray_icon = {
        let state = app_state().lock().unwrap();
        if state.app_icon.0 != 0 {
            state.app_icon
        } else {
            load_app_icon(HINSTANCE(GetModuleHandleW(None)?.0), 20)
        }
    };
    let mut nid = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_MESSAGE | NIF_TIP | NIF_ICON,
        uCallbackMessage: WM_TRAYICON,
        hIcon: tray_icon,
        ..Default::default()
    };
    copy_wide_text(&mut nid.szTip, APP_NAME);
    Shell_NotifyIconW(NIM_ADD, &nid).ok()?;
    Ok(())
}

unsafe fn remove_tray_icon(hwnd: HWND) {
    let nid = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        ..Default::default()
    };
    let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
}

unsafe fn show_tray_menu(hwnd: HWND) {
    let menu = match CreatePopupMenu() {
        Ok(menu) => menu,
        Err(_) => return,
    };

    prune_hidden_windows();

    let (autostart, has_hidden, visible) = {
        let state = app_state().lock().unwrap();
        (
            state.autostart_enabled,
            !state.hidden_windows.is_empty(),
            IsWindowVisible(state.main_hwnd).as_bool(),
        )
    };

    let toggle_label = if visible {
        "隐藏设置窗口"
    } else {
        "显示设置窗口"
    };
    let restore_flags = if has_hidden {
        MF_STRING
    } else {
        MF_STRING | MF_GRAYED
    };
    let autostart_flags = if autostart {
        MF_STRING | MF_CHECKED
    } else {
        MF_STRING | MF_UNCHECKED
    };
    let label_en = to_wide(if visible {
        "Hide Settings"
    } else {
        "Show Settings"
    });
    let restore_last_en = to_wide("Restore Last Hidden Window");
    let restore_all_en = to_wide("Restore All Hidden Windows");
    let autostart_en = to_wide("Launch At Login");
    let exit_en = to_wide("Quit WinMac");

    let _label = to_wide(toggle_label);
    let _restore_last = to_wide("恢复最近隐藏窗口");
    let _restore_all = to_wide("恢复全部隐藏窗口");
    let _autostart_text = to_wide("开机自启动");
    let _exit_text = to_wide("退出 WinMac");

    let _ = AppendMenuW(
        menu,
        MF_STRING,
        ID_TRAY_TOGGLE_WINDOW,
        PCWSTR(label_en.as_ptr()),
    );
    let _ = AppendMenuW(
        menu,
        restore_flags,
        ID_TRAY_RESTORE_LAST,
        PCWSTR(restore_last_en.as_ptr()),
    );
    let _ = AppendMenuW(
        menu,
        restore_flags,
        ID_TRAY_RESTORE_ALL,
        PCWSTR(restore_all_en.as_ptr()),
    );
    let _ = AppendMenuW(
        menu,
        autostart_flags,
        ID_TRAY_AUTOSTART,
        PCWSTR(autostart_en.as_ptr()),
    );
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
    let _ = AppendMenuW(menu, MF_STRING, ID_TRAY_EXIT, PCWSTR(exit_en.as_ptr()));

    let mut cursor = POINT::default();
    let _ = GetCursorPos(&mut cursor);
    let _ = SetForegroundWindow(hwnd);

    let command = TrackPopupMenu(
        menu,
        TPM_RETURNCMD | TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RIGHTBUTTON,
        cursor.x,
        cursor.y,
        0,
        hwnd,
        None,
    );

    let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
    let _ = DestroyMenu(menu);

    match command.0 as usize {
        ID_TRAY_TOGGLE_WINDOW => toggle_main_window_visibility(),
        ID_TRAY_RESTORE_LAST => restore_last_hidden_window(),
        ID_TRAY_RESTORE_ALL => restore_all_hidden_windows(),
        ID_TRAY_AUTOSTART => {
            let enabled = !app_state().lock().unwrap().autostart_enabled;
            set_autostart(enabled);
            sync_autostart_checkbox();
        }
        ID_TRAY_EXIT => request_exit(),
        _ => {}
    }
}

unsafe fn toggle_main_window_visibility() {
    let hwnd = app_state().lock().unwrap().main_hwnd;
    if hwnd.0 == 0 {
        return;
    }
    if IsWindowVisible(hwnd).as_bool() {
        ShowWindow(hwnd, SW_HIDE);
    } else {
        ShowWindow(hwnd, SW_RESTORE);
        let _ = SetForegroundWindow(hwnd);
    }
}

unsafe fn request_exit() {
    let hwnd = {
        let mut state = app_state().lock().unwrap();
        state.exiting = true;
        state.main_hwnd
    };
    if hwnd.0 != 0 {
        let _ = DestroyWindow(hwnd);
    }
}

fn should_exit() -> bool {
    app_state().lock().unwrap().exiting
}

unsafe fn destroy_overlay_window() {
    let hwnd = app_state().lock().unwrap().overlay_hwnd;
    if hwnd.0 != 0 && IsWindow(hwnd).as_bool() {
        let _ = DestroyWindow(hwnd);
    }
}

unsafe fn delete_fonts() {
    let (title, body, overlay) = {
        let mut state = app_state().lock().unwrap();
        let fonts = (state.title_font, state.body_font, state.overlay_font);
        state.title_font = HFONT(0);
        state.body_font = HFONT(0);
        state.overlay_font = HFONT(0);
        fonts
    };

    for font in [title, body, overlay] {
        if font.0 != 0 {
            DeleteObject(font);
        }
    }
}

unsafe fn destroy_loaded_icons() {
    let app_icon = {
        let mut state = app_state().lock().unwrap();
        let icon = state.app_icon;
        state.app_icon = HICON::default();
        icon
    };

    if app_icon.0 != 0 {
        let _ = DestroyIcon(app_icon);
    }
}

unsafe fn register_hotkeys(hwnd: HWND) {
    let modifiers = HOT_KEY_MODIFIERS(MOD_ALT.0 | MOD_NOREPEAT.0);
    let hide_ok = RegisterHotKey(hwnd, HOTKEY_HIDE_ID, modifiers, 'W' as u32).is_ok();
    let quit_ok = RegisterHotKey(hwnd, HOTKEY_QUIT_ID, modifiers, 'Q' as u32).is_ok();
    if !(hide_ok && quit_ok) {
        let _ = MessageBoxW(
            hwnd,
            w!("Alt + W 或 Alt + Q 无法注册，请确认没有被其它程序占用。"),
            w!("WinMac"),
            MB_OK | MB_ICONWARNING,
        );
    }
}

unsafe fn hide_active_window() {
    if active_foreground_is_fullscreen() {
        return;
    }

    resolve_pending_minimize_anchor();
    if try_restore_last_minimized_window() {
        return;
    }
    clear_last_minimized_window();

    let Some(target) = current_manageable_window() else {
        return;
    };

    ShowWindow(target, SW_MINIMIZE);
    let now = GetTickCount64();
    let main_hwnd = {
        let mut state = app_state().lock().unwrap();
        state.pending_minimized_window = target;
        state.pending_minimize_deadline = now + MINIMIZE_ANCHOR_CAPTURE_MS;
        state.main_hwnd
    };
    if main_hwnd.0 != 0 {
        let _ = SetTimer(main_hwnd, TIMER_MINIMIZE_ANCHOR, 5, None);
    }
    let _ = resolve_pending_minimize_anchor();
}

unsafe fn confirm_or_quit_active_window() {
    if active_foreground_is_fullscreen() {
        return;
    }

    let Some(target) = current_manageable_window() else {
        hide_overlay_now();
        return;
    };

    let now = GetTickCount64();
    let should_quit = {
        let mut state = app_state().lock().unwrap();
        let second_press = state.quit_target == target && now <= state.quit_deadline;
        if second_press {
            state.quit_target = HWND(0);
            state.quit_deadline = 0;
            true
        } else {
            state.quit_target = target;
            state.quit_deadline = now + QUIT_CONFIRM_MS;
            false
        }
    };

    if should_quit {
        hide_overlay_now();
        let _ = PostMessageW(target, WM_CLOSE, WPARAM(0), LPARAM(0));
    } else {
        show_quit_overlay(target);
    }
}

unsafe fn current_manageable_window() -> Option<HWND> {
    let root = current_foreground_root_window();
    if root.0 == 0 || !IsWindow(root).as_bool() {
        return None;
    }

    if !is_manageable_window(root) {
        return None;
    }

    Some(root)
}

unsafe fn current_foreground_root_window() -> HWND {
    let foreground = GetForegroundWindow();
    if foreground.0 == 0 {
        return HWND(0);
    }

    let root = GetAncestor(foreground, GA_ROOT);
    if root.0 == 0 { foreground } else { root }
}

unsafe fn active_foreground_is_fullscreen() -> bool {
    let root = current_foreground_root_window();
    root.0 != 0 && is_fullscreen_window(root)
}

unsafe fn try_restore_last_minimized_window() -> bool {
    resolve_pending_minimize_anchor();
    let (target, anchor) = {
        let state = app_state().lock().unwrap();
        (state.last_minimized_window, state.last_minimize_anchor)
    };

    if target.0 == 0 || !IsWindow(target).as_bool() || !IsIconic(target).as_bool() {
        clear_last_minimized_window();
        return false;
    }

    let current = current_foreground_root_window();
    if !should_restore_last_minimized(current, anchor) {
        return false;
    }

    ShowWindow(target, SW_RESTORE);
    let _ = SetForegroundWindow(target);
    clear_last_minimized_window();
    true
}

unsafe fn resolve_pending_minimize_anchor() -> bool {
    let current = current_foreground_root_window();
    let now = GetTickCount64();
    let mut state = app_state().lock().unwrap();
    let target = state.pending_minimized_window;

    if target.0 == 0 {
        return true;
    }

    if current.0 != 0 && current != target {
        state.last_minimized_window = target;
        state.last_minimize_anchor = current;
        state.pending_minimized_window = HWND(0);
        state.pending_minimize_deadline = 0;
        return true;
    }

    if now >= state.pending_minimize_deadline {
        state.last_minimized_window = target;
        state.last_minimize_anchor = HWND(0);
        state.pending_minimized_window = HWND(0);
        state.pending_minimize_deadline = 0;
        return true;
    }

    false
}

fn should_restore_last_minimized(current: HWND, anchor: HWND) -> bool {
    current.0 != 0 && anchor.0 != 0 && current == anchor
}

fn clear_last_minimized_window() {
    let mut state = app_state().lock().unwrap();
    state.last_minimized_window = HWND(0);
    state.last_minimize_anchor = HWND(0);
    state.pending_minimized_window = HWND(0);
    state.pending_minimize_deadline = 0;
}

unsafe fn is_manageable_window(hwnd: HWND) -> bool {
    if hwnd.0 == 0 {
        return false;
    }

    let class_name = get_window_class_name(hwnd);
    if matches!(class_name.as_str(), "Shell_TrayWnd" | "Progman" | "WorkerW") {
        return false;
    }

    let title_len = GetWindowTextLengthW(hwnd);
    if title_len == 0 && !IsWindowVisible(hwnd).as_bool() {
        return false;
    }

    true
}

unsafe fn is_fullscreen_window(hwnd: HWND) -> bool {
    let mut window_rect = RECT::default();
    if GetWindowRect(hwnd, &mut window_rect).is_err() {
        return false;
    }

    let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
    if monitor.0 == 0 {
        return false;
    }

    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !GetMonitorInfoW(monitor, &mut info as *mut _ as *mut _).as_bool() {
        return false;
    }

    rect_covers_monitor(window_rect, info.rcMonitor, 2)
}

fn rect_covers_monitor(window_rect: RECT, monitor_rect: RECT, tolerance: i32) -> bool {
    (window_rect.left - monitor_rect.left).abs() <= tolerance
        && (window_rect.top - monitor_rect.top).abs() <= tolerance
        && (window_rect.right - monitor_rect.right).abs() <= tolerance
        && (window_rect.bottom - monitor_rect.bottom).abs() <= tolerance
}

unsafe fn restore_last_hidden_window() {
    prune_hidden_windows();
    let target = {
        let mut state = app_state().lock().unwrap();
        state.hidden_windows.pop()
    };
    if let Some(hwnd) = target {
        ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }
}

unsafe fn restore_all_hidden_windows() {
    prune_hidden_windows();
    let windows = {
        let mut state = app_state().lock().unwrap();
        std::mem::take(&mut state.hidden_windows)
    };
    for hwnd in windows {
        if IsWindow(hwnd).as_bool() {
            ShowWindow(hwnd, SW_SHOW);
        }
    }
}

unsafe fn prune_hidden_windows() {
    let mut state = app_state().lock().unwrap();
    state
        .hidden_windows
        .retain(|hwnd| IsWindow(*hwnd).as_bool());
}

unsafe fn show_quit_overlay(target: HWND) {
    let hwnd = app_state().lock().unwrap().overlay_hwnd;
    if hwnd.0 == 0 {
        return;
    }

    let dpi = current_dpi_for_window(target);
    ensure_fonts(dpi);
    apply_control_font();
    refresh_theme();

    let monitor = MonitorFromWindow(target, MONITOR_DEFAULTTONEAREST);
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let _ = GetMonitorInfoW(monitor, &mut info as *mut _ as *mut _);

    let work = info.rcWork;
    let overlay_width = scale_px(OVERLAY_WIDTH, dpi);
    let overlay_height = scale_px(OVERLAY_HEIGHT, dpi);
    let x = compute_overlay_x(work, overlay_width);
    let y = compute_overlay_target_y(work, overlay_height, dpi);
    let title = get_window_title(target);

    {
        let mut state = app_state().lock().unwrap();
        state.overlay_x = x;
        state.overlay_target_y = y;
        state.overlay_y = y + scale_px(OVERLAY_LIFT, dpi);
        state.overlay_title = title;
        state.overlay_dpi = dpi;
    }

    let _ = KillTimer(hwnd, TIMER_OVERLAY_FADE_IN);
    let _ = KillTimer(hwnd, TIMER_OVERLAY_DELAY_HIDE);
    discard_overlay_renderer();
    let _ = SetWindowPos(
        hwnd,
        HWND(-1),
        x,
        y + scale_px(OVERLAY_LIFT, dpi),
        overlay_width,
        overlay_height,
        SWP_NOACTIVATE,
    );
    if AnimateWindow(hwnd, 150, AW_BLEND).is_err() {
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
    InvalidateRect(hwnd, None, true);
    SetTimer(hwnd, TIMER_OVERLAY_FADE_IN, 16, None);
    SetTimer(hwnd, TIMER_OVERLAY_DELAY_HIDE, QUIT_CONFIRM_MS as u32, None);
}

unsafe fn hide_overlay_now() {
    let hwnd = app_state().lock().unwrap().overlay_hwnd;
    if hwnd.0 == 0 {
        return;
    }
    let _ = KillTimer(hwnd, TIMER_OVERLAY_FADE_IN);
    let _ = KillTimer(hwnd, TIMER_OVERLAY_DELAY_HIDE);
    ShowWindow(hwnd, SW_HIDE);
}

unsafe fn refresh_theme() {
    let theme = detect_theme();
    let (main_hwnd, overlay_hwnd) = {
        let mut state = app_state().lock().unwrap();
        state.theme = theme;
        (state.main_hwnd, state.overlay_hwnd)
    };
    if main_hwnd.0 != 0 {
        apply_main_window_theme(main_hwnd, theme);
    }
    if overlay_hwnd.0 != 0 {
        apply_overlay_theme(overlay_hwnd, theme);
    }
}

unsafe fn apply_main_window_theme(hwnd: HWND, theme: ThemeMode) {
    let dark_flag: i32 = if theme == ThemeMode::Dark { 1 } else { 0 };
    let round = DWM_WINDOW_CORNER_PREFERENCE(DWMWCP_ROUND.0);
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_USE_IMMERSIVE_DARK_MODE,
        &dark_flag as *const _ as *const c_void,
        size_of::<i32>() as u32,
    );
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_WINDOW_CORNER_PREFERENCE,
        &round as *const _ as *const c_void,
        size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
    );
}

unsafe fn apply_overlay_theme(hwnd: HWND, theme: ThemeMode) {
    let dark_flag: i32 = if theme == ThemeMode::Dark { 1 } else { 0 };
    let round = DWM_WINDOW_CORNER_PREFERENCE(DWMWCP_ROUND.0);
    let backdrop = DWM_SYSTEMBACKDROP_TYPE(DWMSBT_NONE.0);
    let host_backdrop = BOOL(0);
    let no_border = DWMWA_COLOR_NONE;
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_USE_IMMERSIVE_DARK_MODE,
        &dark_flag as *const _ as *const c_void,
        size_of::<i32>() as u32,
    );
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_WINDOW_CORNER_PREFERENCE,
        &round as *const _ as *const c_void,
        size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
    );
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_BORDER_COLOR,
        &no_border as *const _ as *const c_void,
        size_of::<u32>() as u32,
    );
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_USE_HOSTBACKDROPBRUSH,
        &host_backdrop as *const _ as *const c_void,
        size_of::<BOOL>() as u32,
    );
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_SYSTEMBACKDROP_TYPE,
        &backdrop as *const _ as *const c_void,
        size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
    );
}

unsafe fn sync_autostart_checkbox() {
    let (checkbox, checked) = {
        let state = app_state().lock().unwrap();
        (state.checkbox_hwnd, state.autostart_enabled)
    };

    if checkbox.0 != 0 {
        let mark = if checked { BST_CHECKED } else { BST_UNCHECKED };
        SendMessageW(checkbox, BM_SETCHECK, WPARAM(mark.0 as usize), LPARAM(0));
        let _ = DrawMenuBar(app_state().lock().unwrap().main_hwnd);
    }
}

unsafe fn set_autostart(enabled: bool) {
    let result = if enabled {
        write_run_registry_value()
    } else {
        delete_run_registry_value()
    };
    if result {
        app_state().lock().unwrap().autostart_enabled = enabled;
    }
}

unsafe fn is_autostart_enabled() -> bool {
    let mut key = HKEY::default();
    if RegOpenKeyExW(
        HKEY_CURRENT_USER,
        RUN_KEY_PATH,
        0,
        KEY_READ | KEY_WOW64_64KEY,
        &mut key,
    )
    .is_err()
    {
        return false;
    }

    let mut kind = REG_VALUE_TYPE(0);
    let mut buffer = [0u16; 1024];
    let mut byte_len = (buffer.len() * 2) as u32;
    let ok = RegGetValueW(
        key,
        PCWSTR::null(),
        RUN_VALUE_NAME,
        RRF_RT_REG_SZ,
        Some(&mut kind),
        Some(buffer.as_mut_ptr() as *mut c_void),
        Some(&mut byte_len),
    )
    .is_ok();
    let _ = RegCloseKey(key);
    ok
}

unsafe fn write_run_registry_value() -> bool {
    let mut key = HKEY::default();
    if RegCreateKeyExW(
        HKEY_CURRENT_USER,
        RUN_KEY_PATH,
        0,
        None,
        REG_OPTION_NON_VOLATILE,
        KEY_SET_VALUE | KEY_WOW64_64KEY,
        None,
        &mut key,
        None,
    )
    .is_err()
    {
        return false;
    }

    let exe = match std::env::current_exe() {
        Ok(path) => quote_path(&path),
        Err(_) => return false,
    };
    let data = exe
        .as_os_str()
        .encode_wide()
        .chain(once(0))
        .collect::<Vec<_>>();
    let result = RegSetValueExW(
        key,
        RUN_VALUE_NAME,
        0,
        REG_SZ,
        Some(std::slice::from_raw_parts(
            data.as_ptr() as *const u8,
            data.len() * 2,
        )),
    )
    .is_ok();
    let _ = RegCloseKey(key);
    result
}

unsafe fn delete_run_registry_value() -> bool {
    let mut key = HKEY::default();
    if RegOpenKeyExW(
        HKEY_CURRENT_USER,
        RUN_KEY_PATH,
        0,
        KEY_SET_VALUE | KEY_WOW64_64KEY,
        &mut key,
    )
    .is_err()
    {
        return false;
    }

    let result = RegDeleteValueW(key, RUN_VALUE_NAME).is_ok();
    let _ = RegCloseKey(key);
    result
}

unsafe fn detect_theme() -> ThemeMode {
    let mut value: u32 = 1;
    let mut size = size_of::<u32>() as u32;
    let status = RegGetValueW(
        HKEY_CURRENT_USER,
        PERSONALIZE_KEY_PATH,
        LIGHT_THEME_VALUE_NAME,
        RRF_RT_REG_DWORD,
        None,
        Some(&mut value as *mut _ as *mut c_void),
        Some(&mut size),
    );
    if status.is_ok() && value == 0 {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    }
}

unsafe fn get_window_class_name(hwnd: HWND) -> String {
    let mut buffer = [0u16; 256];
    let len = GetClassNameW(hwnd, &mut buffer);
    String::from_utf16_lossy(&buffer[..len as usize])
}

unsafe fn load_app_icon(hinstance: HINSTANCE, size: i32) -> HICON {
    let resource_id = PCWSTR(1 as *const u16);
    if let Ok(handle) = LoadImageW(
        hinstance,
        resource_id,
        IMAGE_ICON,
        size,
        size,
        LR_DEFAULTSIZE,
    ) {
        HICON(handle.0)
    } else {
        LoadIconW(None, IDI_APPLICATION).unwrap_or_default()
    }
}

unsafe fn get_window_title(hwnd: HWND) -> String {
    let length = GetWindowTextLengthW(hwnd).max(0) as usize;
    if length == 0 {
        return String::from("Current window");
    }

    let mut buffer = vec![0u16; length + 1];
    let written = GetWindowTextW(hwnd, &mut buffer).max(0) as usize;
    let title = String::from_utf16_lossy(&buffer[..written])
        .trim()
        .to_string();
    truncate_label(&title, 32)
}

fn truncate_label(value: &str, max_chars: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_string();
    }
    if max_chars <= 3 {
        return ".".repeat(max_chars);
    }

    let mut truncated = value
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    truncated.push_str("...");
    truncated
}

fn hwnd_checkbox() -> HWND {
    app_state().lock().unwrap().checkbox_hwnd
}

fn loword(value: usize) -> u16 {
    (value & 0xffff) as u16
}

fn hiword(value: usize) -> u16 {
    ((value >> 16) & 0xffff) as u16
}

fn centered_x(width: i32) -> i32 {
    (screen_width() - width) / 2
}

fn centered_y(height: i32) -> i32 {
    (screen_height() - height) / 2
}

fn compute_overlay_x(work: RECT, overlay_width: i32) -> i32 {
    work.left + ((work.right - work.left - overlay_width) / 2)
}

fn compute_overlay_target_y(work: RECT, overlay_height: i32, dpi: u32) -> i32 {
    let work_height = (work.bottom - work.top).max(0);
    let preferred_center = work.top + ((work_height * OVERLAY_VERTICAL_ANCHOR_PERCENT) / 100);
    let preferred_y = preferred_center - (overlay_height / 2);
    let min_y = work.top + scale_px(OVERLAY_TOP_MARGIN, dpi);
    let max_y = work.bottom - overlay_height - scale_px(OVERLAY_BOTTOM_MARGIN, dpi);
    preferred_y.clamp(min_y, max_y.max(min_y))
}

fn current_system_dpi() -> u32 {
    unsafe { GetDpiForSystem().max(96) }
}

fn current_dpi_for_window(hwnd: HWND) -> u32 {
    unsafe {
        let dpi = GetDpiForWindow(hwnd);
        if dpi == 0 {
            current_system_dpi()
        } else {
            dpi.max(96)
        }
    }
}

fn scale_px(value: i32, dpi: u32) -> i32 {
    ((value as i64 * dpi as i64 + 48) / 96) as i32
}

fn screen_width() -> i32 {
    unsafe { GetSystemMetrics(SM_CXSCREEN) }
}

fn screen_height() -> i32 {
    unsafe { GetSystemMetrics(SM_CYSCREEN) }
}

fn quote_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("\"{}\"", path.as_os_str().to_string_lossy()))
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(once(0)).collect()
}

fn to_text_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().collect()
}

fn copy_wide_text<const N: usize>(dst: &mut [u16; N], src: &str) {
    let wide = to_wide(src);
    let len = wide.len().min(N);
    dst[..len].copy_from_slice(&wide[..len]);
}

fn colorref_from_rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> RECT {
        RECT {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn truncate_label_keeps_short_text() {
        assert_eq!(truncate_label("Explorer", 32), "Explorer");
    }

    #[test]
    fn truncate_label_uses_ascii_ellipsis() {
        assert_eq!(truncate_label("1234567890", 6), "123...");
        assert_eq!(truncate_label("abcd", 3), "...");
    }

    #[test]
    fn scale_px_rounds_for_high_dpi() {
        assert_eq!(scale_px(100, 96), 100);
        assert_eq!(scale_px(100, 144), 150);
    }

    #[test]
    fn compute_overlay_x_centers_in_work_area() {
        assert_eq!(compute_overlay_x(rect(100, 0, 1500, 900), 336), 632);
    }

    #[test]
    fn compute_overlay_target_y_moves_card_lower_than_center() {
        let work = rect(0, 0, 2560, 1400);
        let overlay_height = 118;
        let y = compute_overlay_target_y(work, overlay_height, 96);

        assert_eq!(y, 977);
        assert!(y > (work.bottom - work.top - overlay_height) / 2);
    }

    #[test]
    fn compute_overlay_target_y_clamps_to_bottom_margin() {
        let work = rect(0, 0, 800, 180);
        let overlay_height = 118;

        assert_eq!(compute_overlay_target_y(work, overlay_height, 96), 16);
    }

    #[test]
    fn restore_last_minimized_when_still_on_anchor_window() {
        assert!(should_restore_last_minimized(HWND(22), HWND(22)));
    }

    #[test]
    fn restore_last_minimized_when_desktop_anchor_matches_current() {
        assert!(should_restore_last_minimized(HWND(33), HWND(33)));
    }

    #[test]
    fn do_not_restore_after_switching_to_new_manageable_window() {
        assert!(!should_restore_last_minimized(HWND(44), HWND(22)));
    }

    #[test]
    fn do_not_restore_when_anchor_was_not_captured() {
        assert!(!should_restore_last_minimized(HWND(22), HWND(0)));
    }

    #[test]
    fn rect_covers_monitor_accepts_fullscreen_with_small_tolerance() {
        assert!(rect_covers_monitor(
            rect(-1, 0, 2560, 1441),
            rect(0, 0, 2560, 1440),
            2,
        ));
    }

    #[test]
    fn rect_covers_monitor_rejects_window_that_only_fills_work_area() {
        assert!(!rect_covers_monitor(
            rect(0, 0, 2560, 1400),
            rect(0, 0, 2560, 1440),
            2,
        ));
    }
}
