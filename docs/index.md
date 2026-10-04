# Autobahn

_Subsecond sync with German precision._

Keep your files in sync as fast as you (or an agent) edit them across a fleet. No cloud service, no account, no third party.

```sh
curl -fsSL https://github.com/fny/autobahn/releases/latest/download/install.sh | sh
```

Point a coding agent at [INSTALL.md](https://github.com/fny/autobahn/blob/main/INSTALL.md) for interactive setup, or install [Autobahn Dash](app.md) from the [releases page](https://github.com/fny/autobahn/releases).

![Autobahn Dash showing three sync groups across four sessions, all synchronized](assets/screenshots/groups.png)

## Start here

Each **group** connects one root — the _primary_ — to one or more destinations, the _replicas_. Each pair becomes a session. Sync can be one-way, two-way, or P2P.

- [Configuration](configuration.md) — the register in `~/.autobahn/config.toml`, which is the source of truth
- [Commands](commands.md) — what `autobahn` can be asked to do
- [Modes](modes.md) — one-way, two-way, and what each does when both sides change
- [Ignores](ignores.md) — what never travels

## Watching it run

- [Autobahn Dash](app.md) — the desktop app: groups, conflicts, configuration, and the service
- [The menu bar item](tray.md) — the small always-there status item
- [Alerts](alerts.md) — when it tells you something is wrong, and how it decides
- [Logging](logging.md) and [State](state.md) — where it writes, and what lives under `~/.autobahn`

## When two sides disagree

- [Conflict resolution](conflicts.md) — finding them, reading the diff, choosing a winner
- [Git checkouts](git.md) — syncing a working tree without fighting the index

## Whether to trust it

This is the part worth reading before you point it at anything you care about.

- [Safety](safety.md) — the guarantees, in plain terms
- [Invariants](correctness/invariants.md) — what is actually promised, and what proves it
- [Accepted risks](correctness/accepted-risks.md) — the known limits of those promises, written down rather than implied
- [Scope and support](limitations.md) — what is deliberately out of scope
- [Architecture](how-it-works.md) — how the speed is achieved, if you want the mechanism

## How fast

- [Autobahn vs mutagen](benchmarks.md) — the headline comparison
- [Benchmark matrix](benchmark-matrix.md) — every measurement, with its provenance

## Licence

AGPL-3.0-or-later, or a commercial licence free of the AGPL if you [donate to the Justice-in-Education Initiative](donations.md).
