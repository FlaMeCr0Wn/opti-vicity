//! The F1 options menu, after VIctory-NVIDIA's (trustno1's `host/menu.rs`):
//! F1 shows and hides it while the game window is in front; Up/Down pick a
//! line, Left/Right change a setting, Enter runs the line picked, Esc closes.
//! While it is shown the title sees no keyboard and no mouse
//! ([`input_blocked`], read by the input module).
//!
//! Frame generation switches at once when this run started with it ready
//! (AMD's: `VICTORY_FRAMEGEN=amd`, the launcher asks for it whenever AMD's PC
//! FSR upscales; DLSS's: Streamline's swap chain, RTX 40 and later); the other
//! settings are the launcher's and apply at the next start: they are written
//! to its settings file (`VICTORY_MENU_FILE`), and "save and restart" ends this
//! run with exit code [`RESTART_CODE`], on which the launcher starts the game
//! again (`VICTORY_MENU_RESTART=1`).
//!
//! Drawn by the overlay thread (`crate::overlay`), which also polls the keys.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

/// The exit code that asks the launcher to start the game again.
pub const RESTART_CODE: u32 = 82;

/// The settings UI in Portuguese (the system's language) or English.
pub(crate) fn portuguese() -> bool {
    static PT: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    // SAFETY: a plain query.
    *PT.get_or_init(|| unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() } & 0x3ff == 0x16)
}

pub(crate) fn t(pt: &'static str, en: &'static str) -> &'static str {
    if portuguese() { pt } else { en }
}

/// Keys still held when the menu closed (Esc, Enter, F1): the title sees
/// them only once they are let go, or Esc would also open its pause menu.
static HELD_AT_CLOSE: AtomicBool = AtomicBool::new(false);

/// The title sees no keyboard and no mouse: the menu is shown, or a key that
/// closed it is still down.
pub fn input_blocked() -> bool {
    if crate::SETTINGS_MENU_OPEN.load(Relaxed) {
        return true;
    }
    if HELD_AT_CLOSE.load(Relaxed) {
        const VK: [i32; 3] = [0x1b, 0x0d, 0x70]; // Esc, Enter, F1
        // SAFETY: plain queries.
        if VK.iter().any(|&k| unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(k) } < 0) {
            return true;
        }
        HELD_AT_CLOSE.store(false, Relaxed);
    }
    false
}

