# The menu bar item

**Experimental.** The item works, and it is what the release ships, but its menu and what it does on a click are still moving. What it reads — `status --json` and `resolve` — is not, so nothing it shows can go stale behind Autobahn's back.

It watches the supervisor and holds no state of its own. The supervisor runs in the background whether the item is there or not.

## What it shows

The Autobahn sign — two lanes to the horizon under a bridge — drawn in the menu bar's own ink, with a dot at its corner for the state of every session:

|                            |                                    |
| -------------------------- | ---------------------------------- |
| **Green**                  | everything synchronized            |
| **Amber**                  | something is in conflict           |
| **Red**                    | something is halted or unreachable |
| **Struck through, no dot** | nothing is running                 |

The ink comes from the menu bar itself, which matters on macOS 26, where the bar picks black or white from the wallpaper behind it — so a light system over a dark wallpaper still gets a white sign. It follows a change on the next poll.

## The menu

Each group, and each destination with its state. Under a conflict, the ways to settle it: diff, keep the primary's version, keep that destination's, or keep both. It also starts, stops and restarts the login service, and opens its log.

When the running supervisor has refused an edit to the configuration, the menu says so on a line of its own and notifies once; sessions carry on under the last configuration that loaded — see [live reload](./configuration.md#live-reload-behavior).

Choices are queued rather than run where you click. They go to a worker thread in the order you made them, and the menu says how many are waiting — so a second choice made while the first is still going is kept instead of lost, and a slow resolve cannot freeze the menu bar.

## Two ways to have one

**Inside [Autobahn Dash](./app.md).** Dash includes the item and runs it in the same process. Choose _Menu Bar_ in its Service pane for the item alone, or _Both_ for the item and the window. This is the easier route if you already want Dash.

**The standalone app.** The `autobahn` binary with `--features tray` and nothing else — no window, no GPUI, no graphics stack. Worth it on a machine where you want a status light and not an application. On macOS, build it from the repository:

```sh
apps/tray/build.sh          # builds Autobahn.app
open apps/tray/Autobahn.app # or drag it to /Applications
```

It is a way to launch `autobahn tray`, not a second implementation — the same binary, running the same `resolve` a terminal would. What the bundle adds is an identity: macOS takes a notification's icon from the bundle that sent it, and a bare executable has none, so without one every alert wears the icon of whatever ran it.

From a terminal, `autobahn tray` runs the same thing, but only in a binary built with `--features tray`. A plain build answers that it has no menu bar app. It accepts `--config FILE` and `--state-root DIRECTORY`; without them it uses the default configuration and state root (`AUTOBAHN_HOME`, or `~/.autobahn`).

## Notifications

With no `on_alert` hook configured, the item raises desktop notifications itself, under exactly the rules the hook would use: a condition has to hold before it counts, only something _joining_ the set in trouble is news, a cascade is gathered into one, and recovery is silent. See [Alerts](./alerts.md).

With a hook configured it stays quiet. The hook is then the one place notifications come from — two sources following identical rules would still say everything twice.

Inside Dash, the item is what speaks, and Dash's notification switch turns it off. Dash notifies for itself only when no item is running. See [the app](./app.md#notifications).

## Starting at login

**It does not start itself.** Add it under System Settings → General → Login Items on macOS, or your desktop's autostart settings on Linux.

**The supervisor is separate.** `autobahn install` registers it to start at login, independently. That is what keeps your files in sync; the item only watches it.

## On Linux

Dash's menu bar item runs on Linux from the `app-latest` archives, though it is the half most likely not to appear — see [the app](./app.md#on-linux).

The **standalone tray** is a different matter:

> **Build it yourself, at your own risk.** The tagged releases include no standalone Linux tray, and ordinary CI does not build the `tray` feature on Linux. Nobody has confirmed that the steps below build it, or that it runs once built. It can break between versions without anyone noticing.

Desktop notifications go through the freedesktop notification service, and the diff opens with `xdg-open`.

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

## See Also

- [Autobahn Dash](./app.md): The desktop app and its built-in menu bar item
- [Alerts](./alerts.md): Notification rules and custom hooks
- [Conflicts](./conflicts.md): Conflict resolution actions available from the menu
- [Terminal interface](./shop.md): Session monitoring and control in a terminal
- [Development](./development.md#the-menu-bar-app-bundle): How to build the tray bundle
- [Releases](./releases.md#signing-and-notarising-macos): macOS signing and notarization.
