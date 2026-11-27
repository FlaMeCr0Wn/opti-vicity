//! What the launcher remembers, where things are, and the settings the game
//! runs with (Yaekai's `scripts/play-3070.ps1` set, with the changes the README lists).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The title's executable in a dump's folder.
pub const TITLE_EXE: &str = "game_scarlett_master.exe";

/// The user interface in Portuguese (the system's language) or English.
pub fn portuguese() -> bool {
    static PT: OnceLock<bool> = OnceLock::new();
    *PT.get_or_init(|| {
        // SAFETY: a plain query.
        let lang = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
        lang & 0x3ff == 0x16
    })
}

/// `pt` in Portuguese, else `en`.
pub fn t(pt: &'static str, en: &'static str) -> &'static str {
    if portuguese() { pt } else { en }
}

/// The launcher's settings, kept in `%LOCALAPPDATA%\opti-vicity\launcher.ini`.
#[derive(Clone, Debug)]
pub struct Config {
    pub game: Option<PathBuf>,
    pub series_x: bool,
    pub overlay: bool,
    /// An 8 GB card: the top texture level is dropped (`VICTORY_TEXTURE_MIP_DROP=1`).
    pub small_card: bool,
    /// Mouse look speed (`VICTORY_MOUSE`; 0 = the mouse is left alone).
    pub mouse: f32,
    /// Diagnostic mode: the detailed log, every long frame, and a sample a
    /// second of memory and disk, with F8 marking the hitches felt
    /// (`logs\diag\<date_time>\`).
    pub diagnostic: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            game: None,
            series_x: false,
            overlay: true,
            small_card: false,
            mouse: 1.0,
            diagnostic: false,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let mut cfg = Self::default();
        let Ok(text) = std::fs::read_to_string(config_file()) else {
            return cfg;
        };
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            let on = value == "1";
            match key {
                "game" if !value.is_empty() => cfg.game = Some(PathBuf::from(value)),
                "series_x" => cfg.series_x = on,
                "overlay" => cfg.overlay = on,
                "small_card" => cfg.small_card = on,
                "mouse" => cfg.mouse = value.parse().unwrap_or(1.0),
                "diagnostic" => cfg.diagnostic = on,
                _ => {}
            }
        }
        cfg
    }

    pub fn save(&self) {
        let game = self
            .game
            .as_ref()
            .map(|g| g.display().to_string())
            .unwrap_or_default();
        let text = format!(
            "game={game}\nseries_x={}\noverlay={}\nsmall_card={}\nmouse={}\ndiagnostic={}\n",
            u8::from(self.series_x),
            u8::from(self.overlay),
            u8::from(self.small_card),
            self.mouse,
            u8::from(self.diagnostic)
        );
        let _ = std::fs::create_dir_all(local_dir());
        let _ = std::fs::write(config_file(), text);
    }
}

/// `%LOCALAPPDATA%\opti-vicity`: settings, cache and logs.
pub fn local_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("opti-vicity")
}

fn config_file() -> PathBuf {
    local_dir().join("launcher.ini")
}

pub fn logs_dir() -> PathBuf {
    local_dir().join("logs")
}

/// Translated shaders and compiled pipelines. The first time, the cache of
/// the earlier `play-unificado.ps1` launcher (opti-vicity before this launcher) is taken over (same build, same
/// translations).
pub fn cache_dir() -> PathBuf {
    let cache = local_dir().join("cache");
    if !cache.exists()
        && let Some(old) = std::env::var_os("LOCALAPPDATA")
            .map(|l| PathBuf::from(l).join(r"victory\cache-unificado"))
        && old.is_dir()
    {
        let _ = std::fs::create_dir_all(local_dir());
        let _ = std::fs::rename(&old, &cache);
    }
    cache
}

/// The engine (`victory.exe`): in `bin\` beside the launcher, or beside it.
pub fn engine() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    [dir.join(r"bin\victory.exe"), dir.join("victory.exe")]
        .into_iter()
        .find(|p| p.is_file())
}

/// A dump's folder from a dropped or picked path: the folder itself, or the
/// folder of its `game_scarlett_master.exe`.
pub fn game_folder(path: &Path) -> Option<PathBuf> {
    let folder = if path.is_file() {
        path.parent()?.to_path_buf()
    } else {
        path.to_path_buf()
    };
    folder.join(TITLE_EXE).is_file().then_some(folder)
}

/// What the cache says about this engine build: shaders and pipelines made
/// ahead for it. Changes with the engine file.
pub fn engine_stamp(engine: &Path) -> String {
    let meta = std::fs::metadata(engine).ok();
    let len = meta.as_ref().map_or(0, |m| m.len());
    let modified = meta
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    format!("{len}:{modified}")
}

pub fn stamp_file(cache: &Path) -> PathBuf {
    cache.join("opti-prepared.txt")
}

pub fn prepared(engine: &Path, cache: &Path) -> bool {
    std::fs::read_to_string(stamp_file(cache)).is_ok_and(|s| s.trim() == engine_stamp(engine))
}

