//! Diagnostic mode: what is needed to find where hitches come from.
//!
//! A session folder (`logs\diag\<date_time>\`) gets the engine's detailed
//! log (`game.log`, with a line for every long frame), the settings it ran
//! with, a sample a second while the game runs (`samples.tsv`: the window's
//! fps/ms/draws, free memory and free commit, the game's memory, its disk
//! reads) and the F8 presses (`marks.txt`, "a hitch was felt here"). When the
//! game closes, `summary.txt` puts the main numbers together.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM, SYSTEMTIME, WAIT_TIMEOUT};
use windows_sys::Win32::System::Diagnostics::Debug::Beep;
use windows_sys::Win32::System::ProcessStatus::{
    GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
};
use windows_sys::Win32::System::SystemInformation::{
    GetLocalTime, GetSystemTime, GlobalMemoryStatusEx, MEMORYSTATUSEX,
};
use windows_sys::Win32::System::Threading::{
    GetProcessIoCounters, IO_COUNTERS, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_SYNCHRONIZE, WaitForSingleObject,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, GetWindowThreadProcessId,
};

use crate::settings;

const VK_F8: i32 = 0x77;
const MIB: f64 = 1024.0 * 1024.0;
/// The engine's memory brake comes on at this much free commit or less.
const PRESSURE_ON_MIB: f64 = 4096.0;

fn local_time() -> SYSTEMTIME {
    // SAFETY: a plain query into a local.
    unsafe {
        let mut t: SYSTEMTIME = std::mem::zeroed();
        GetLocalTime(&mut t);
        t
    }
}

/// UTC, as the engine's log lines have it: `2026-11-27T09:12:33.123Z`.
fn utc_now() -> String {
    // SAFETY: a plain query into a local.
    let t = unsafe {
        let mut t: SYSTEMTIME = std::mem::zeroed();
        GetSystemTime(&mut t);
        t
    };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds
    )
}

fn clock() -> String {
    let t = local_time();
    format!("{:02}:{:02}:{:02}", t.wHour, t.wMinute, t.wSecond)
}

