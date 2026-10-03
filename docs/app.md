# The Autobahn app

**Experimental.** The app works, and it is what the release ships, but its layout and controls are still moving. What it reads — `status --json` and `resolve` — is not, so nothing it shows can go stale behind Autobahn's back.

The app watches the supervisor. It does not replace it: the supervisor runs in the background whether the app is open or not, and nothing the app shows is state of its own.

## Two forms

| | |
|---|---|
| **A window** | Groups, hosts, conflicts, the log, service controls, and a configuration editor. |
| **A menu bar item** | The Autobahn sign with a status dot, and a menu with every group and conflict under it. |

**Autobahn Dash** gives you both from one app, and you choose which to show. The standalone **menu bar app** is a separate, smaller build that is only the menu bar item — useful if a window is more than you want running.

## Getting it

**Autobahn Dash** is built by the `dash.yml` workflow and published to the moving `dash-latest` prerelease: macOS Apple Silicon and Linux x86-64/arm64. This channel is unsigned and separate from the signed command-line releases. A platform whose build failed is simply absent, so check the release notes for the build commit and what it contains.

- **macOS** — open `Autobahn Dash.app`. These builds are not Developer ID signed or notarised, so a downloaded copy is quarantined; the release notes explain the step to clear it.
- **Linux** — extract the archive and run `./autobahn-dash`, keeping the companion `autobahn` executable beside it. You need a graphical session (Wayland or X11), a Vulkan driver, and the desktop libraries the workflow lists.

**The menu bar app** on macOS is built from the repository:

```sh
apps/tray/build.sh          # builds Autobahn.app
open apps/tray/Autobahn.app # or drag it to /Applications
```

It is a way to launch `autobahn tray`, not a second implementation — the same binary, running the same `resolve` a terminal would. What the bundle adds is an identity: macOS takes a notification's icon from the bundle that sent it, and a bare executable has none, so without it every alert wears the icon of whatever ran it.

## First run

![The welcome screen, offering to install the autobahn command](../assets/screenshots/welcome.png)

The app needs the `autobahn` command. It looks beside itself first, then in `~/.local/bin`, `/usr/local/bin`, `/opt/homebrew/bin`, and your PATH.

If it finds none, a welcome screen offers to run the installer for you or to copy the shell command and run it yourself. Installer output goes to `install.log` under the state root, and the Log pane shows it as it happens.

Then configure a group before starting the service — see [Installation](../INSTALL.md).

Keep the command matched to the running supervisor. A version mismatch is reported in the status area rather than left to surprise you.

## The window

![The Groups pane: three groups across four sessions, all synchronized](../assets/screenshots/groups.png)

| Pane | What it does |
|---|---|
| **Groups** | The configured groups, their destinations, current work, and anything wrong. |
| **Hosts** | The same sessions collected by host, with host controls. |
| **Conflicts** | Disagreements and diffs, with actions to keep one side or both. |
| **Log** | The supervisor log, or installer output during setup. |
| **Service** | Program and service state, with install, start, stop, restart, and cleanup. |
| **Config** | Top-level settings, defaults, and groups, which can be added, renamed, or removed. |

Conflict actions run the same operations as the command line — see [Conflicts](./conflicts.md). The app never merges file contents. A conflict can be about a file's executable bit, a symbolic link, or a directory, so there is not always a text difference to show.

**Diff** puts the two sides underneath, labelled by side rather than by the files actually compared:

![The Conflicts pane with a unified diff open between the primary and a replica](../assets/screenshots/conflicts.png)

## The menu bar

The Autobahn sign — two lanes to the horizon under a bridge — drawn in the menu bar's own ink, with a dot at its corner for the state of every session:

| | |
|---|---|
| **Green** | everything synchronized |
| **Amber** | something is in conflict |
| **Red** | something is halted or unreachable |
| **Struck through, no dot** | nothing is running |

The ink is read from the menu bar itself, which matters on macOS 26, where the bar picks black or white from the wallpaper behind it — so a light system over a dark wallpaper still gets a white sign. It follows a change on the next poll.