/// A setting: its key in the launcher's settings file, its name and the
/// values it takes (`(value, shown)`).
pub(crate) struct Item {
    pub key: &'static str,
    pub label: &'static str,
    pub values: Vec<(&'static str, String)>,
}

/// The frame generation values: off, AMD's (2x), DLSS's 2x to 6x.
const FG_DLSS: [&str; 5] = ["2", "3", "4", "5", "6"];

fn items() -> Vec<Item> {
    let owned = |v: &[(&'static str, &str)]| v.iter().map(|&(a, b)| (a, b.to_owned())).collect::<Vec<_>>();
    let mut framegen = vec![("0", t("DESLIGADA", "OFF").to_owned()), ("amd", "AMD FSR 3  (2x)".to_owned())];
    framegen.extend(FG_DLSS.iter().map(|&x| (x, format!("DLSS  {x}x{}", if x == "6" { t("  (máx.)", "  (most)") } else { "" }))));
    vec![
        Item { key: "framegen", label: t("Geração de quadros (FG)", "Frame generation (FG)"), values: framegen },
        Item {
            key: "upscaler",
            label: t("Upscaler", "Upscaler"),
            values: owned(&[
                ("game", t("DO JOGO (FSR do console)", "THE GAME'S (console FSR)")),
                ("fsr", "AMD FSR 3.1 (PC)"),
                ("dlss", t("NVIDIA DLSS (RTX)", "NVIDIA DLSS (RTX)")),
            ]),
        },
        Item {
            key: "render_scale",
            label: t("Resolução interna", "Render resolution"),
            values: owned(&[
                ("quality", t("QUALIDADE (67%)", "QUALITY (67%)")),
                ("balanced", t("EQUILIBRADO (59%)", "BALANCED (59%)")),
                ("performance", t("DESEMPENHO (50%)", "PERFORMANCE (50%)")),
                ("ultraperformance", t("ULTRA DESEMPENHO (33%)", "ULTRA PERFORMANCE (33%)")),
                ("native", t("NATIVA (100%)", "NATIVE (100%)")),
            ]),
        },
        Item { key: "series_x", label: t("Console", "Console"), values: owned(&[("0", "SERIES S"), ("1", "SERIES X")]) },
        Item {
            key: "overlay",
            label: t("Painel de desempenho (F10)", "Performance panel (F10)"),
            values: owned(&[("1", t("LIGADO", "ON")), ("0", t("DESLIGADO", "OFF"))]),
        },
    ]
}

pub(crate) fn actions() -> [&'static str; 3] {
    [
        t("Salvar e reiniciar agora", "Save and restart now"),
        t("Salvar para o próximo início", "Save for the next start"),
        t("Fechar", "Close"),
    ]
}

/// What this run started with, as the launcher's settings say it.
fn started_value(key: &str) -> String {
    let var = |v: &str| std::env::var(v).ok().map(|s| s.trim().to_ascii_lowercase()).filter(|s| !s.is_empty());
    match key {
        "framegen" => match var("VICTORY_FRAMEGEN").as_deref() {
            Some("amd") => if var("VICTORY_FRAMEGEN_START").as_deref() == Some("0") { "0" } else { "amd" }.to_owned(),
            Some("dlss" | "1") => var("VICTORY_FRAMEGEN_X").filter(|x| FG_DLSS.contains(&x.as_str())).unwrap_or_else(|| "0".into()),
            _ => "0".into(),
        },
        "upscaler" => {
            if var("VICTORY_DLSS").as_deref() == Some("1") {
                "dlss".into()
            } else if var("VICTORY_FIDELITYFX_DIR").is_some() {
                "fsr".into()
            } else {
                "game".into()
            }
        }
        "render_scale" => var("VICTORY_RENDER_SCALE")
            .or_else(|| var("VICTORY_DLSS_MODE").filter(|_| var("VICTORY_DLSS").as_deref() == Some("1")))
            .map(|s| if s == "dlaa" { "native".into() } else { s })
            .unwrap_or_else(|| "quality".into()),
        "series_x" => if crate::console::current() == crate::console::Console::SeriesX { "1" } else { "0" }.into(),
        "overlay" => if var("VICTORY_OVERLAY").as_deref() == Some("0") { "0" } else { "1" }.into(),
        _ => String::new(),
    }
}

/// Whether frame generation value `value` shows at once in this run.
fn framegen_live(value: &str) -> bool {
    match value {
        "0" => crate::framegen::ready() || crate::streamline::framegen_live(),
        "amd" => crate::framegen::ready(),
        _ => crate::streamline::framegen_live(),
    }
}

/// The menu's state: the values chosen, the line picked, a note.
pub(crate) struct Menu {
    pub items: Vec<Item>,
    pub values: Vec<String>,
    pub started: Vec<String>,
    pub at: usize,
    pub note: String,
}

pub(crate) static MENU: Mutex<Option<Menu>> = Mutex::new(None);

impl Menu {
    fn new() -> Menu {
        let items = items();
        let values: Vec<String> = items.iter().map(|i| started_value(i.key)).collect();
        Menu { started: values.clone(), values, items, at: 0, note: String::new() }
    }

    pub fn lines(&self) -> usize {
        self.items.len() + actions().len()
    }

    /// The value shown for item `i`, and whether a change of it shows at once.
    pub fn shown(&self, i: usize) -> (String, bool) {
        let item = &self.items[i];
        let value = &self.values[i];
        let shown = item.values.iter().find(|(v, _)| v == value).map_or_else(|| value.to_ascii_uppercase(), |(_, s)| s.clone());
        (shown, self.live(i))
    }

    pub fn live(&self, i: usize) -> bool {
        match self.items[i].key {
            "overlay" => true,
            "framegen" => framegen_live(&self.values[i]) && framegen_live(&self.started[i]),
            _ => false,
        }
    }

    pub fn changed(&self, i: usize) -> bool {
        self.values[i] != self.started[i]
    }

    fn change(&mut self, i: usize, step: isize) {
        let n = self.items[i].values.len() as isize;
        let now = self.items[i].values.iter().position(|(v, _)| *v == self.values[i]).map_or(-1, |x| x as isize);
        let next = if now < 0 { if step > 0 { 0 } else { n - 1 } } else { (now + step).rem_euclid(n) };
        self.values[i] = self.items[i].values[next as usize].0.to_owned();
        self.note.clear();
        self.apply_live(i);
    }

    /// A setting that shows at once: applied now.
    fn apply_live(&self, i: usize) {
        let value = self.values[i].as_str();
        match self.items[i].key {
            "overlay" => crate::overlay::set_visible(value != "0"),
            "framegen" if framegen_live(value) => match value {
                "amd" => crate::framegen::set_enabled(true),
                "0" => {
                    if crate::framegen::ready() {
                        crate::framegen::set_enabled(false);
                    }
                    if crate::streamline::framegen_live() {
                        crate::streamline::set_framegen(0);
                    }
                }
                x => crate::streamline::set_framegen(x.parse().unwrap_or(0)),
            },
            _ => {}
        }
    }

    /// Writes the changed values over those in the launcher's settings file.
    fn save(&mut self) -> bool {
        let Some(path) = std::env::var_os("VICTORY_MENU_FILE") else {
            self.note = t("Sem arquivo de opções: abra pelo launcher", "No settings file: start from the launcher").into();
            return false;
        };
        let mut lines: Vec<String> = std::fs::read_to_string(&path).unwrap_or_default().lines().map(str::to_owned).collect();
        for (i, item) in self.items.iter().enumerate() {
            if !self.changed(i) {
                continue;
            }
            lines.retain(|l| l.split_once('=').is_none_or(|(k, _)| k.trim() != item.key));
            lines.push(format!("{}={}", item.key, self.values[i]));
        }
        let mut text = lines.join("\n");
        text.push('\n');
        match std::fs::write(&path, text) {
            Ok(()) => {
                tracing::info!(path = %std::path::Path::new(&path).display(), "options menu: saved");
                self.started = self.values.clone();
                self.note = t("Salvo: vale no próximo início", "Saved: applies at the next start").into();
                true
            }
            Err(e) => {
                tracing::warn!(error = %e, "options menu: not saved");
                self.note = t("Não consegui salvar o arquivo de opções", "Could not save the settings file").into();
                false
            }
        }
    }

    /// Ends this run for the launcher to start the game again.
    fn restart(&mut self) {
        if std::env::var_os("VICTORY_MENU_RESTART").is_none_or(|v| v != "1") {
            self.note = t("Salvo. Abra o jogo de novo pelo launcher", "Saved. Start the game again from the launcher").into();
            return;
        }
        tracing::info!("options menu: restarting through the launcher");
        // Ended at once: unloading the DLLs (Streamline's) may not come back.
        // SAFETY: this process, with nothing of its own left to save.
        unsafe {
            use windows_sys::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};
            TerminateProcess(GetCurrentProcess(), RESTART_CODE);
        }
    }

    /// A key pressed (a virtual-key code) while the game window is in front.
    /// Returns whether the menu changed.
    fn key(&mut self, vk: u16) -> bool {
        const F1: u16 = 0x70;
        const ESC: u16 = 0x1b;
        const ENTER: u16 = 0x0d;
        const LEFT: u16 = 0x25;
        const UP: u16 = 0x26;
        const RIGHT: u16 = 0x27;
        const DOWN: u16 = 0x28;
        let open = crate::SETTINGS_MENU_OPEN.load(Relaxed);
        if vk == F1 || (open && vk == ESC) {
            let now = !open && vk == F1;
            if !now {
                HELD_AT_CLOSE.store(true, Relaxed);
            }
            crate::SETTINGS_MENU_OPEN.store(now, Relaxed);
            self.note.clear();
            return true;
        }
        if !open {
            return false;
        }
        let total = self.lines();
        let items = self.items.len();
        match vk {
            UP => self.at = (self.at + total - 1) % total,
            DOWN => self.at = (self.at + 1) % total,
            LEFT if self.at < items => self.change(self.at, -1),
            RIGHT | ENTER if self.at < items => self.change(self.at, 1),
            ENTER => match self.at - items {
                0 => {
                    if self.save() {
                        self.restart();
                    }
                }
                1 => {
                    self.save();
                }
                _ => {
                    HELD_AT_CLOSE.store(true, Relaxed);
                    crate::SETTINGS_MENU_OPEN.store(false, Relaxed);
                }
            },
            _ => return false,
        }
        true
    }
}

/// The keys the menu watches.
pub(crate) const WATCHED: [u16; 7] = [0x70, 0x26, 0x28, 0x25, 0x27, 0x0d, 0x1b];

/// Keys newly pressed (by the overlay thread's poll): returns whether the
/// menu changed (to draw it again).
pub(crate) fn pressed(keys: &[u16]) -> bool {
    let mut menu = MENU.lock().unwrap_or_else(|e| e.into_inner());
    let menu = menu.get_or_insert_with(Menu::new);
    let mut changed = false;
    for &vk in keys {
        changed |= menu.key(vk);
    }
    changed
}

/// `VICTORY_MENU_OPEN=1`: shown from the start (to look at it in a run
/// nobody presses keys in).
pub(crate) fn open_at_start() {
    if std::env::var_os("VICTORY_MENU_OPEN").is_some_and(|v| v == "1") {
        crate::SETTINGS_MENU_OPEN.store(true, Relaxed);
    }
}

/// Runs `f` on the menu (made on first use).
pub(crate) fn with<R>(f: impl FnOnce(&Menu) -> R) -> R {
    let mut menu = MENU.lock().unwrap_or_else(|e| e.into_inner());
    f(menu.get_or_insert_with(Menu::new))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_cycle_both_ways() {
        let mut menu = Menu::new();
        let n = menu.items[2].values.len();
        let first = menu.values[2].clone();
        for _ in 0..n {
            menu.change(2, 1);
        }
        assert_eq!(menu.values[2], first);
        menu.change(2, -1);
        assert!(menu.changed(2));
    }

    #[test]
    fn every_item_starts_on_one_of_its_values() {
        let menu = Menu::new();
        for (i, item) in menu.items.iter().enumerate() {
            assert!(item.values.iter().any(|(v, _)| *v == menu.values[i]), "{}: {}", item.key, menu.values[i]);
        }
    }
}