/// A new session folder: `logs\diag\2026-11-27_091233`.
pub fn new_session() -> std::io::Result<PathBuf> {
    let t = local_time();
    let dir = settings::logs_dir().join("diag").join(format!(
        "{:04}-{:02}-{:02}_{:02}{:02}{:02}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond
    ));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

struct Memory {
    free_ram: f64,
    free_commit: f64,
    commit_limit: f64,
    total_ram: f64,
}

fn memory() -> Memory {
    // SAFETY: a sized struct for the query.
    let m = unsafe {
        let mut m: MEMORYSTATUSEX = std::mem::zeroed();
        m.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
        GlobalMemoryStatusEx(&mut m);
        m
    };
    Memory {
        free_ram: m.ullAvailPhys as f64 / MIB,
        free_commit: m.ullAvailPageFile as f64 / MIB,
        commit_limit: m.ullTotalPageFile as f64 / MIB,
        total_ram: m.ullTotalPhys as f64 / MIB,
    }
}

/// The settings the game runs with, and the machine's memory at the start.
pub fn write_settings(dir: &Path, env: &[(String, String)]) {
    let mut text = String::new();
    let mut env: Vec<_> = env.iter().collect();
    env.sort();
    for (k, v) in env {
        let _ = writeln!(text, "{k}={v}");
    }
    let m = memory();
    let _ = writeln!(
        text,
        "\nram_total_mb={:.0}\nram_free_mb={:.0}\ncommit_limit_mb={:.0}\ncommit_free_mb={:.0}\nstarted_utc={}\nlauncher_build={}",
        m.total_ram,
        m.free_ram,
        m.commit_limit,
        m.free_commit,
        utc_now(),
        option_env!("OPTI_BUILD").unwrap_or("?")
    );
    let _ = std::fs::write(dir.join("settings.txt"), text);
}

/// One second of the session.
struct Sample {
    t: f64,
    ms: Option<f64>,
    free_ram: f64,
    free_commit: f64,
    private: f64,
    read_mb_s: f64,
}

/// What the sampler saw, for the summary.
pub struct Seen {
    samples: Vec<Sample>,
    /// F8 presses: seconds since the start, and the UTC time.
    marks: Vec<(f64, String)>,
    commit_limit: f64,
}

struct TitleSearch {
    pid: u32,
    title: String,
}

unsafe extern "system" fn each_window(hwnd: HWND, data: LPARAM) -> i32 {
    // SAFETY: `data` is the TitleSearch `game_title` passes; the window is live.
    unsafe {
        let search = &mut *(data as *mut TitleSearch);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid != search.pid {
            return 1;
        }
        let mut buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
        let text = String::from_utf16_lossy(&buf[..len]);
        if text.starts_with("Scarlett") {
            search.title = text;
            return 0;
        }
        1
    }
}

/// The game window's caption: `Scarlett | <game> | F FPS | M ms | D draws`.
fn game_title(pid: u32) -> String {
    let mut search = TitleSearch {
        pid,
        title: String::new(),
    };
    // SAFETY: the callback only reads windows and writes `search`, which outlives the call.
    unsafe { EnumWindows(Some(each_window), &mut search as *mut TitleSearch as LPARAM) };
    search.title
}

/// (fps, ms, draws) from the caption.
fn caption_numbers(title: &str) -> (Option<f64>, Option<f64>, Option<f64>) {
    let mut out = (None, None, None);
    for part in title.split(" | ") {
        let part = part.trim();
        let number = |suffix: &str| part.strip_suffix(suffix)?.trim().parse::<f64>().ok();
        if let Some(v) = number(" FPS") {
            out.0 = Some(v);
        } else if let Some(v) = number(" ms") {
            out.1 = Some(v);
        } else if let Some(v) = number(" draws") {
            out.2 = Some(v);
        }
    }
    out
}

fn opt(v: Option<f64>, digits: usize) -> String {
    v.map(|v| format!("{v:.digits$}")).unwrap_or_default()
}

/// Samples the game (`pid`) until it exits: a row a second, F8 polled every
/// 100 ms (a beep says it was taken).
pub fn sample(dir: &Path, pid: u32) -> Seen {
    let mut seen = Seen {
        samples: Vec::new(),
        marks: Vec::new(),
        commit_limit: memory().commit_limit,
    };
    // SAFETY: a query handle on the game, closed below.
    let process = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            pid,
        )
    };
    if process.is_null() {
        return seen;
    }
    let mut rows = std::fs::File::create(dir.join("samples.tsv")).ok();
    let mut marks = std::fs::File::create(dir.join("marks.txt")).ok();
    if let Some(f) = rows.as_mut() {
        let _ = writeln!(
            f,
            "t_s\tclock\tutc\tfps\tms\tdraws\tfree_ram_mb\tfree_commit_mb\tcommit_limit_mb\tgame_private_mb\tgame_ws_mb\tdisk_read_mb_s\tdisk_read_ops_s\tf8"
        );
    }
    let start = Instant::now();
    let mut next = start;
    let mut f8_held = false;
    let mut last_io: Option<(Instant, IO_COUNTERS)> = None;
    loop {
        // SAFETY: our handle; 100 ms at most.
        if unsafe { WaitForSingleObject(process, 100) } != WAIT_TIMEOUT {
            break;
        }
        // SAFETY: a plain key state query.
        let down = unsafe { GetAsyncKeyState(VK_F8) } < 0;
        if down && !f8_held {
            let t = start.elapsed().as_secs_f64();
            let utc = utc_now();
            if let Some(f) = marks.as_mut() {
                let _ = writeln!(f, "{t:.1}\t{}\t{utc}", clock());
            }
            seen.marks.push((t, utc));
            // SAFETY: a short tone.
            unsafe { Beep(880, 80) };
        }
        f8_held = down;
        let now = Instant::now();
        if now < next {
            continue;
        }
        next += Duration::from_secs(1);
        let m = memory();
        // SAFETY: sized structs for queries on our handle.
        let (pmc, io) = unsafe {
            let mut pmc: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
            pmc.cb = size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
            GetProcessMemoryInfo(
                process,
                &mut pmc as *mut PROCESS_MEMORY_COUNTERS_EX as *mut PROCESS_MEMORY_COUNTERS,
                pmc.cb,
            );
            let mut io: IO_COUNTERS = std::mem::zeroed();
            GetProcessIoCounters(process, &mut io);
            (pmc, io)
        };
        let (mut read_mb_s, mut read_ops_s) = (0.0, 0.0);
        if let Some((at, before)) = &last_io {
            let dt = now.duration_since(*at).as_secs_f64();
            if dt > 0.0 {
                read_mb_s = io
                    .ReadTransferCount
                    .saturating_sub(before.ReadTransferCount) as f64
                    / MIB
                    / dt;
                read_ops_s = io
                    .ReadOperationCount
                    .saturating_sub(before.ReadOperationCount) as f64
                    / dt;
            }
        }
        last_io = Some((now, io));
        let (fps, ms, draws) = caption_numbers(&game_title(pid));
        let t = now.duration_since(start).as_secs_f64();
        let private = pmc.PrivateUsage as f64 / MIB;
        if let Some(f) = rows.as_mut() {
            let _ = writeln!(
                f,
                "{t:.1}\t{}\t{}\t{}\t{}\t{}\t{:.0}\t{:.0}\t{:.0}\t{private:.0}\t{:.0}\t{read_mb_s:.1}\t{read_ops_s:.0}\t{}",
                clock(),
                utc_now(),
                opt(fps, 1),
                opt(ms, 1),
                opt(draws, 0),
                m.free_ram,
                m.free_commit,
                m.commit_limit,
                pmc.WorkingSetSize as f64 / MIB,
                seen.marks.len()
            );
        }
        seen.samples.push(Sample {
            t,
            ms,
            free_ram: m.free_ram,
            free_commit: m.free_commit,
            private,
            read_mb_s,
        });
    }
    // SAFETY: the handle opened above.
    unsafe { CloseHandle(process) };
    seen
}

