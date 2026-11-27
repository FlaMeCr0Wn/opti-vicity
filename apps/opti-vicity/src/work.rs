//! The launcher's work off the window's thread: shaders and pipelines made
//! ahead (with progress from the engine's `progress <n> of <total>` lines,
//! `VICTORY_PROGRESS=1`), then the game itself. Everything it learns goes to
//! the window as [`Event`]s.

use std::io::{BufRead, BufReader, Write};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::settings::{self, Config, t};

/// No console window for the engine (its children share the hidden one).
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub enum Event {
    /// The current step, shown above the bar.
    Phase(String),
    /// Progress in the current step; `total` 0 is "no total known".
    Progress {
        done: u64,
        total: u64,
    },
    /// Preparing ended (`run` false), or the game is starting.
    Prepared {
        recorded_none: bool,
    },
    Started,
    /// The game closed (its exit code), and the diagnostic session folder if one was kept.
    Exited(Option<i32>, Option<PathBuf>),
    Failed(String),
    /// Windows refused to load a file the engine makes from the game's exe:
    /// Smart App Control or another application-control policy (the log
    /// says so; shown after `Exited`).
    Blocked,
}

/// What one press of Play (or Prepare) asks for.
pub struct Plan {
    pub engine: PathBuf,
    pub game: PathBuf,
    pub cfg: Config,
    pub cache: PathBuf,
    /// Make shaders and pipelines ahead even when this build already has them.
    pub force_prepare: bool,
    /// Start the game afterwards.
    pub run: bool,
}

/// Runs `plan` on a thread of its own; `post` delivers each event to the window.
pub fn start(plan: Plan, post: impl Fn(Event) + Send + 'static) {
    std::thread::spawn(move || {
        let _ = std::fs::create_dir_all(settings::logs_dir());
        let mut recorded_none = false;
        if plan.force_prepare || !settings::prepared(&plan.engine, &plan.cache) {
            match prepare(&plan, &post) {
                Ok(none) => recorded_none = none,
                Err(why) => {
                    post(Event::Failed(why));
                    return;
                }
            }
        }
        post(Event::Prepared { recorded_none });
        if plan.run {
            run(&plan, &post);
        }
    });
}