/// The environment the engine runs with: Yaekai's play-3070 set with full
/// textures (or MipDrop 1 on 8 GB cards), the first-run fixes and the
/// launcher's choices.
pub fn environment(cfg: &Config, game: &Path, cache: &Path) -> Vec<(String, String)> {
    let mut set: Vec<(String, String)> = vec![
        ("VICTORY_GAME_DIR".into(), game.display().to_string()),
        ("VICTORY_CACHE_DIR".into(), cache.display().to_string()),
    ];
    let fixed: &[(&str, &str)] = &[
        ("VICTORY_LOG", "warn"),
        ("VICTORY_PIPELINE_BOOK", "1"),
        ("VICTORY_FRAME_INTERVAL_MS", "16.667"),
        ("VICTORY_DYNAMIC_SLOTS", "524288"),
        ("VICTORY_MAX_PRELUDES", "32"),
        ("VICTORY_CARD_BUFFERS_MB", "1024"),
        ("VICTORY_CARD_WINDOWS_MB", "512"),
        ("VICTORY_TEXTURE_ROOM_PERCENT", "94"),
        ("VICTORY_PAGE_POOL", "1"),
        ("VICTORY_STAGING_PACK", "1"),
        ("VICTORY_LEVELS_ON_DEMAND", "0"),
        ("VICTORY_WAIT_RESIDENT", "0"),
        ("VICTORY_JOIN_COPIES", "0"),
        ("VICTORY_REFRESH_ONCE", "1"),
        ("VICTORY_CARD_REHASH_RUNS", "1"),
        ("VICTORY_BACK_OWN_VIEW", "1"),
        ("VICTORY_WIDE_MERGE", "1"),
        ("VICTORY_MIRROR_FAST", "1"),
        ("VICTORY_ASYNC_SUBMIT", "1"),
        ("VICTORY_QUEUE2", "1"),
        ("VICTORY_INDIRECT_WINDOW", "1"),
        ("VICTORY_TEXTURE_NO_WAIT", "1"),
        ("VICTORY_COPY_PLAIN", "1"),
        ("VICTORY_WIDE_TEXTURES", "1"),
        ("VICTORY_INDIRECT_GATHER", "1"),
        ("VICTORY_LATE_WAIT_LISTS", "1024"),
        ("VICTORY_CARD_FILLS_BATCH", "0"),
        ("VICTORY_GATHER_DISPATCH", "0"),
        ("VICTORY_RELAY", "2"),
        ("VICTORY_WIDE_BACK", "1"),
        ("VICTORY_RELAY_COPY", "1"),
        ("VICTORY_CARD_STEPS", "2"),
        ("VICTORY_INDIRECT_BATCH", "2"),
        ("VICTORY_MAP_THREADS", "1"),
        ("VICTORY_CARD_AWAKE", "0"),
        ("VICTORY_SHOW_AFTER", "1"),
        ("VICTORY_JOIN_PARTS", "8"),
        ("VICTORY_UPLOAD_AHEAD", "1"),
        ("VICTORY_CARD_BUFFERS_MIN_KB", "64"),
        ("VICTORY_POOL_DISCARD", "1"),
        ("VICTORY_FILL_TITLE", "1"),
        ("VICTORY_PARTIAL_DELTA", "1"),
        ("VICTORY_BAKE_DELTA", "1"),
        ("VICTORY_MAP_LOCK_FINE", "1"),
        ("VICTORY_POOL_CAP", "1"),
        ("VICTORY_TABLE_MEMO", "1"),
        ("VICTORY_LIST_ALIAS_EXACT", "1"),
        ("VICTORY_LIST_HOLD", "1"),
        ("VICTORY_EARLY_TEXTURES_MS", "40"),
        // The title's character/decoration bake waits up to 10 s for its textures.
        ("VICTORY_BAKE_TEXTURES_MS", "10000"),
        ("VICTORY_HEAP_PASS_MAX", "0"),
        ("VICTORY_HEAP_CHANGED_MAX", "0"),
        (
            "VICTORY_SKIP_PIPELINES",
            "02384a60fda6f2d0,731a39171b568298",
        ),
        ("VICTORY_WC_NO_BACK", "1"),
        ("VICTORY_WC_NO_BACK_MIN_KB", "1024"),
        ("VICTORY_SYNC_IN_PLACE", "1"),
        ("VICTORY_SPANS_QUICK", "1"),
        ("VICTORY_SYNC_FINE", "1"),
        ("VICTORY_HEAP_SYNC_FIRST", "1"),
        // No ray tracing: the stand-in ambient light.
        ("VICTORY_AMBIENT", "0.8"),
        ("VICTORY_AMBIENT_AIR", "1.5"),
        // First visits: wait up to 2 s for a pipeline still compiling instead of skipping the draw.
        ("VICTORY_PIPELINE_WAIT_MS", "2000"),
        // Pipelines with static samplers kept in the library too: the 3080 Ti
        // reads them right when loaded in another process (examples/
        // sampler_order_probe), and ~1,600 are then not compiled each session.
        ("VICTORY_LIBRARY_STATIC_SAMPLERS", "1"),
    ];
    set.extend(fixed.iter().map(|&(k, v)| (k.to_owned(), v.to_owned())));
    set.push((
        "VICTORY_TEXTURE_MIP_DROP".into(),
        if cfg.small_card { "1" } else { "0" }.into(),
    ));
    set.push(("VICTORY_MOUSE".into(), format!("{}", cfg.mouse)));
    if !cfg.overlay {
        set.push(("VICTORY_OVERLAY".into(), "0".into()));
    }
    if cfg.diagnostic {
        // The detailed log, a line for every long frame with what it was
        // made of, and where the threads wait when no frame comes for 5 s.
        for (key, value) in [
            ("VICTORY_LOG", "info"),
            ("VICTORY_FRAME_LOG", "1"),
            ("VICTORY_HANG_WATCH_S", "5"),
        ] {
            set.retain(|(k, _)| k != key);
            set.push((key.into(), value.into()));
        }
    }
    set
}
