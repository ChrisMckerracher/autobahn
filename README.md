# Autobahn <picture><source media="(prefers-color-scheme: dark)" srcset="assets/sign-readme-white.svg"><img src="assets/sign-readme.svg" alt="" height="23"></picture>

_Subsecond sync with German precision._

Autobahn keeps your files in sync as fast as you (or an agent) edit them across a fleet.

<img src="assets/screenshots/groups.png" alt="Dash showing three sync groups across four sessions, all synchronized" width="900">

Install the latest version of the [app](docs/app.md) from its [releases page](https://github.com/fny/autobahn/releases) or as a standalone CLI service:

```sh
curl -fsSL https://github.com/fny/autobahn/releases/latest/download/install.sh | sh
```

```
For AI: Follow the instructions here http://github.com/fny/autobahn/blob/main/INSTALL.md to drive an interactive setup of Autobahn.
```

> ✨ Want to support Autobahn? Want to use Autobahn free of AGPLv3?
> Simply donate to the [Justice-in-Education Initiative](docs/donations.md).

## The Problem

- Browsing files over SSH or NFS is clunky.
- Agents that run `--dangerously` should do it in a VM elsewhere, but you can't use your local tools.
- Some sync tools require gigs of RAM for big trees, or a cloud account, or both.

Solution: keep everything in sync so editing local files is the same as editing remote ones.

## Why Autobahn

- **Fast as hell.** Delivers sub-30ms propagation times for small-file updates across trees containing hundreds of thousands of files.
- **Lightweight.** Employs immutable shared-tree structures in memory, requiring significantly less RAM and idle CPU than conventional sync daemons.
- **Safe.** Choose a sync policy per group that matches your risk profile backed by tests and bounded formal models. See [Safety](docs/safety.md) for the guarantees and their limits.
- **Reviewed to death.** GLM 5.3, KIMI 3, Astra, and Fable were used to perform correctness and secuirty reviews.
- **Privacy first.** No cloud service, no account, no third party.

## Quick Start

After you [install Autobahn](INSTALL.md) you need to set up your configuration. By default, the configuration is written to `~/.autobahn/config.toml`. You can edit it by hand or use [Dash](docs/app.md).

Each group connects one root, the primary, to one or more destinations, the replicas. Sync can be one-way, bidirectional, or P2P (experimental.)

```toml
# ~/.autobahn/config.toml

[defaults]
mode = "two-way-conflict"                   # sync modes explained below
ignores = ["file:Essential.gitignore"]      # written by `autobahn init`

[groups.work]
primary = "~/Workspace"
replicas = [                                # sync targets
  "dev@build.audi.de:/home/dev/workspace",  #  - fully specified
  "laptop.bmw.de",                          #  - inherits the primary path
]
ignores = ["target", "node_modules"]        # appended to the defaults'

[groups.backup]
mode = "one-way-primary"                    # the disk is made identical
primary = "~/Workspace"
replicas = ["/Volumes/Backup/Workspace"]    #  - local paths work too
```

```sh
autobahn watch              # monitor every session as a one off
autobahn install            # or install as a login service
```

## Sync Modes

Autobahn has several sync modes with different resolution strsategies.

Start with `two-way-conflict` for editing on both sides. It propagates changes in either direction and reports competing edits for you to resolve.

| Mode | Behavior |
| --- | --- |
| `two-way-conflict` | Bidirectional sync with conflict reports |
| `two-way-primary` | Favors the primary in conflicts, with deletion safeguards |
| `two-way-primary-strict` | Favors the primary, including its deletions |
| `one-way-conflict` | Reports changes on the replica that prevent a safe copy |
| `one-way-primary` (alias: `mirror`) | Makes the replica match the primary |
| `p2p-*-dangerously-experimental` | Lets a replica lead while the primary is offline |

For mode details, see [Modes](docs/modes.md) and [Conflict Resolution](docs/conflicts.md).

Read [P2P](docs/p2p.md) before P2P use.

## Benchmarks

Autobahn began as an effort to reduce the memory use of [Mutagen](https://mutagen.io/) which offers similar sync features, and then I got carried away.

| Measurement | Autobahn | mutagen | Ratio |
| --- | --: | --: | --: |
| Small-file edit, Chromium, 1 editor, p50 | **23.8 ms** | 6,232.2 ms | 261.9× |
| Small-file edit, 50k subset, 10 editors, p50 | **13.4 ms** | 1,809.8 ms | 135.1× |
| Peak controller memory, Chromium, 1 editor | **479 MiB** | 2,081 MiB | 4.3× |
| Idle controller CPU, Chromium | **0.1% of a core** | 49.9% | rounded values |
| First sync, Chromium | **228.1 s** | 454.8 s | 2.0× |

See [Benchmarks](docs/benchmarks.md) for details.

## Safety

### Empirically

I have been running this on my own fleet every day: 20 sessions across five hosts. One of them is a 13 GB, 215,000-file tree. Autobahn has also undergone a battery of tests and benchmarks including a 24-hour soak test.

### Formally

Autobahn prevents data loss through strict operational invariants:

- **Three-Way Reconciliation:** Tracks a shared ancestor to accurately distinguish deletions, modifications, and concurrent edits.
- **Atomic File Transitions:** All file writes stage content to temporary paths and swap into place using atomic filesystem operations.
- **Fail-Closed Guarantees:** Any ambiguous state, communication failure, or unexpected filesystem mutation results in a pause rather than accidental overwrites.

See [Safety](docs/safety.md) for guarantees and the related invariants in [Correctness](docs/correctness/).

## UI Goodness

In addition to the standard CLI, several user interfaces are available:

- **[Dash](docs/app.md):** Experimental desktop window.
- **[Menu bar item](docs/tray.md):** Status light and a menu without need for the full app.
- **[Terminal UI](docs/shop.md):** Interactive curses-based console monitor (`autobahn shop`).
- **[Alert Hooks](docs/alerts.md):** Event notification script support (`on_alert`).

```toml
on_alert = "~/.autobahn/on-alert.sh"   # written for you by `autobahn init`
```

None of this has undergone nearly the same level of testing as `autobahn` itself, so consider them experimental.

## AI Disclaimer

This project was heavily vibe coded, and with great vibe coding comes great responsibility. So I have: scanned every line in this repo, run extensive soak testing, used Autobahn myself for weeks, had guardrail-free models run security scans, and put it through thousands of benchmark runs. Most of the internal documentation was first drafted by LLMs. Please forgive the lingering Claudeisms.

## Contributing

- I won't accept PRs unless I know you. I prefer my slop over your slop, so instead file an issue for a bug report or (small) feature request.
- Bug reports should come with detailed context from a human or LLM.
- Feature requests should be small with high impact.
- Have a greater request? Go fork yourself. ;D

## Documentation

### Operations & Configuration

- [Installation Guide](INSTALL.md)
- [Configuration Reference](docs/configuration.md)
- [Command-Line Reference](docs/commands.md)
- [Sync Modes](docs/modes.md)
- [Ignore Rules](docs/ignores.md)
- [Conflict Resolution](docs/conflicts.md)
- [Git Repository Best Practices](docs/git.md)
- [Logging & Diagnostics](docs/logging.md)
- [State Directory Layout](docs/state.md)

### Architecture & Design

- [Architecture](docs/architecture.md)
- [Safety Guarantees](docs/safety.md)
- [Limitations](docs/limitations.md)
- [Failover P2P (Experimental)](docs/p2p.md)

### Verification & Performance

- [Benchmark Results](docs/benchmarks.md)
- [Full Benchmark Matrix](docs/benchmark-matrix.md)
- [Invariants](docs/correctness/invariants.md)
- [Accepted risks](docs/correctness/accepted-risks.md)
- [Development Guide](docs/development.md)
- [Release Process](docs/releases.md)
- [Roadmap and proposals](docs/wishlist.md)

## System Requirements & Limitations

- **Supported Platforms:** Linux (`x86_64`, `aarch64`) and macOS (`Apple Silicon`).
- **Filesystems:** Requires local POSIX filesystems. Network filesystems (NFS, SMB, CIFS) receive best-effort support only.
- **Editor Saves on macOS:** On macOS, atomic save operations (write-temporary and rename) lack immediate completion events from the kernel, occasionally requiring an extra polling cycle compared to Linux. Details are available in [Architecture](docs/architecture.md).

## License

Autobahn is dual licensed: `AGPL-3.0-or-later OR LicenseRef-Commercial`.

- **[AGPL-3.0-or-later](LICENSE)**: free for any use with copy left caveats
- **[Donor's](DONOR-LICENSE.md)** [donate to Justice-In-Education](docs/donations.md) to use Autobahn free of GPL
