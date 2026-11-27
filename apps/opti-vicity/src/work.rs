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
    Exited(Option<i32>),
    Failed(String),
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
    let log_path = settings::logs_dir().join("game.log");
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
    let code = child.wait().ok().and_then(|s| s.code());
    post(Event::Exited(code));
}

/// Where a game log is, for the window's Logs button.
pub fn logs_folder() -> PathBuf {
    settings::logs_dir()
}

#[cfg(test)]
mod tests {
    #[test]
    fn progress_lines_parse() {
        assert_eq!(super::progress("progress 100 of 25889"), Some((100, 25889)));
        assert_eq!(super::progress("1000 of 2047, 63 s"), None);
        assert_eq!(super::progress("progress x of 3"), None);
    }
}
