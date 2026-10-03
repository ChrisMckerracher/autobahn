#!/bin/sh
# Every view of the app, against fixtures, without touching this machine.
#
#   scripts/views.sh list                 the fixtures and what each is for
#   scripts/views.sh show calm conflicts  open one, to tweak how it looks
#   scripts/views.sh sheet                photograph all of them, tiled
#
# The fixtures are built by examples/fixtures.rs into target/views. Each
# one is a configuration and a state root the app reads as if it were
# real, plus an `env` file saying whether the command is installed and
# what the login service is doing — the two answers that are not in a
# state root.
set -eu

here=$(cd "$(dirname "$0")/.." && pwd)
views="$here/target/views"
shots="$here/target/views-shots"
cargo="cargo +1.98.0"

# Every pane the window can open on. `config:<section>` opens the
# configuration on one section, which is how a picture of a section is
# taken without a hand on the mouse.
panes="groups hosts conflicts log service config"

build() {
  $cargo build --quiet --features dash --bin autobahn-dash
  $cargo run --quiet --example fixtures -- "$views" >/dev/null
}

# Runs the app against one fixture. Everything after the fixture name is
# handed to the binary, so `shoot` and `show` share one launcher.
launch() {
  fixture=$1
  shift
  at="$views/$fixture"
  [ -d "$at" ] || { echo "no fixture named $fixture — try: $0 list" >&2; exit 1; }
  # The fixture's own answers. Exported rather than passed, because the
  # library reads them from the environment wherever it asks.
  # shellcheck disable=SC2046
  export $(grep -v '^#' "$at/env" | xargs)
  "$here/target/debug/autobahn-dash" \
    --config "$at/autobahn.toml" \
    --state-root "$at/state" \
    "$@"
}

case "${1:-show}" in
list)
  $cargo run --quiet --example fixtures -- --list
  ;;

show)
  build
  fixture=${2:-calm}
  pane=${3:-}
  if [ -n "$pane" ]; then
    launch "$fixture" --pane "$pane"
  else
    launch "$fixture"
  fi
  ;;

sheet)
  build
  rm -rf "$shots"
  mkdir -p "$shots"
  for fixture in $($cargo run --quiet --example fixtures -- --list | awk '{print $1}'); do
    # A fixture with no command installed draws the splash whatever pane
    # is asked for, so asking for seven is six identical pictures.
    case "$fixture" in
      fresh) wanted="welcome" ;;
      *)     wanted="$panes" ;;
    esac
    for pane in $wanted; do
      into="$shots/$fixture"
      mkdir -p "$into"
      launch "$fixture" --pane "$pane" --shoot "$into" >/dev/null
      # The binary names its own file after the pane; the sheet needs the
      # fixture in the name too, since every fixture has a `log`.
      taken="$into/kit-${pane%%:*}.png"
      [ -f "$taken" ] && mv "$taken" "$shots/$fixture-${pane%%:*}.png"
    done
    rmdir "$shots/$fixture" 2>/dev/null || true
  done
  count=$(find "$shots" -name '*.png' | wc -l | tr -d ' ')
  echo "$count views in $shots"
  if command -v montage >/dev/null 2>&1; then
    # ImageMagick has no default font on macOS, so name one. Labelled,
    # because a sheet of twenty windows is unreadable without knowing
    # which fixture each one came from.
    font=/System/Library/Fonts/Supplemental/Arial.ttf
    [ -f "$font" ] || font=Helvetica
    montage -label '%t' "$shots"/*.png \
      -tile 4x -geometry '420x+12+12' -background '#1c1c1e' -fill '#f2f2f7' \
      -font "$font" -pointsize 13 "$shots/sheet.png"
    echo "$shots/sheet.png"
  else
    echo "install imagemagick for the tiled sheet (brew install imagemagick)" >&2
  fi
  ;;

*)
  sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'
  exit 1
  ;;
esac