/// The engine with the game's environment: inherited `VICTORY_*` variables
/// are dropped (except where saves go), the launcher's set put in.
fn engine(plan: &Plan) -> Command {
    let mut cmd = Command::new(&plan.engine);
    for (key, _) in std::env::vars_os() {
        let name = key.to_string_lossy();
        if name.starts_with("VICTORY_") && name != "VICTORY_DATA_DIR" {
            cmd.env_remove(&key);
        }
    }
    cmd.envs(settings::environment(&plan.cfg, &plan.game, &plan.cache));
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Shaders translated, then the pipelines the game was seen using compiled.
/// Ok(true): there were no pipelines written down yet (a first session).
fn prepare(plan: &Plan, post: &impl Fn(Event)) -> Result<bool, String> {
    let _ = std::fs::create_dir_all(&plan.cache);
    let log_path = settings::logs_dir().join("prepare.log");
    let log =
        std::fs::File::create(&log_path).map_err(|e| format!("{}: {e}", log_path.display()))?;
    let mut ok = true;
    let rpf = plan.game.join("shaders.rpf");
    if rpf.is_file() {
        post(Event::Phase(
            t(
                "Traduzindo os shaders do jogo",
                "Translating the game's shaders",
            )
            .into(),
        ));
        ok &= step(plan, &[rpf.as_os_str()], "shaders", &log, post)?;
    }
    let known = plan.cache.join(r"pipelines\known-pipelines.bin");
    let recorded_none = !known.is_file();
    if !recorded_none {
        post(Event::Phase(
            t("Compilando os pipelines", "Compiling the pipelines").into(),
        ));
        ok &= step(plan, &["--used".as_ref()], "warm", &log, post)?;
    }
    // Written only when everything is there: after a first session (nothing
    // written down yet) the next start makes its pipelines ahead.
    if ok && !recorded_none {
        let _ = std::fs::write(
            settings::stamp_file(&plan.cache),
            settings::engine_stamp(&plan.engine),
        );
    }
    Ok(recorded_none)
}

/// One engine command with its progress lines read; the rest goes to the log.
fn step(
    plan: &Plan,
    args: &[&std::ffi::OsStr],
    command: &str,
    log: &std::fs::File,
    post: &impl Fn(Event),
) -> Result<bool, String> {
    let mut cmd = engine(plan);
    cmd.arg(command).args(args).env("VICTORY_PROGRESS", "1");
    cmd.stdout(Stdio::piped());
    cmd.stderr(log.try_clone().map(Stdio::from).unwrap_or(Stdio::null()));
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("{}: {e}", plan.engine.display()))?;
    post(Event::Progress { done: 0, total: 0 });
    if let Some(out) = child.stdout.take() {
        let mut log = log.try_clone().ok();
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if let Some((done, total)) = progress(&line) {
                post(Event::Progress { done, total });
            } else if let Some(log) = log.as_mut() {
                let _ = writeln!(log, "{line}");
            }
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    Ok(status.success())
}

/// "progress 123 of 456".
fn progress(line: &str) -> Option<(u64, u64)> {
    let rest = line.strip_prefix("progress ")?;
    let (done, total) = rest.split_once(" of ")?;
    Some((done.trim().parse().ok()?, total.trim().parse().ok()?))
}

fn run(plan: &Plan, post: &impl Fn(Event)) {
    // Diagnostic mode: everything of this session in a folder of its own.
    let session = if plan.cfg.diagnostic {
        match crate::diag::new_session() {
            Ok(dir) => {
                crate::diag::write_settings(
                    &dir,
                    &settings::environment(&plan.cfg, &plan.game, &plan.cache),
                );
                Some(dir)
            }
            Err(e) => {
                post(Event::Failed(format!("diag: {e}")));
                return;
            }
        }
    } else {
        None
    };
    let log_path = session.as_ref().map_or_else(
        || settings::logs_dir().join("game.log"),
        |d| d.join("game.log"),
    );
    let mut cmd = engine(plan);
    cmd.arg("run")
        .arg(plan.game.join(settings::TITLE_EXE))
        .arg("--execute");
    cmd.args([
        "--console",
        if plan.cfg.series_x {
            "series-x"
        } else {
            "series-s"
        },
    ]);
    match std::fs::File::create(&log_path) {
        Ok(log) => {
            cmd.stdout(log.try_clone().map(Stdio::from).unwrap_or(Stdio::null()));
            cmd.stderr(Stdio::from(log));
        }
        Err(_) => {
            cmd.stdout(Stdio::null()).stderr(Stdio::null());
        }
    }
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            post(Event::Failed(format!("{}: {e}", plan.engine.display())));
            return;
        }
    };
    post(Event::Started);
    let sampler = session.clone().map(|dir| {
        let pid = child.id();
        std::thread::spawn(move || crate::diag::sample(&dir, pid))
    });
    let code = child.wait().ok().and_then(|s| s.code());
    if let (Some(dir), Some(sampler)) = (&session, sampler)
        && let Ok(seen) = sampler.join()
    {
        crate::diag::summarize(dir, &seen);
    }
    post(Event::Exited(code, session));
    if code != Some(0) && blocked_by_app_control(&log_tail(&log_path)) {
        post(Event::Blocked);
    }
}

/// The last 64 KiB of the game log (the error is the run's last line).
fn log_tail(path: &std::path::Path) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let len = file.metadata().map_or(0, |m| m.len());
    let _ = file.seek(SeekFrom::Start(len.saturating_sub(64 * 1024)));
    let mut bytes = Vec::new();
    let _ = file.read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Whether the log says Windows' application control refused a file:
/// ERROR_SYSTEM_INTEGRITY_POLICY_VIOLATION (4551), or one of Smart App
/// Control's reputation refusals (4556 to 4559). The loader turns the game's
/// exe into an unsigned DLL in the cache and loads it; with Smart App
/// Control on, Windows refuses it ("os error 4551").
fn blocked_by_app_control(log: &str) -> bool {
    [4551, 4556, 4557, 4558, 4559]
        .iter()
        .any(|code| log.contains(&format!("(os error {code})")))
}

/// Where a game log is, for the window's Logs button.
pub fn logs_folder() -> PathBuf {
    settings::logs_dir()
}

#[cfg(test)]
mod tests {
    #[test]
    fn application_control_refusals_are_seen() {
        let line = "Error: LoadLibraryExW(C:\\x\\victory-image-b99f876b74d4a77f.dll) failed: Uma política de \
                    Controle de Aplicativo bloqueou este arquivo. (os error 4551)";
        assert!(super::blocked_by_app_control(line));
        assert!(super::blocked_by_app_control("... (os error 4558)\n"));
        assert!(!super::blocked_by_app_control(
            "failed: Access is denied. (os error 5)"
        ));
        assert!(!super::blocked_by_app_control("(os error 45510)"));
    }

    #[test]
    fn progress_lines_parse() {
        assert_eq!(super::progress("progress 100 of 25889"), Some((100, 25889)));
        assert_eq!(super::progress("1000 of 2047, 63 s"), None);
        assert_eq!(super::progress("progress x of 3"), None);
    }
}
