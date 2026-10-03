#!/bin/bash
# Signs, notarises and staples Autobahn.app for another machine.
#
# Gatekeeper refuses an app it cannot attribute, and it will not attribute
# one signed ad-hoc or with an Apple Development certificate — those are
# for the machine that built them. Distribution needs a Developer ID
# Application certificate, and then Apple's own scan: notarisation is that
# scan, and stapling attaches its verdict to the app so it is trusted with
# no network.
#
# Only needed for an app someone downloads. A copy that arrives by scp or
# by autobahn itself is never quarantined, so Gatekeeper never asks.
#
#   apps/tray/release.sh                       # build, sign, notarise, staple
#   apps/tray/release.sh --sign-only <app>     # sign, notarise, staple a built app
#   AUTOBAHN_NOTARY_PROFILE=work release.sh     # a different stored credential
#
# --sign-only compiles nothing. CI builds with `build.sh --unsigned` first
# and only then imports the certificate, so no dependency's build script or
# proc macro runs while the signing identity is usable.
#
# CI has no keychain to store a notary profile in, so it passes an App
# Store Connect API key instead, and asks for the stapled app archived:
#
#   AUTOBAHN_NOTARY_KEY=AuthKey_XXXXXXXXXX.p8 AUTOBAHN_NOTARY_KEY_ID=XXXXXXXXXX \
#   AUTOBAHN_NOTARY_ISSUER=<issuer uuid> AUTOBAHN_RELEASE_ZIP=Autobahn.zip \
#   apps/tray/release.sh
set -euo pipefail
usage() { echo "usage: $0 [--sign-only path/to/Some.app]" >&2; exit 2; }
BUILD=yes
APP="apps/tray/Autobahn.app"
case $# in
    0) ;;
    2) [ "$1" = --sign-only ] || usage
       BUILD=no
       # The caller's path, resolved before leaving their directory.
       APP="$2"; [[ "$APP" = /* ]] || APP="$PWD/$APP" ;;
    *) usage ;;
esac
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
source apps/tray/notary.sh
ZIP_OUT="${AUTOBAHN_RELEASE_ZIP:-}"

# Whatever the bundle calls its main executable. The menu bar app's is
# `autobahn` and the window's is `autobahn-app`, and this script signs
# either, so the name to check is the one the bundle states.
if [ "$BUILD" = no ]; then
    if [ ! -f "$APP/Contents/Info.plist" ]; then
        echo "not an app bundle: $APP (no Contents/Info.plist)" >&2
        exit 1
    fi
    MAIN="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' \
            "$APP/Contents/Info.plist" 2>/dev/null || true)"
    if [ -z "$MAIN" ] || [ ! -x "$APP/Contents/MacOS/$MAIN" ]; then
        echo "not a built app: $APP has no runnable Contents/MacOS/${MAIN:-<unset>}" >&2
        exit 1
    fi
fi

# Both settled before the build, not after it: a missing credential should
# cost a second, not a minute of compiling first.
signing_identity
notary_credential

if [ "$BUILD" = yes ]; then apps/tray/build.sh --unsigned "$APP"; fi

# Cleared again because a bundle copied in (from another job, say) is
# tagged anew, and codesign refuses one carrying com.apple.provenance.
xattr -cr "$APP"
# The hardened runtime and a secure timestamp are both what notarisation
# requires, so there is no fallback without the timestamp here: better to
# fail now than after an upload.
#
# Inner first, and no --deep. Signing a bundle signs its main executable
# and seals its resources; a *second* Mach-O beside it in MacOS/ is
# reached by neither, and notarisation rejects the bundle for the
# unsigned one. The window's bundle carries the command next to it for
# exactly that reason, so this is not hypothetical. --deep would find
# them, but it signs everything with one set of options and Apple has
# told people not to use it for years.
MAIN="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' \
        "$APP/Contents/Info.plist")"
for nested in "$APP/Contents/MacOS/"*; do  # shellcheck disable=SC2043
    [ -f "$nested" ] || continue
    [ "$(basename "$nested")" = "$MAIN" ] && continue
    echo "  nested: $(basename "$nested")"
    codesign --force --options runtime --timestamp --sign "$IDENTITY" "$nested"
done
codesign --force --options runtime --timestamp --sign "$IDENTITY" "$APP"
codesign --verify --strict "$APP"
echo "signed as: $IDENTITY"

# notarytool takes an archive, never a bare bundle.
ZIP="$(mktemp -d)/Autobahn.zip"
ditto -c -k --keepParent "$APP" "$ZIP"
notarize "$ZIP"
# Stapling the app, not the archive: the ticket has to travel inside the
# thing that gets opened.
xcrun stapler staple "$APP"
spctl -a -vvv "$APP"
echo
if [ -n "$ZIP_OUT" ]; then
    # Archived after stapling, so the ticket is inside what ships.
    ditto -c -k --keepParent "$APP" "$ZIP_OUT"
    echo "notarised and archived: $ZIP_OUT"
else
    echo "notarised. Ship an archive made *after* stapling:"
    echo "  ditto -c -k --keepParent $APP Autobahn.zip"
fi
