//! opti-vicity launcher: one small window to pick the game folder (drag and
//! drop, or the folder button), choose a few options, prepare the shaders and
//! pipelines with a progress bar, and start the game.
//!
//! `opti-vicity.exe [--game <folder>] [--play]`
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    eprintln!("opti-vicity runs on Windows only");
}

#[cfg(windows)]
mod diag;
#[cfg(windows)]
mod settings;
#[cfg(windows)]
mod work;

#[cfg(windows)]
fn main() {
    ui::main();
}

#[cfg(windows)]
mod ui {
    use std::ffi::c_void;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Mutex, OnceLock};

    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        BeginPaint, CreateFontIndirectW, CreateSolidBrush, DrawTextW, EndPaint, FillRect, LOGFONTW,
        PAINTSTRUCT, SelectObject, SetBkColor, SetBkMode, SetTextColor,
    };
    use windows_sys::Win32::System::Com::{
        COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Controls::{
        ICC_PROGRESS_CLASS, ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx,
        TTTOOLINFOW,
    };
    use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
    use windows_sys::Win32::UI::Shell::{
        BROWSEINFOW, DragAcceptFiles, DragFinish, DragQueryFileW, SHBrowseForFolderW,
        SHGetPathFromIDListW, ShellExecuteW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AdjustWindowRectEx, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DispatchMessageW,
        GetMessageW, GetSystemMetrics, IDC_ARROW, IsDialogMessageW, LoadCursorW, LoadIconW, MSG,
        PostMessageW, PostQuitMessage, RegisterClassExW, SM_CXSCREEN, SM_CYSCREEN, SW_MINIMIZE,
        SW_RESTORE, SW_SHOW, SW_SHOWNORMAL, SendMessageW, SetForegroundWindow, SetWindowTextW,
        ShowWindow, TranslateMessage, WM_APP, WM_CLOSE, WM_COMMAND, WM_CTLCOLORBTN,
        WM_CTLCOLORSTATIC, WM_DESTROY, WM_PAINT, WM_SETFONT, WNDCLASSEXW, WS_CAPTION, WS_CHILD,
        WS_EX_ACCEPTFILES, WS_EX_CLIENTEDGE, WS_EX_TOPMOST, WS_MINIMIZEBOX, WS_OVERLAPPED,
        WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
    };

    use crate::settings::{self, Config, t};
    use crate::work::{self, Event, Plan};

    // Control styles and messages (as numbers: the same in every SDK).
    const BS_PUSHBUTTON: u32 = 0x0;
    const BS_DEFPUSHBUTTON: u32 = 0x1;
    const BS_AUTOCHECKBOX: u32 = 0x3;
    const CBS_DROPDOWNLIST: u32 = 0x3;
    const ES_AUTOHSCROLL: u32 = 0x80;
    const ES_READONLY: u32 = 0x800;
    const SS_NOPREFIX: u32 = 0x80;
    const SS_ENDELLIPSIS: u32 = 0x4000;
    const WS_VSCROLL: u32 = 0x0020_0000;
    const BM_GETCHECK: u32 = 0xF0;
    const BM_SETCHECK: u32 = 0xF1;
    const CB_ADDSTRING: u32 = 0x143;
    const CB_GETCURSEL: u32 = 0x147;
    const CB_SETCURSEL: u32 = 0x14E;
    const CBN_SELCHANGE: u32 = 1;
    const BN_CLICKED: u32 = 0;
    const PBM_SETPOS: u32 = 0x402;
    const PBM_SETRANGE32: u32 = 0x406;
    const PBM_SETMARQUEE: u32 = 0x40A;
    const PBS_MARQUEE: u32 = 0x08;
    const TTM_ADDTOOLW: u32 = 0x432;
    const TTF_IDISHWND: u32 = 0x1;
    const TTF_SUBCLASS: u32 = 0x10;
    const TTS_ALWAYSTIP: u32 = 0x1;
    const BIF_RETURNONLYFSDIRS: u32 = 0x1;
    const BIF_NEWDIALOGSTYLE: u32 = 0x40;
    const BIF_NONEWFOLDERBUTTON: u32 = 0x200;
    const BFFM_INITIALIZED: u32 = 1;
    const BFFM_SETSELECTIONW: u32 = 0x467;
    const WM_DROPFILES: u32 = 0x233;
    const IDOK: usize = 1;

    const ID_BROWSE: usize = 101;
    const ID_SHOW: usize = 102;
    const ID_SERIES_X: usize = 103;
    const ID_OVERLAY: usize = 104;
    const ID_CARD: usize = 105;
    const ID_MOUSE: usize = 106;
    const ID_DIAG: usize = 107;
    const ID_PLAY: usize = 109;
    const ID_PREPARE: usize = 110;
    const ID_LOGS: usize = 111;

    /// Mouse speeds in the list (0 = off).
    const MOUSE_SPEEDS: [f32; 7] = [0.0, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0];

    /// The window's controls (handles as numbers, so they can live in a static).
    #[derive(Clone, Copy, Default)]
    struct Controls {
        window: isize,
        path: isize,
        hint: isize,
        series_x: isize,
        overlay: isize,
        card: isize,
        mouse: isize,
        diag: isize,
        progress: isize,
        status: isize,
        play: isize,
        prepare: isize,
        browse: isize,
        show: isize,
    }

    #[derive(Clone, Copy, PartialEq)]
    enum Busy {
        Idle,
        Preparing,
        Running,
    }

    struct State {
        cfg: Config,
        engine: Option<PathBuf>,
        busy: Busy,
        /// The current step's name, for the progress line.
        phase: String,
        /// The current work starts the game after preparing (Play, not Prepare).
        run_after: bool,
        /// No pipelines were written down yet: this is a first session.
        first_session: bool,
    }

    static CONTROLS: OnceLock<Controls> = OnceLock::new();
    static STATE: Mutex<Option<State>> = Mutex::new(None);
    /// Brushes and fonts: (white, header, ui font, title font, small font, icon font).
    static GDI: OnceLock<[isize; 6]> = OnceLock::new();
    /// Text colours of the hint and status lines.
    static HINT_COLOR: AtomicU32 = AtomicU32::new(GRAY);
    static STATUS_COLOR: AtomicU32 = AtomicU32::new(GRAY);
    static DPI: AtomicU32 = AtomicU32::new(96);

    const fn rgb(r: u32, g: u32, b: u32) -> u32 {
        r | (g << 8) | (b << 16)
    }
    const GRAY: u32 = rgb(0x5f, 0x63, 0x6b);
    const GREEN: u32 = rgb(0x1a, 0x7f, 0x37);
    const RED: u32 = rgb(0xc4, 0x2b, 0x1c);
    const HEADER: u32 = rgb(0x16, 0x1a, 0x24);

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn h(v: isize) -> HWND {
        v as HWND
    }

    /// Base layout units (96 DPI) to pixels.
    fn s(v: i32) -> i32 {
        v * DPI.load(Ordering::Relaxed) as i32 / 96
    }

    fn state<R>(f: impl FnOnce(&mut State) -> R) -> R {
        let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        f(guard.as_mut().expect("state"))
    }

    fn ctl() -> Controls {
        CONTROLS.get().copied().unwrap_or_default()
    }

    fn set_text(hwnd: isize, text: &str) {
        let text = wide(text);
        // SAFETY: a live control and a terminated string.
        unsafe { SetWindowTextW(h(hwnd), text.as_ptr()) };
    }

    fn enable(hwnd: isize, on: bool) {
        // SAFETY: a live control.
        unsafe { EnableWindow(h(hwnd), i32::from(on)) };
    }

    fn checked(hwnd: isize) -> bool {
        // SAFETY: a live checkbox.
        unsafe { SendMessageW(h(hwnd), BM_GETCHECK, 0, 0) == 1 }
    }

    fn font(height_pt: i32, weight: i32, face: &str) -> isize {
        // SAFETY: a filled-in description.
        unsafe {
            let mut lf: LOGFONTW = std::mem::zeroed();
            lf.lfHeight = -(height_pt * DPI.load(Ordering::Relaxed) as i32 / 72);
            lf.lfWeight = weight;
            lf.lfQuality = 5; // ClearType
            for (dst, src) in lf.lfFaceName.iter_mut().zip(face.encode_utf16().take(31)) {
                *dst = src;
            }
            CreateFontIndirectW(&lf) as isize
        }
    }

    pub fn main() {
        // SAFETY: plain Win32 set-up, the window procedure below, and a message loop.
        unsafe {
            CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as _);
            let icc = INITCOMMONCONTROLSEX {
                dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
                dwICC: ICC_STANDARD_CLASSES | ICC_PROGRESS_CLASS,
            };
            InitCommonControlsEx(&icc);
            DPI.store(GetDpiForSystem().max(96), Ordering::Relaxed);

            let mut cfg = Config::load();
            let mut autoplay = false;
            let mut args = std::env::args().skip(1);
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--play" => autoplay = true,
                    "--game" => {
                        if let Some(folder) = args
                            .next()
                            .and_then(|g| settings::game_folder(Path::new(&g)))
                        {
                            cfg.game = Some(folder);
                        }
                    }
                    _ => {}
                }
            }
            *STATE.lock().unwrap_or_else(|e| e.into_inner()) = Some(State {
                cfg,
                engine: settings::engine(),
                busy: Busy::Idle,
                phase: String::new(),
                run_after: false,
                first_session: false,
            });

            let white = CreateSolidBrush(rgb(255, 255, 255)) as isize;
            let header = CreateSolidBrush(HEADER) as isize;
            let _ = GDI.set([
                white,
                header,
                font(9, 400, "Segoe UI"),
                font(20, 600, "Segoe UI Semibold"),
                font(8, 400, "Segoe UI"),
                font(11, 400, "Segoe MDL2 Assets"),
            ]);

            let instance = GetModuleHandleW(std::ptr::null());
            let class = wide("OptiVicityLauncher");
            // MAKEINTRESOURCE(1): the icon build.rs puts in.
            let icon = LoadIconW(instance, std::ptr::without_provenance(1));
            let wc = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                style: 0,
                lpfnWndProc: Some(window_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance,
                hIcon: icon,
                hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
                hbrBackground: white as _,
                lpszMenuName: std::ptr::null(),
                lpszClassName: class.as_ptr(),
                hIconSm: icon,
            };
            RegisterClassExW(&wc);

            let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: s(WIDTH),
                bottom: s(HEIGHT),
            };
            AdjustWindowRectEx(&mut rect, style, 0, WS_EX_ACCEPTFILES);
            let (w, hgt) = (rect.right - rect.left, rect.bottom - rect.top);
            let (x, y) = (
                (GetSystemMetrics(SM_CXSCREEN) - w) / 2,
                (GetSystemMetrics(SM_CYSCREEN) - hgt) / 2,
            );
            let title = wide("opti-vicity");
            let window = CreateWindowExW(
                WS_EX_ACCEPTFILES,
                class.as_ptr(),
                title.as_ptr(),
                style,
                x.max(0),
                y.max(0),
                w,
                hgt,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            if window.is_null() {
                return;
            }
            build(window as isize, instance);
            DragAcceptFiles(window, 1);
            ShowWindow(window, SW_SHOW);
            refresh();
            status(t("Pronto", "Ready"), GRAY);
            if autoplay {
                PostMessageW(window, WM_COMMAND, ID_PLAY, 0);
            }

            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                if IsDialogMessageW(window, &msg) != 0 {
                    continue;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    const WIDTH: i32 = 600;
    const HEIGHT: i32 = 404;
    const HEADER_H: i32 = 76;
    const PAD: i32 = 24;

    /// Creates the controls.
    unsafe fn build(window: isize, instance: *mut c_void) {
        let gdi = GDI.get().copied().unwrap_or_default();
        let make = |ex: u32,
                    class: &str,
                    text: &str,
                    style: u32,
                    x: i32,
                    y: i32,
                    w: i32,
                    hh: i32,
                    id: usize,
                    font: isize|
         -> isize {
            let (class, text) = (wide(class), wide(text));
            // SAFETY: a child of our window; the strings outlive the call.
            unsafe {
                let child = CreateWindowExW(
                    ex,
                    class.as_ptr(),
                    text.as_ptr(),
                    WS_CHILD | WS_VISIBLE | style,
                    s(x),
                    s(y),
                    s(w),
                    s(hh),
                    h(window),
                    id as *mut c_void,
                    instance,
                    std::ptr::null(),
                );
                SendMessageW(child, WM_SETFONT, font as usize, 1);
                child as isize
            }
        };
        let (ui, small, icons) = (gdi[2], gdi[4], gdi[5]);
        let inner = WIDTH - 2 * PAD;
        let mut y = HEADER_H + 18;
        make(
            0,
            "STATIC",
            t("Pasta do jogo", "Game folder"),
            SS_NOPREFIX,
            PAD,
            y,
            300,
            20,
            0,
            ui,
        );
        y += 22;
        let button_w = 36;
        let path_w = inner - 2 * button_w - 12;
        let path = make(
            WS_EX_CLIENTEDGE,
            "EDIT",
            "",
            ES_READONLY | ES_AUTOHSCROLL | WS_TABSTOP,
            PAD,
            y,
            path_w,
            28,
            0,
            ui,
        );
        let browse = make(
            0,
            "BUTTON",
            "\u{ED25}",
            BS_PUSHBUTTON | WS_TABSTOP,
            PAD + path_w + 6,
            y,
            button_w,
            28,
            ID_BROWSE,
            icons,
        );
        let show = make(
            0,
            "BUTTON",
            "\u{E8A7}",
            BS_PUSHBUTTON | WS_TABSTOP,
            PAD + path_w + 12 + button_w,
            y,
            button_w,
            28,
            ID_SHOW,
            icons,
        );
        y += 32;
        let hint = make(
            0,
            "STATIC",
            "",
            SS_NOPREFIX | SS_ENDELLIPSIS,
            PAD,
            y,
            inner,
            20,
            0,
            small,
        );
        y += 30;
        let series_x = make(
            0,
            "BUTTON",
            t(
                "Series X (mais nítido, mais lento)",
                "Series X (sharper, slower)",
            ),
            BS_AUTOCHECKBOX | WS_TABSTOP,
            PAD,
            y,
            270,
            22,
            ID_SERIES_X,
            ui,
        );
        let overlay = make(
            0,
            "BUTTON",
            t("Overlay de desempenho (F10)", "Performance overlay (F10)"),
            BS_AUTOCHECKBOX | WS_TABSTOP,
            PAD + 290,
            y,
            260,
            22,
            ID_OVERLAY,
            ui,
        );
        y += 34;
        make(
            0,
            "STATIC",
            t("Placa de vídeo", "Graphics card"),
            SS_NOPREFIX,
            PAD,
            y + 4,
            100,
            20,
            0,
            ui,
        );
        let card = make(
            0,
            "COMBOBOX",
            "",
            CBS_DROPDOWNLIST | WS_TABSTOP | WS_VSCROLL,
            PAD + 100,
            y,
            180,
            200,
            ID_CARD,
            ui,
        );
        make(
            0,
            "STATIC",
            t("Mouse", "Mouse"),
            SS_NOPREFIX,
            PAD + 300,
            y + 4,
            60,
            20,
            0,
            ui,
        );
        let mouse = make(
            0,
            "COMBOBOX",
            "",
            CBS_DROPDOWNLIST | WS_TABSTOP | WS_VSCROLL,
            PAD + 360,
            y,
            130,
            240,
            ID_MOUSE,
            ui,
        );
        y += 38;
        let diag = make(
            0,
            "BUTTON",
            t(
                "Modo diagnóstico (aperte F8 a cada travada; salva tudo para análise)",
                "Diagnostic mode (press F8 at each hitch; keeps everything for analysis)",
            ),
            BS_AUTOCHECKBOX | WS_TABSTOP,
            PAD,
            y,
            inner,
            22,
            ID_DIAG,
            ui,
        );
        y += 42;
        let progress = make(0, "msctls_progress32", "", 0, PAD, y, inner, 12, 0, ui);
        y += 18;
        let status = make(
            0,
            "STATIC",
            "",
            SS_NOPREFIX | SS_ENDELLIPSIS,
            PAD,
            y,
            inner,
            20,
            0,
            ui,
        );
        let by = HEIGHT - 24 - 36;
        make(
            0,
            "BUTTON",
            t("Logs", "Logs"),
            BS_PUSHBUTTON | WS_TABSTOP,
            PAD,
            by,
            90,
            36,
            ID_LOGS,
            ui,
        );
        let prepare = make(
            0,
            "BUTTON",
            t("Preparar shaders", "Prepare shaders"),
            BS_PUSHBUTTON | WS_TABSTOP,
            PAD + 100,
            by,
            150,
            36,
            ID_PREPARE,
            ui,
        );
        // Play in a bigger, bold face.
        let bold = font(11, 600, "Segoe UI");
        let play = make(
            0,
            "BUTTON",
            t("Jogar", "Play"),
            BS_DEFPUSHBUTTON | WS_TABSTOP,
            WIDTH - PAD - 170,
            by,
            170,
            36,
            ID_PLAY,
            bold,
        );
        // SAFETY: live controls.
        unsafe {
            for (i, label) in [
                t("12 GB ou mais", "12 GB or more"),
                t("8 GB (texturas menores)", "8 GB (smaller textures)"),
            ]
            .iter()
            .enumerate()
            {
                let label = wide(label);
                SendMessageW(h(card), CB_ADDSTRING, i, label.as_ptr() as isize);
            }
            for speed in MOUSE_SPEEDS {
                let label = if speed == 0.0 {
                    t("Desligado", "Off").to_owned()
                } else if speed == 1.0 {
                    t("1 (padrão)", "1 (default)").to_owned()
                } else {
                    let text = format!("{speed}");
                    if settings::portuguese() {
                        text.replace('.', ",")
                    } else {
                        text
                    }
                };
                let label = wide(&label);
                SendMessageW(h(mouse), CB_ADDSTRING, 0, label.as_ptr() as isize);
            }
        }
        let _ = CONTROLS.set(Controls {
            window,
            path,
            hint,
            series_x,
            overlay,
            card,
            mouse,
            diag,
            progress,
            status,
            play,
            prepare,
            browse,
            show,
        });
        tooltip(
            window,
            instance,
            browse,
            t("Escolher a pasta do jogo", "Choose the game folder"),
        );
        tooltip(
            window,
            instance,
            show,
            t("Mostrar o jogo no Explorer", "Show the game in Explorer"),
        );
        tooltip(
            window,
            instance,
            diag,
            t(
                "Log detalhado, cada frame longo, e memória/disco a cada segundo, em Logs\\diag. Um bipe confirma cada F8.",
                "Detailed log, every long frame, and memory/disk each second, in Logs\\diag. A beep confirms each F8.",
            ),
        );
        // The saved choices.
        let cfg = state(|st| st.cfg.clone());
        // SAFETY: live controls.
        unsafe {
            SendMessageW(h(series_x), BM_SETCHECK, usize::from(cfg.series_x), 0);
            SendMessageW(h(overlay), BM_SETCHECK, usize::from(cfg.overlay), 0);
            SendMessageW(h(diag), BM_SETCHECK, usize::from(cfg.diagnostic), 0);
            SendMessageW(h(card), CB_SETCURSEL, usize::from(cfg.small_card), 0);
            let pick = MOUSE_SPEEDS
                .iter()
                .position(|&m| (m - cfg.mouse).abs() < 0.01)
                .unwrap_or(3);
            SendMessageW(h(mouse), CB_SETCURSEL, pick, 0);
        }
    }

    fn tooltip(window: isize, instance: *mut c_void, tool: isize, text: &str) {
        let class = wide("tooltips_class32");
        let mut text = wide(text);
        // SAFETY: a tooltip window owned by ours; the text is copied by the control.
        unsafe {
            let tip = CreateWindowExW(
                WS_EX_TOPMOST,
                class.as_ptr(),
                std::ptr::null(),
                WS_POPUP | TTS_ALWAYSTIP,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                h(window),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            let mut info: TTTOOLINFOW = std::mem::zeroed();
            info.cbSize = size_of::<TTTOOLINFOW>() as u32;
            info.uFlags = TTF_IDISHWND | TTF_SUBCLASS;
            info.hwnd = h(window);
            info.uId = tool as usize;
            info.lpszText = text.as_mut_ptr();
            SendMessageW(tip, TTM_ADDTOOLW, 0, &info as *const TTTOOLINFOW as isize);
        }
    }

    /// Puts the state on screen: the game line, the buttons.
    fn refresh() {
        let c = ctl();
        let (game, engine, busy) = state(|st| (st.cfg.game.clone(), st.engine.clone(), st.busy));
        let game_ok = game.as_deref().and_then(settings::game_folder).is_some();
        set_text(
            c.path,
            &game
                .as_ref()
                .map(|g| g.display().to_string())
                .unwrap_or_default(),
        );
        let (hint, color) = if engine.is_none() {
            (
                t(
                    "victory.exe não encontrado: ele deve estar em bin\\ ao lado deste programa",
                    "victory.exe not found: it belongs in bin\\ beside this program",
                )
                .to_owned(),
                RED,
            )
        } else if game.is_none() {
            (
                t(
                    "Arraste a pasta do jogo para esta janela, ou clique na pasta à direita",
                    "Drag your game folder onto this window, or click the folder on the right",
                )
                .to_owned(),
                GRAY,
            )
        } else if game_ok {
            (
                format!("✓ {} {}", settings::TITLE_EXE, t("encontrado", "found")),
                GREEN,
            )
        } else {
            (
                format!(
                    "{} {}",
                    t("Esta pasta não tem", "This folder has no"),
                    settings::TITLE_EXE
                ),
                RED,
            )
        };
        HINT_COLOR.store(color, Ordering::Relaxed);
        set_text(c.hint, &hint);
        let ready = engine.is_some() && game_ok && busy == Busy::Idle;
        enable(c.play, ready);
        enable(c.prepare, ready);
        enable(c.browse, busy == Busy::Idle);
        enable(c.show, game.is_some());
        for control in [c.series_x, c.overlay, c.card, c.mouse, c.diag] {
            enable(control, busy == Busy::Idle);
        }
    }

    fn status(text: &str, color: u32) {
        STATUS_COLOR.store(color, Ordering::Relaxed);
        set_text(ctl().status, text);
    }

    fn set_game(path: &Path) {
        match settings::game_folder(path) {
            Some(folder) => state(|st| {
                st.cfg.game = Some(folder);
                st.cfg.save();
            }),
            None => {
                // Kept on screen (in red) so the user sees what was wrong.
                state(|st| st.cfg.game = Some(path.to_path_buf()));
            }
        }
        refresh();
        status(t("Pronto", "Ready"), GRAY);
    }

    fn read_options() {
        let c = ctl();
        // SAFETY: live controls.
        let (card, mouse) = unsafe {
            (
                SendMessageW(h(c.card), CB_GETCURSEL, 0, 0),
                SendMessageW(h(c.mouse), CB_GETCURSEL, 0, 0),
            )
        };
        let (series_x, overlay, diagnostic) =
            (checked(c.series_x), checked(c.overlay), checked(c.diag));
        state(|st| {
            st.cfg.series_x = series_x;
            st.cfg.overlay = overlay;
            st.cfg.diagnostic = diagnostic;
            st.cfg.small_card = card == 1;
            st.cfg.mouse = MOUSE_SPEEDS.get(mouse as usize).copied().unwrap_or(1.0);
            st.cfg.save();
        });
    }

    /// Play (`run`) or Prepare (`force` without `run`).
    fn go(run: bool, force: bool) {
        read_options();
        let plan = state(|st| {
            let engine = st.engine.clone()?;
            let game = st.cfg.game.as_deref().and_then(settings::game_folder)?;
            st.busy = Busy::Preparing;
            st.phase.clear();
            st.run_after = run;
            Some(Plan {
                engine,
                game,
                cfg: st.cfg.clone(),
                cache: settings::cache_dir(),
                force_prepare: force,
                run,
            })
        });
        let Some(plan) = plan else { return };
        refresh();
        status(t("Verificando…", "Checking…"), GRAY);
        let window = ctl().window;
        work::start(plan, move |event| {
            let boxed = Box::into_raw(Box::new(event));
            // SAFETY: our window; on failure the event is taken back.
            unsafe {
                if PostMessageW(h(window), WM_APP, 0, boxed as isize) == 0 {
                    drop(Box::from_raw(boxed));
                }
            }
        });
    }

    fn on_event(event: Event) {
        let c = ctl();
        match event {
            Event::Phase(phase) => {
                state(|st| st.phase = phase.clone());
                status(&phase, GRAY);
            }
            Event::Progress { done, total } => {
                // SAFETY: a live progress bar.
                unsafe {
                    if total == 0 {
                        set_marquee(c.progress, true);
                    } else {
                        set_marquee(c.progress, false);
                        SendMessageW(h(c.progress), PBM_SETRANGE32, 0, total as isize);
                        SendMessageW(h(c.progress), PBM_SETPOS, done.min(total) as usize, 0);
                    }
                }
                let phase = state(|st| st.phase.clone());
                if total > 0 {
                    status(
                        &format!("{phase}: {done} / {total} ({}%)", done * 100 / total.max(1)),
                        GRAY,
                    );
                }
            }
            Event::Prepared { recorded_none } => {
                set_marquee(c.progress, false);
                // SAFETY: a live progress bar.
                unsafe {
                    SendMessageW(h(c.progress), PBM_SETRANGE32, 0, 100);
                    SendMessageW(h(c.progress), PBM_SETPOS, 100, 0);
                }
                let run_after = state(|st| {
                    st.first_session = recorded_none;
                    st.phase.clear();
                    if !st.run_after {
                        st.busy = Busy::Idle;
                    }
                    st.run_after
                });
                if run_after {
                    status(t("Iniciando o jogo…", "Starting the game…"), GRAY);
                } else {
                    refresh();
                    if recorded_none {
                        status(
                            t(
                                "Shaders prontos. Os pipelines são gravados na primeira sessão de jogo.",
                                "Shaders ready. The pipelines are written down in the first game session.",
                            ),
                            GREEN,
                        );
                    } else {
                        status(
                            t("Shaders e pipelines prontos", "Shaders and pipelines ready"),
                            GREEN,
                        );
                    }
                }
            }
            Event::Started => {
                state(|st| st.busy = Busy::Running);
                refresh();
                let first = state(|st| st.first_session);
                if first {
                    status(
                        t(
                            "Jogo aberto. Primeira vez: feche e abra de novo depois para carregar completo.",
                            "Game running. First time: close and start it again afterwards for complete loading.",
                        ),
                        GREEN,
                    );
                } else {
                    status(t("Jogo aberto", "Game running"), GREEN);
                }
                // SAFETY: our window.
                unsafe { ShowWindow(h(c.window), SW_MINIMIZE) };
            }
            Event::Exited(code, session) => {
                state(|st| st.busy = Busy::Idle);
                refresh();
                if let Some(dir) = &session {
                    // The session's folder, with the summary picked.
                    open(
                        "explorer.exe",
                        &format!("/select,\"{}\"", dir.join("summary.txt").display()),
                    );
                }
                match code {
                    Some(0) if session.is_some() => status(
                        t(
                            "O jogo fechou. Diagnóstico salvo (pasta aberta no Explorer).",
                            "The game closed. Diagnostic saved (folder opened in Explorer).",
                        ),
                        GREEN,
                    ),
                    Some(0) => status(t("O jogo fechou", "The game closed"), GRAY),
                    other => status(
                        &format!(
                            "{} ({}). {}",
                            t("O jogo fechou com erro", "The game closed with an error"),
                            other.map_or("?".into(), |c| c.to_string()),
                            t("Veja em Logs: game.log", "See Logs: game.log")
                        ),
                        RED,
                    ),
                }
                // SAFETY: our window.
                unsafe {
                    ShowWindow(h(c.window), SW_RESTORE);
                    SetForegroundWindow(h(c.window));
                }
            }
            Event::Failed(why) => {
                state(|st| st.busy = Busy::Idle);
                set_marquee(c.progress, false);
                refresh();
                status(&format!("{}: {why}", t("Falhou", "Failed")), RED);
            }
        }
    }

    fn set_marquee(progress: isize, on: bool) {
        // SAFETY: a live progress bar; the style switch is what the control documents.
        unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                GWL_STYLE, GetWindowLongW, SetWindowLongW,
            };
            let style = GetWindowLongW(h(progress), GWL_STYLE) as u32;
            let want = if on {
                style | PBS_MARQUEE
            } else {
                style & !PBS_MARQUEE
            };
            if want != style {
                SetWindowLongW(h(progress), GWL_STYLE, want as i32);
            }
            SendMessageW(h(progress), PBM_SETMARQUEE, usize::from(on), 30);
        }
    }

    fn browse() {
        let c = ctl();
        let start = state(|st| st.cfg.game.clone()).map(|g| wide(&g.display().to_string()));
        let title = wide(t(
            "Escolha a pasta do jogo (a que tem game_scarlett_master.exe)",
            "Choose the game folder (the one with game_scarlett_master.exe)",
        ));
        let mut name = [0u16; 260];
        // SAFETY: a filled-in description; the returned list is freed below.
        unsafe {
            let mut info: BROWSEINFOW = std::mem::zeroed();
            info.hwndOwner = h(c.window);
            info.pszDisplayName = name.as_mut_ptr();
            info.lpszTitle = title.as_ptr();
            info.ulFlags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE | BIF_NONEWFOLDERBUTTON;
            info.lpfn = Some(browse_start);
            info.lParam = start.as_ref().map_or(0, |s| s.as_ptr() as isize);
            let list = SHBrowseForFolderW(&info);
            if list.is_null() {
                return;
            }
            let mut path = [0u16; 1024];
            let got = SHGetPathFromIDListW(list, path.as_mut_ptr()) != 0;
            CoTaskMemFree(list as *const c_void);
            if got {
                let len = path.iter().position(|&c| c == 0).unwrap_or(path.len());
                set_game(Path::new(&String::from_utf16_lossy(&path[..len])));
            }
        }
    }

    unsafe extern "system" fn browse_start(
        hwnd: HWND,
        message: u32,
        _: LPARAM,
        data: LPARAM,
    ) -> i32 {
        if message == BFFM_INITIALIZED && data != 0 {
            // SAFETY: the dialog is up; `data` is our terminated path.
            unsafe { SendMessageW(hwnd, BFFM_SETSELECTIONW, 1, data) };
        }
        0
    }

    fn open(file: &str, params: &str) {
        let (op, file, params) = (wide("open"), wide(file), wide(params));
        // SAFETY: terminated strings.
        unsafe {
            ShellExecuteW(
                h(ctl().window),
                op.as_ptr(),
                file.as_ptr(),
                params.as_ptr(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };
    }

    fn show_game() {
        let Some(game) = state(|st| st.cfg.game.clone()) else {
            return;
        };
        let exe = game.join(settings::TITLE_EXE);
        if exe.is_file() {
            open("explorer.exe", &format!("/select,\"{}\"", exe.display()));
        } else {
            open("explorer.exe", &format!("\"{}\"", game.display()));
        }
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let gdi = GDI.get().copied().unwrap_or_default();
        match message {
            WM_PAINT => {
                // SAFETY: the paint protocol on our own window.
                unsafe {
                    let mut ps: PAINTSTRUCT = std::mem::zeroed();
                    let dc = BeginPaint(hwnd, &mut ps);
                    let band = RECT {
                        left: 0,
                        top: 0,
                        right: s(WIDTH),
                        bottom: s(HEADER_H),
                    };
                    FillRect(dc, &band, gdi[1] as _);
                    SetBkMode(dc, 1); // transparent
                    SetTextColor(dc, rgb(255, 255, 255));
                    SelectObject(dc, gdi[3] as _);
                    let mut title_rect = RECT {
                        left: s(PAD),
                        top: s(10),
                        right: s(WIDTH - PAD),
                        bottom: s(46),
                    };
                    let title = wide("opti-vicity");
                    DrawTextW(dc, title.as_ptr(), -1, &mut title_rect, 0x20 | 0x800); // single line, no prefix
                    SetTextColor(dc, rgb(0xa9, 0xb1, 0xc2));
                    SelectObject(dc, gdi[4] as _);
                    let mut sub_rect = RECT {
                        left: s(PAD),
                        top: s(48),
                        right: s(WIDTH - PAD),
                        bottom: s(68),
                    };
                    let sub = wide(&format!(
                        "{}  ·  build {}",
                        t(
                            "Overlay, mouse e teclado, carregamento corrigido",
                            "Overlay, mouse and keyboard, fixed first loads"
                        ),
                        option_env!("OPTI_BUILD").unwrap_or("?")
                    ));
                    DrawTextW(dc, sub.as_ptr(), -1, &mut sub_rect, 0x20 | 0x800);
                    EndPaint(hwnd, &ps);
                }
                0
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
                let c = ctl();
                let child = lparam;
                let color = if child == c.hint {
                    HINT_COLOR.load(Ordering::Relaxed)
                } else if child == c.status {
                    STATUS_COLOR.load(Ordering::Relaxed)
                } else {
                    rgb(0x20, 0x22, 0x26)
                };
                // SAFETY: the control's device context, handed to us for this.
                unsafe {
                    SetTextColor(wparam as _, color);
                    SetBkColor(wparam as _, rgb(255, 255, 255));
                }
                gdi[0]
            }
            WM_DROPFILES => {
                let drop = wparam as *mut c_void;
                let mut buf = [0u16; 1024];
                // SAFETY: the drop handle Windows gave us, released after.
                let len = unsafe {
                    let len = DragQueryFileW(drop as _, 0, buf.as_mut_ptr(), buf.len() as u32);
                    DragFinish(drop as _);
                    len as usize
                };
                if len > 0 && state(|st| st.busy) == Busy::Idle {
                    set_game(Path::new(&String::from_utf16_lossy(
                        &buf[..len.min(buf.len())],
                    )));
                }
                0
            }
            WM_COMMAND => {
                let id = wparam & 0xffff;
                let code = ((wparam >> 16) & 0xffff) as u32;
                match id {
                    ID_PLAY | IDOK if code == BN_CLICKED => {
                        if state(|st| st.busy) == Busy::Idle {
                            go(true, false);
                        }
                    }
                    ID_PREPARE if code == BN_CLICKED => go(false, true),
                    ID_BROWSE if code == BN_CLICKED => browse(),
                    ID_SHOW if code == BN_CLICKED => show_game(),
                    ID_LOGS if code == BN_CLICKED => {
                        let logs = work::logs_folder();
                        let _ = std::fs::create_dir_all(&logs);
                        open("explorer.exe", &format!("\"{}\"", logs.display()));
                    }
                    ID_SERIES_X | ID_OVERLAY | ID_DIAG if code == BN_CLICKED => read_options(),
                    ID_CARD | ID_MOUSE if code == CBN_SELCHANGE => read_options(),
                    _ => {}
                }
                0
            }
            WM_APP => {
                // SAFETY: boxed by `go`'s poster for this message.
                let event = unsafe { Box::from_raw(lparam as *mut Event) };
                on_event(*event);
                0
            }
            WM_CLOSE => {
                // SAFETY: our window.
                unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
            }
            WM_DESTROY => {
                // SAFETY: ends the message loop.
                unsafe { PostQuitMessage(0) };
                0
            }
            // SAFETY: everything else as Windows does it.
            _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
        }
    }
}