The menu lists each group and each destination with its state, and under a conflict the ways to settle it: show the difference, keep the primary's version, keep that destination's, or keep both. It also starts, stops, and restarts the login service, and opens its log. When the running supervisor has refused an edit to the configuration, the menu says so on a line of its own and notifies once; sessions carry on under the last configuration that loaded (see [editing it while it runs](./configuration.md#live-reload-behavior)).

Choices are queued rather than run where you click. They go to a worker thread in the order you made them, and the menu says how many are waiting — so a second choice made while the first is still going is kept instead of lost, and a slow resolve cannot freeze the menu bar.

## Choosing what shows

![The Service pane: the supervisor, login service, notifications, and what the app shows](../assets/screenshots/service.png)

In Dash's Service pane, pick a window, a menu bar item, or both. The default is both.

The choice is this machine's, and so is the notification switch above it. Both are saved in `dash.toml` under the state root, which is not part of the fleet configuration:

```toml
presence = "both"   # both, window, or menubar
notify = true       # whether the app raises desktop notifications itself
```

## Editing configuration

![The Configuration pane, editing the top-level settings](../assets/screenshots/configuration.png)

Edits stay in the form until you press **Save**. The editor checks them through Autobahn's own configuration loader and marks errors on the fields they belong to; an invalid configuration is never written. **Reload** reads the file again and throws away pending changes. Optional switches keep the difference between inheriting a value and setting it deliberately.

Advanced fields fold away. Experimental controls appear after five clicks on the Autobahn wordmark; what they mean and what they risk is in [Configuration](./configuration.md), [Alerts](./alerts.md), and [P2P](./p2p.md).

A running supervisor picks up a saved file through [live reload](./configuration.md#live-reload-behavior). With live reload off, restart it to apply the change.

## Notifications

The app raises desktop notifications itself, under exactly the rules an `on_alert` hook would use: a condition has to hold before it counts, only something *joining* the set in trouble is news, a cascade is gathered into one, and recovery is silent. See [Alerts](./alerts.md).

This does not depend on showing a menu bar item. A window with no menu bar still notifies — choosing where the app appears is not a choice about whether anything tells you a session has halted. Exactly one thing speaks per app: the menu bar item when there is one, the window when there is not.

Turn them off with the switch in the Service pane.

**Do not leave them on alongside an `on_alert` hook.** The hook follows the same rules, so both means being told everything twice. The Service pane says so when it finds a hook configured and the switch still on; turn off whichever you want less. A hook you add while the app is open is noticed within a few seconds, without a restart.

## Starting at login

**The app does not start itself.** Add it under System Settings → General → Login Items on macOS, or your desktop's autostart settings on Linux.

**The supervisor is separate.** `autobahn install`, or the service controls in the app, registers it to start at login. That is what keeps your files in sync; the app only watches it.

## Options

Both forms accept `--config FILE` and `--state-root DIRECTORY`. Without them they use the default configuration and state root (`AUTOBAHN_HOME`, or `~/.autobahn`).

From a terminal, `autobahn tray` runs the menu bar app — but only in a binary built with `--features tray`. A plain build answers that it has no menu bar app.

## On Linux

Dash runs on Linux from the `dash-latest` archives, including its menu bar item.

The **standalone tray** on Linux is a different matter:

> **Build it yourself, at your own risk.** The tagged releases include no standalone Linux tray, and ordinary CI does not build the `tray` feature on Linux. Nobody has confirmed that the steps below build it, or that it runs once built. It can break between versions without anyone noticing.

Desktop notifications go through the freedesktop notification service, and "Show diff" opens with `xdg-open`.

**What you need**

- Rust, installed with `rustup`.
- GTK 3 development packages. On Debian and Ubuntu this is probably:

  ```sh
  sudo apt install libgtk-3-dev libxdo-dev libayatana-appindicator3-dev
  ```

  That list comes from the tray libraries' own requirements and has not been checked here. Other distributions name these packages differently.
- A desktop that shows tray icons through AppIndicator or StatusNotifierItem. KDE and most others do; GNOME needs an extension such as "AppIndicator and KStatusNotifierItem Support".

**Build and run**

```sh
git clone https://github.com/fny/autobahn && cd autobahn
CARGO_TARGET_DIR=target/tray cargo build --release --locked --features tray
target/tray/release/autobahn tray
```

Never build into the directory a login service runs from. If the service runs `target/release/autobahn`, the `CARGO_TARGET_DIR` above is what keeps the two apart.

**Known problems**

- **It may build and never show an icon.** The tray libraries need GTK started on the thread running the event loop, and the code does not start it today. This is expected from the libraries' documentation rather than observed.
- **It adds GTK 3 and notification dependencies** that the plain command-line tool and agent do not have. See `Cargo.lock` and `deny.toml`.
- **It does not start at login.** Add `autobahn tray` to your desktop's autostart settings.

If you get it working, the exact packages, desktop, and steps are worth reporting, so this section can drop its warning.

## See also

- [Alerts](./alerts.md) — the rules behind every notification
- [Conflicts](./conflicts.md) — what the resolve actions do
- [The shop](./shop.md) — the terminal counterpart
- [Development](./development.md#desktop-app-and-shared-text) — building either app, and the icon
- [Releases](./releases.md#signing-and-notarising-macos) — signing and notarising
