//! State roots to open the app against, so every view can be looked at
//! without touching this machine.
//!
//! The app reads almost everything it draws out of its state root: the
//! status files under `status/`, the log beside them, and the socket it
//! probes to decide whether a supervisor is up. Point it at a directory
//! built here and the panes fill with a fleet that does not exist.
//!
//! Two things are not in the state root, and each gets an environment
//! variable the library honours: whether `autobahn` is installed
//! (`AUTOBAHN_BIN`) and what the login service is doing
//! (`AUTOBAHN_SERVICE_STATE`). Each fixture writes an `env` file holding
//! its own answers, which `scripts/views.sh` reads before launching.
//!
//!     cargo run --example fixtures -- <directory>
//!     cargo run --example fixtures -- --list

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use autobahn::config::Config;
use autobahn::supervisor::{ConflictDetail, ConflictSide, SessionStatus, Unsynchronizable};

/// One fixture: a configuration, a status per session, and the two
/// answers that do not live in a state root.
struct Fixture {
    name: &'static str,
    /// What it is for, printed by `--list` and written into the fixture.
    about: &'static str,
    config: &'static str,
    /// Whether the machine has an `autobahn` to talk to. False is the
    /// welcome pane, which is otherwise unreachable here.
    installed: bool,
    service: &'static str,
    log: &'static str,
    /// Status per `group@host`, in the shape `plans()` will name them.
    /// A group with no entry here has never run, which is its own state.
    statuses: fn(now: u64) -> BTreeMap<&'static str, SessionStatus>,
}

fn main() -> Result<()> {
    let mut out: Option<String> = None;
    let mut keys = false;
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--list" => {
                for fixture in FIXTURES {
                    println!("{:<10} {}", fixture.name, fixture.about);
                }
                return Ok(());
            }
            // The names a status has to be filed under. An identifier is
            // derived, so adding a session below means asking what it is
            // called rather than working it out. It rides on a real
            // generation because a `file:` entry is only resolvable once
            // the fixture's own ignore directory is on disk.
            "--keys" => keys = true,
            other => out = Some(other.to_owned()),
        }
    }
    // Absolute, because the `env` file names paths and whoever reads it
    // is not standing where this ran.
    let out = PathBuf::from(out.unwrap_or_else(|| "target/views".to_owned()));
    let out = match out.is_absolute() {
        true => out,
        false => std::env::current_dir()
            .context("unable to say where this is running")?
            .join(out),
    };
    // Built fresh every time: a fixture half from this build and half
    // from the last one is a bug that looks like a drawing bug.
    if out.exists() {
        std::fs::remove_dir_all(&out).context("unable to clear the fixture directory")?;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0);
    for fixture in FIXTURES {
        let at = out.join(fixture.name);
        let filed = write(fixture, &at, now)?;
        match keys {
            true => {
                for key in filed {
                    println!("{:<10} {key}", fixture.name);
                }
            }
            false => println!("{}", at.display()),
        }
    }
    Ok(())
}