/// Seconds in a month from a log line's UTC time (`2026-11-27T09:12:33.123456Z ...`);
/// enough to set lines and F8 presses of one session side by side.
fn log_seconds(stamp: &str) -> Option<f64> {
    let (date, time) = stamp.split_once('T')?;
    let day: f64 = date.rsplit('-').next()?.parse().ok()?;
    let time = time.trim_end_matches('Z');
    let mut parts = time.split(':');
    let h: f64 = parts.next()?.parse().ok()?;
    let m: f64 = parts.next()?.parse().ok()?;
    let s: f64 = parts.next()?.parse().ok()?;
    Some(day * 86400.0 + h * 3600.0 + m * 60.0 + s)
}

/// The text without terminal colour codes.
fn plain(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// What the engine's log says, for the summary.
#[derive(Default)]
struct LogFacts {
    /// Every frame's length (ms) from the frame-time lines, with the line's time.
    frames: Vec<(f64, f64)>,
    long_lines: usize,
    pressure_on: Vec<f64>,
    pressure_cleared: usize,
    no_room: usize,
    hangs: usize,
    warnings: usize,
    errors: usize,
    first_line: Option<f64>,
}

fn read_log(path: &Path) -> LogFacts {
    let mut facts = LogFacts::default();
    let Ok(text) = std::fs::read(path) else {
        return facts;
    };
    for raw in String::from_utf8_lossy(&text).lines() {
        let line = plain(raw);
        let at = line.split_whitespace().next().and_then(log_seconds);
        if facts.first_line.is_none() {
            facts.first_line = at;
        }
        if line.contains(" WARN ") {
            facts.warnings += 1;
        } else if line.contains(" ERROR ") {
            facts.errors += 1;
        }
        if let Some((_, each)) = line
            .split_once("frame times: ")
            .and_then(|(_, r)| r.split_once("each: "))
        {
            let at = at.unwrap_or(0.0);
            facts.frames.extend(
                each.split_whitespace()
                    .filter_map(|v| v.parse::<f64>().ok())
                    .map(|ms| (at, ms)),
            );
        } else if line.contains("long frame ") {
            facts.long_lines += 1;
        } else if line.contains("host memory pressure cleared") {
            facts.pressure_cleared += 1;
        } else if line.contains("host memory pressure:") {
            facts.pressure_on.push(at.unwrap_or(0.0));
        } else if line.contains("has no room") {
            facts.no_room += 1;
        } else if line.contains("hang watch: no frame") {
            facts.hangs += 1;
        }
    }
    facts
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i.min(sorted.len() - 1)]
}