/// Builds one fixture on disk.
fn write(fixture: &Fixture, at: &Path, now: u64) -> Result<Vec<String>> {
    let state = at.join("state");
    std::fs::create_dir_all(state.join("status")).context("unable to make the state root")?;
    std::fs::create_dir_all(state.join("ignores")).context("unable to make the ignores")?;
    let config = at.join("autobahn.toml");
    std::fs::write(&config, fixture.config).context("unable to write the configuration")?;
    // A `file:` entry is read from the state root's `ignores`, not from
    // beside the configuration, and the configuration is refused without
    // it. Writing it here means the fixture exercises that path instead
    // of stepping around it — and, because the state root is this
    // fixture's, it never reads the real one.
    std::fs::write(
        state.join("ignores").join("Essential.gitignore"),
        autobahn::config::ESSENTIAL_IGNORES,
    )
    .context("unable to write the essential ignores")?;
    // `AUTOBAHN_HOME` is the state root everything else derives from —
    // the ignores above, and the locks and caches nothing here touches.
    // `--state-root` tells the app; this tells the library underneath it.
    std::env::set_var("AUTOBAHN_HOME", &state);
    std::fs::write(state.join("service.log"), fixture.log).context("unable to write the log")?;

    // The names the app will look for. Asking the library rather than
    // guessing: an identifier is derived, and a fixture whose filenames
    // are a guess is a fixture that silently shows nothing.
    let loaded = Config::load(&config)
        .with_context(|| format!("the {} fixture's configuration does not load", fixture.name))?;
    let plans = loaded.plans().with_context(|| {
        format!(
            "the {} fixture's configuration makes no plans",
            fixture.name
        )
    })?;
    let mut wanted = (fixture.statuses)(now);
    let mut filed = Vec::new();
    for plan in &plans {
        let key = format!("{}@{}", plan.group, plan.host);
        filed.push(key.clone());
        let Some(mut status) = wanted.remove(key.as_str()) else {
            continue;
        };
        status.group = plan.group.clone();
        status.host = plan.host.clone();
        status.beta = plan.beta_spec();
        status.mode = plan.mode_name().to_owned();
        status.updated_at = now.saturating_sub(3);
        let path = state
            .join("status")
            .join(format!("{}.json", plan.identifier()));
        let text = serde_json::to_vec_pretty(&status).context("unable to encode a status")?;
        std::fs::write(&path, text)
            .with_context(|| format!("unable to write {}", path.display()))?;
    }
    anyhow::ensure!(
        wanted.is_empty(),
        "the {} fixture has statuses for sessions its configuration does not describe: {:?}",
        fixture.name,
        wanted.keys().collect::<Vec<_>>(),
    );

    // A shim rather than the real command: a fixture that could run
    // `autobahn clean` against a made-up state root is a fixture that
    // can do damage. This one says what it was asked and stops.
    let shim = at.join("bin").join("autobahn");
    if fixture.installed {
        std::fs::create_dir_all(shim.parent().expect("bin has a parent"))
            .context("unable to make the shim directory")?;
        std::fs::write(&shim, SHIM).context("unable to write the shim")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755))
                .context("unable to make the shim runnable")?;
        }
    }

    // What `views.sh` exports. An uninstalled fixture points at a path
    // that is not there, which is the answer "no command" rather than
    // "carry on looking in the usual places".
    let told = match fixture.installed {
        true => shim,
        false => at.join("bin").join("autobahn"),
    };
    std::fs::write(
        at.join("env"),
        format!(
            "AUTOBAHN_HOME={}\nAUTOBAHN_BIN={}\nAUTOBAHN_SERVICE_STATE={}\n",
            state.display(),
            told.display(),
            fixture.service,
        ),
    )
    .context("unable to write the fixture's environment")?;
    Ok(filed)
}

/// Stands in for the command, for the buttons that shell out.
const SHIM: &str = r#"#!/bin/sh
# A fixture's autobahn. It runs nothing and changes nothing; it exists so
# the app believes the command is installed, and answers plausibly when a
# button shells out to it.
case "$1" in
  --version) echo "autobahn 0.4.0 (fixture)" ;;
  status)    echo "3 sessions, 1 needs you" ;;
  clean)     echo "nothing to clean" ;;
  resolve)   echo "resolved (fixture: nothing moved)" ;;
  *)         echo "fixture autobahn: $*" ;;
esac
"#;

const FIXTURES: &[Fixture] = &[
    Fixture {
        name: "fresh",
        about: "nothing installed — the welcome splash",
        installed: false,
        service: "not-installed",
        config: CONFIG_EMPTY,
        log: "",
        statuses: |_| BTreeMap::new(),
    },
    Fixture {
        name: "quiet",
        about: "installed, configured, never run",
        installed: true,
        service: "stopped",
        config: CONFIG_THREE,
        log: LOG_QUIET,
        statuses: |_| BTreeMap::new(),
    },
    Fixture {
        name: "calm",
        about: "everything synchronized",
        installed: true,
        service: "running",
        config: CONFIG_THREE,
        log: LOG_CALM,
        statuses: calm,
    },
    Fixture {
        name: "trouble",
        about: "a conflict, a blocked path, a host away",
        installed: true,
        service: "running",
        config: CONFIG_THREE,
        log: LOG_TROUBLE,
        statuses: trouble,
    },
];

fn calm(_now: u64) -> BTreeMap<&'static str, SessionStatus> {
    let mut all = BTreeMap::new();
    for (key, cycles, entries, files, bytes) in [
        (
            "work@build.audi.de",
            18_204u64,
            61_880u64,
            402_118u64,
            94_221_880_440u64,
        ),
        (
            "work@laptop.bmw.de",
            18_201,
            61_880,
            398_004,
            93_880_112_006,
        ),
        (
            "backup@/Volumes/Backup/Workspace",
            304,
            61_880,
            221_440,
            90_114_002_118,
        ),
        ("notes@laptop.bmw.de", 9_118, 2_044, 14_902, 408_221_118),
    ] {
        all.insert(
            key,
            SessionStatus {
                state: "synchronized".into(),
                cycles,
                alpha_entries: entries,
                beta_entries: entries,
                moved_files: files,
                moved_bytes: bytes,
                // The p2p session is the only one with a role and a
                // term, and the pane has to have somewhere to put them.
                role: match key.starts_with("notes@") {
                    true => "leader".into(),
                    false => String::new(),
                },
                term: match key.starts_with("notes@") {
                    true => 7,
                    false => 0,
                },
                ..Default::default()
            },
        );
    }
    all
}

fn trouble(now: u64) -> BTreeMap<&'static str, SessionStatus> {
    let mut all = BTreeMap::new();
    all.insert(
        "work@build.audi.de",
        SessionStatus {
            state: "conflicts".into(),
            cycles: 18_207,
            conflicts: vec!["api/src/config.rs".into(), "web/package.json".into()],
            conflict_details: vec![
                ConflictDetail {
                    path: "api/src/config.rs".into(),
                    alpha: ConflictSide {
                        present: true,
                        kind: "file".into(),
                        size: 11_204,
                        mtime_seconds: now.saturating_sub(640) as i64,
                        unsynchronizable: None,
                    },
                    beta: ConflictSide {
                        present: true,
                        kind: "file".into(),
                        size: 10_880,
                        mtime_seconds: now.saturating_sub(98) as i64,
                        unsynchronizable: None,
                    },
                },
                ConflictDetail {
                    path: "web/package.json".into(),
                    alpha: ConflictSide {
                        present: true,
                        kind: "file".into(),
                        size: 2_914,
                        mtime_seconds: now.saturating_sub(120) as i64,
                        unsynchronizable: None,
                    },
                    beta: ConflictSide::default(),
                },
            ],
            alpha_entries: 61_902,
            beta_entries: 61_898,
            moved_files: 402_440,
            moved_bytes: 94_228_118_004,
            ..Default::default()
        },
    );
    all.insert(
        "work@laptop.bmw.de",
        SessionStatus {
            state: "blocked".into(),
            cycles: 18_206,
            blocked: vec![
                "beta api/.venv/bin/python: broken symlink".into(),
                "alpha web/.next/cache: permission denied".into(),
            ],
            conflict_details: vec![ConflictDetail {
                path: "api/data".into(),
                alpha: ConflictSide {
                    present: true,
                    kind: "directory".into(),
                    mtime_seconds: now.saturating_sub(40) as i64,
                    ..Default::default()
                },
                beta: ConflictSide {
                    present: true,
                    kind: "directory".into(),
                    mtime_seconds: now.saturating_sub(44) as i64,
                    unsynchronizable: Some(Unsynchronizable {
                        entries: 3,
                        example: "api/data/dev.sqlite-wal".into(),
                        reason: "a database being written".into(),
                    }),
                    ..Default::default()
                },
            }],
            alpha_entries: 61_902,
            beta_entries: 61_899,
            moved_files: 398_220,
            moved_bytes: 93_886_004_552,
            ..Default::default()
        },
    );
    all.insert(
        "backup@/Volumes/Backup/Workspace",
        SessionStatus {
            // The disk that was unplugged, which is the ordinary way a
            // one-way group stops: the beta is simply not there.
            state: "halted".into(),
            cycles: 304,
            error: Some("beta /Volumes/Backup/Workspace is not a directory any more".into()),
            alert_after_seconds: Some(900),
            alpha_entries: 61_880,
            moved_files: 221_440,
            moved_bytes: 90_114_002_118,
            ..Default::default()
        },
    );
    all.insert(
        "notes@laptop.bmw.de",
        SessionStatus {
            // The other machine took the lead while this one slept, so
            // this side is a follower and the term has moved on.
            state: "synchronized".into(),
            cycles: 9_121,
            role: "follower".into(),
            term: 9,
            alpha_entries: 2_046,
            beta_entries: 2_046,
            moved_files: 14_910,
            moved_bytes: 408_440_002,
            ..Default::default()
        },
    );
    all
}

/// A fresh install's configuration: what `autobahn init` writes, with
/// every group still commented out.
const CONFIG_EMPTY: &str = r#"# A machine that has only just run `autobahn init`.
log_level = "normal"

[defaults]
mode = "two-way-conflict"
ignores = ["file:Essential.gitignore"]
interval = 5
"#;