/// `summary.txt`: the main numbers of the session, and around each F8.
pub fn summarize(dir: &Path, seen: &Seen) -> String {
    let log = read_log(&dir.join("game.log"));
    let pt = settings::portuguese();
    let tr = |a: &'static str, b: &'static str| if pt { a } else { b };
    let mut s = String::new();
    let samples = &seen.samples;
    let duration = samples.last().map_or(0.0, |x| x.t);
    let _ = writeln!(
        s,
        "{} {:.1} min · {} F8",
        tr("Sessão de diagnóstico:", "Diagnostic session:"),
        duration / 60.0,
        seen.marks.len()
    );

    // Frames, from the engine's own frame-time lines (every frame).
    let mut frames: Vec<f64> = log.frames.iter().map(|f| f.1).collect();
    frames.sort_by(f64::total_cmp);
    if frames.is_empty() {
        let _ = writeln!(
            s,
            "{}",
            tr(
                "Frames: o log não tem linhas de tempo de frame.",
                "Frames: the log has no frame-time lines."
            )
        );
    } else {
        let over = |ms: f64| frames.iter().filter(|&&f| f > ms).count();
        let _ = writeln!(
            s,
            "{} {}: {} {:.0} ms · P90 {:.0} · P99 {:.0} · {} {:.0} ms",
            tr("Frames", "Frames"),
            frames.len(),
            tr("mediana", "median"),
            percentile(&frames, 0.5),
            percentile(&frames, 0.9),
            percentile(&frames, 0.99),
            tr("pior", "worst"),
            frames.last().copied().unwrap_or(0.0)
        );
        let _ = writeln!(
            s,
            "  {} 100 ms: {} · 250 ms: {} · 500 ms: {} · 1 s: {}",
            tr("acima de", "over"),
            over(100.0),
            over(250.0),
            over(500.0),
            over(1000.0)
        );
    }

    // Memory and disk, from the samples.
    if !samples.is_empty() {
        let min = |f: fn(&Sample) -> f64| samples.iter().map(f).fold(f64::INFINITY, f64::min);
        let max = |f: fn(&Sample) -> f64| samples.iter().map(f).fold(0.0, f64::max);
        let braked = samples
            .iter()
            .filter(|x| x.free_commit <= PRESSURE_ON_MIB)
            .count();
        let _ = writeln!(
            s,
            "{} {:.1} GB · {} {:.1} GB {} {:.1} GB",
            tr("RAM livre mínima:", "Least free RAM:"),
            min(|x| x.free_ram) / 1024.0,
            tr("commit livre mínimo:", "least free commit:"),
            min(|x| x.free_commit) / 1024.0,
            tr("de um limite de", "of a limit of"),
            seen.commit_limit / 1024.0
        );
        let _ = writeln!(
            s,
            "{} {braked} s ({:.0}%)",
            tr(
                "Segundos com commit livre <= 4 GB (freio de texturas do motor):",
                "Seconds with free commit <= 4 GB (the engine's texture brake):"
            ),
            braked as f64 * 100.0 / samples.len() as f64
        );
        let _ = writeln!(
            s,
            "{} {:.1} GB · {} {:.0} MB/s",
            tr("Memória do jogo (máx.):", "Game memory (most):"),
            max(|x| x.private) / 1024.0,
            tr("leitura de disco (pico):", "disk reads (peak):"),
            max(|x| x.read_mb_s)
        );
    }
    let _ = writeln!(
        s,
        "{} {} ({} {}) · {} {} · {} {} · {} {} · WARN {} · ERROR {}",
        tr("Freio ligado no log:", "Brake on in the log:"),
        log.pressure_on.len(),
        tr("liberado", "cleared"),
        log.pressure_cleared,
        tr("placa sem espaço:", "card out of room:"),
        log.no_room,
        tr("frames longos listados:", "long frames listed:"),
        log.long_lines,
        tr(
            "travamentos (5 s sem frame):",
            "stalls (5 s with no frame):"
        ),
        log.hangs,
        log.warnings,
        log.errors
    );

    // Around each F8: the samples of the 5 s before, and the frame-time lines near it.
    if !seen.marks.is_empty() {
        let _ = writeln!(
            s,
            "\n{}",
            tr("Em cada F8 (5 s antes):", "At each F8 (the 5 s before):")
        );
    }
    for (i, (t, utc)) in seen.marks.iter().enumerate() {
        let near: Vec<&Sample> = samples
            .iter()
            .filter(|x| x.t >= t - 5.0 && x.t <= t + 0.5)
            .collect();
        let worst_ms = near.iter().filter_map(|x| x.ms).fold(0.0, f64::max);
        let commit = near
            .iter()
            .map(|x| x.free_commit)
            .fold(f64::INFINITY, f64::min);
        let disk = near.iter().map(|x| x.read_mb_s).fold(0.0, f64::max);
        let at = log_seconds(utc);
        let longest = at.map_or(0.0, |at| {
            log.frames
                .iter()
                .filter(|f| f.0 >= at - 6.0 && f.0 <= at + 6.0)
                .map(|f| f.1)
                .fold(0.0, f64::max)
        });
        let braked = at.is_some_and(|at| log.pressure_on.iter().any(|&p| p <= at));
        let _ = writeln!(
            s,
            "  F8 #{} {} {:.0} s: {} {:.0} ms · {} {:.0} ms · {} {:.1} GB{} · {} {:.0} MB/s",
            i + 1,
            tr("aos", "at"),
            t,
            tr("ms/s pior", "worst ms/s"),
            worst_ms,
            tr("frame mais longo", "longest frame"),
            longest,
            tr("commit livre", "free commit"),
            if commit.is_finite() {
                commit / 1024.0
            } else {
                0.0
            },
            if braked {
                tr(" (freio ligado)", " (brake on)")
            } else {
                ""
            },
            tr("disco", "disk"),
            disk
        );
    }
    let _ = std::fs::write(dir.join("summary.txt"), &s);
    s
}

#[cfg(test)]
mod tests {
    #[test]
    fn caption_parses() {
        assert_eq!(
            super::caption_numbers("Scarlett | Game | 8.9 FPS | 112.4 ms | 4225 draws"),
            (Some(8.9), Some(112.4), Some(4225.0))
        );
        assert_eq!(
            super::caption_numbers("Scarlett | 0.0 FPS | – ms | 0 draws").1,
            None
        );
    }

    #[test]
    fn log_time_and_colours() {
        let a = super::log_seconds("2026-11-27T09:00:01.500000Z").unwrap();
        let b = super::log_seconds("2026-11-27T09:00:00Z").unwrap();
        assert!((a - b - 1.5).abs() < 1e-6);
        assert_eq!(
            super::plain("\u{1b}[2m2026\u{1b}[0m  \u{1b}[33m WARN\u{1b}[0m x"),
            "2026   WARN x"
        );
    }
}