/// A configuration somebody has been living in.
///
/// Not a demonstration of five features in a row: a tidy example teaches
/// nothing, because nobody's file is tidy. This one has a group that is
/// switched off, ignores that name the actual offenders rather than
/// standing in for them, intervals that differ because the folders do,
/// and an alert hook — the accretions a file picks up over a year.
///
/// Every mode is here, and each because its folder wants it. The
/// destinations are car makers because this is a motorway.
const CONFIG_THREE: &str = r#"# Run when a session needs a person.
on_alert = "~/.autobahn/on-alert.sh"
log_level = "normal"

[defaults]
mode = "two-way-conflict"
ignores = ["file:Essential.gitignore"]
interval = 5

# Work, on the box with the cores. Both ends edit it — an agent over
# there, me over here — so a clash is reported and nothing is touched.
[groups.work]
alpha = "~/Workspace"
betas = [
  "dev@build.audi.de:/home/dev/workspace",
  "laptop.bmw.de",
]
ignores = ["target", "node_modules", ".venv", ".next", "*.sqlite"]

# The same folder onto the disk that keeps a copy. One way, and the disk
# is made identical: a backup that can push a deletion back is not one.
# Slower, because nothing is waiting on it.
[groups.backup]
mode = "one-way-alpha"
alpha = "~/Workspace"
betas = ["/Volumes/Backup/Workspace"]
interval = 300

# Notes, where either machine may be the one that is awake. The lease a
# p2p leader holds is renewed once a cycle and lasts 30s, so the interval
# cannot go past half of that.
[groups.notes]
mode = "p2p-conflict-dangerously-experimental"
alpha = "~/Documents/Notes"
betas = ["laptop.bmw.de"]
interval = 10

# The photo library onto the NAS. Off since the NAS started refusing
# connections; turn it back on when that is sorted.
[groups.photos]
alpha = "~/Pictures/Lightroom"
betas = ["nas.porsche.de:/volume1/photos"]
disabled = true
"#;

const LOG_QUIET: &str = "\
2026-10-02 09:14:02 info  autobahn 0.4.0 starting
2026-10-02 09:14:02 info  read 4 groups (1 disabled), 4 sessions
2026-10-02 09:14:02 info  no session has run yet
";

const LOG_CALM: &str = "\
2026-10-02 09:14:02 info  autobahn 0.4.0 starting
2026-10-02 09:14:02 info  read 4 groups (1 disabled), 4 sessions
2026-10-02 09:14:03 info  work@build.audi.de scanning alpha
2026-10-02 09:14:03 info  work@build.audi.de 61880 entries, 0 changed
2026-10-02 09:14:04 info  work@laptop.bmw.de 61880 entries, 0 changed
2026-10-02 09:14:04 info  notes@laptop.bmw.de leader for term 7
2026-10-02 09:14:04 info  notes@laptop.bmw.de 2044 entries, 0 changed
2026-10-02 09:18:02 info  backup@/Volumes/Backup/Workspace 61880 entries, 0 changed
2026-10-02 09:18:02 info  all sessions synchronized
2026-10-02 09:23:02 info  heartbeat: 4 synchronized
2026-10-02 09:28:02 info  heartbeat: 4 synchronized
";

const LOG_TROUBLE: &str = "\
2026-10-02 09:14:02 info  autobahn 0.4.0 starting
2026-10-02 09:14:02 info  read 4 groups (1 disabled), 4 sessions
2026-10-02 09:14:03 info  work@build.audi.de scanning alpha
2026-10-02 09:14:04 warn  work@build.audi.de conflict at api/src/config.rs
2026-10-02 09:14:04 warn  work@build.audi.de conflict at web/package.json
2026-10-02 09:14:05 error work@laptop.bmw.de beta api/.venv/bin/python: broken symlink
2026-10-02 09:14:05 error work@laptop.bmw.de alpha web/.next/cache: permission denied
2026-10-02 09:14:05 warn  work@laptop.bmw.de 2 paths could not be carried
2026-10-02 09:14:06 info  notes@laptop.bmw.de lease lost, follower for term 9
2026-10-02 09:18:02 error backup@/Volumes/Backup/Workspace beta is not a directory any more
2026-10-02 09:18:02 error backup@/Volumes/Backup/Workspace halted, will not retry
2026-10-02 09:23:02 warn  heartbeat: 1 conflicts, 1 blocked, 1 halted
";
