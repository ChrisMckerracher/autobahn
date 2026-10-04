# Accepted risks

This document records unresolved risks associated with [the invariants](./invariants.md). Each entry explains the limitation, why it remains, evidence that can justify further work, and a possible fix.

## 1. Mounts that were never observed mounted

**Risk.** Scans record mount boundaries. With `ignore_mounts = false`, a recorded mount that disappears or empties over ancestor content halts the session at any size. With `ignore_mounts = true`, recorded paths remain excluded on both sides after unmount.

An unobserved mount has no recorded identity. Below the root, its disappearance follows ordinary reconciliation unless `guard_dir_deletes_over` applies. Content below the count threshold, including one large file, receives no additional guard.

The whole-root emptying guard requires two ancestor entries. A missing primary has a separate refusal.

**Reason retained.** An unseen unmount and deliberate deletion have the same tree shape. Byte thresholds also interrupt intentional large-file deletion and cannot establish mount identity.

**Reason to revisit.** Actual losses from mounts that appear and disappear between scans, or a requirement to declare mounts before initial scanning.

**Possible fix.** Add expected-mount configuration or OS mount-event tracking. Test unplugged-at-startup and mounted-between-scans cases.

## 2. A save landing between a check and a replacement

**Risk.** A transition checks an entry against the last scan, then replaces or removes it. Most saves landing in between are now caught and put back (below). These can still be lost:

- **Without an atomic exchange.** On FreeBSD and the other BSDs, and on filesystems that refuse an exchange (some network and FUSE volumes), a save landing after a replacement's last check is overwritten. The last check, made just before acting, keeps that window to microseconds.
- **A program writing into a file it holds open.** It goes on writing to whatever version it opened, so writes after a replacement or removal land in a version being deleted. Linux probes for such a program and leaves the file alone while it holds it open, for up to 30 seconds. Writes made past that, or by a program that opens the file in the microseconds between the probe and the act, are lost. macOS has no such probe, and network filesystems grant no leases, so there it goes undetected.
- **Creations off Linux and macOS.** `RENAME_NOREPLACE` and `RENAME_EXCL` refuse to create over anything that appeared since the check. Other platforms, and filesystems without those flags, keep the window for creations.

**What closes the rest.** Each replacement or removal (`Transitioner::put_in_place` and `Transitioner::remove_checked_file` in `src/endpoint/local.rs`):

- **Checks the target again just before acting**, on every platform. Preparing new content (copying or verifying a large file) takes seconds, all of it after the first validation. *`a_save_landing_while_the_replacement_is_prepared_is_never_replaced`*
- **Replaces by exchange**, on Linux (`RENAME_EXCHANGE`) and macOS (`RENAME_SWAP`): the new file is swapped in, and what came out is checked against the version validated. A save that landed after the last check came out instead, and is swapped back, with the replacement refused as a disagreement for the next cycle to reconcile. *`a_save_landing_after_the_last_check_is_swapped_back_not_replaced`*
- **Removes by moving aside first**, on every platform: the file is renamed out of the way, checked, and only then deleted, or put back. *`a_save_landing_while_a_file_is_removed_is_put_back`*
- **Leaves a file another program holds open**, on Linux. A write lease is granted only on a file nobody else has open, so one is taken and handed straight back as a probe. *`a_file_another_program_has_open_is_left_for_now_then_replaced`* and *`a_file_held_open_past_the_grace_is_replaced_anyway`*

When putting a save back fails, it is kept visibly beside its name, as `<name>.kept`, and reported.

**No longer at risk: redirection out of the root.** The same window once let a local process replace a checked directory with a symbolic link and redirect the operation outside the root. Under `--allow-root`, that reached anything the daemon can. A transition now walks to an entry's directory once and holds each directory open by descriptor: `openat2` with `RESOLVE_BENEATH` on Linux, `openat` with `O_NOFOLLOW` elsewhere. It then acts relative to that descriptor, and nothing it does follows a symbolic link (`src/endpoint/dir.rs`). Reading a file, moving one, and opening a delta base go through the same walk. Staging works through the staging directory it checked, held open from the check onward, so one replaced by a link afterwards, as `staging = "inside-root"` allows a local writer to do, redirects nothing.

That rests on the mechanism, not on a test winning the race. `a_held_directory_is_not_redirected_by_a_link_swapped_in_for_it`, `a_parent_replaced_by_a_link_after_resolution_does_not_redirect_a_move` and `a_staging_directory_replaced_by_a_link_after_its_check_does_not_redirect_staging` swap a directory for a link inside the window, and show the operation stays where it was resolved.

One path is outside it. Supplying content (`open_scanned`) opens a file by name, then compares the opened file's inode and size with the scan, so a redirected open is refused rather than served. On a filesystem that reports no inode numbers only the size is compared.

**Reason retained.** What remains needs an atomic compare-and-replace, which no platform offers, or the writing program's cooperation. Each case needs a write landing in a window of microseconds, or a program writing into a file for longer than the grace while it is replaced.

**Reason to revisit.** Saves lost to in-place writers on macOS, or a need for these guarantees on the BSDs.

**Possible fix.** For in-place writers: hold the replaced version for a few seconds, and check it again before deleting it. For supplying content: open it through the walk, as reads and moves are.

## 3. Network filesystems beyond warn-and-document

**Risk.** NFS, SMB/CIFS, and FUSE roots receive a startup warning and best-effort, single-writer support. Attribute caches can hide another client’s writes from scans and destructive-operation checks. NFS defaults can cache attributes for up to 60 seconds. Watcher events can be absent.

**Reason retained.** Fixes depend on the protocol and server. Close-to-open consistency requires reopening on each read path. Lease handling varies, and neither restores local notification semantics.

**Reason to revisit.** Supporting multi-client network mounts requires a broader product commitment. Evidence of common single-writer NFS use can justify narrower hardening.

**Possible fix.** Start with `fstat` after `open` on destructive paths. Then consider a mount-aware mode without digest reuse. Validate guarantees against a real NFS server before expanding support.

## 4. Cross-process overlapping configurations

**Risk.** The support boundary is one supervisor per folder; see [Limitations](../limitations.md#one-supervisor-per-folder). Nothing enforces it.

Within one configuration the loader refuses nested writable endpoints, and *warns* about equal ones, because an equal one is fan-out: one source to several destinations, which is a supported topology. The endpoint-pair lock then excludes a second run of the same pair, machine-wide per user, from the real `~/.autobahn` — so `--state-root` and `--state-dir` cannot dodge it. Different machines, users, and `AUTOBAHN_HOME` directories are outside it entirely.

What is left is one folder paired with something different in two configurations. Each takes a different pair lock, both run, and each writes the folder from its own ancestor.

**Reason retained.** This is not a topology nobody wants. It is the same shape the loader permits inside one configuration, where one supervisor sequences the sessions over a folder. Across processes the shape is unchanged and the coordinator is gone — so refusing it outright would refuse fan-out, and permitting it is what the documented boundary already tells a reader not to do.

Closing it properly means locking endpoints rather than pairs, on every host involved, agents included. The protocol must then answer stale locks, acquisition order, and supervisor deadlock. Intent records already soften the consequence: two runs tend to produce conflicts rather than silent replacement.

**Reason to revisit.** Broader agent-protocol changes, or evidence of common multi-machine synchronization into shared storage.

**Possible fix.** Add advisory locks keyed by resolved endpoint identity under the endpoint host’s default state root. Use shared locks for read-only one-way primaries and exclusive locks for writable endpoints. Retain the existing pair lock.

## 5. Content changed without its metadata moving

**Risk.** A scan reuses a file's recorded checksum when its modification time, size, inode, and type all match what was recorded (`src/scan/mod.rs`). A rewrite that keeps the length and restores the timestamp, in place, matches all four — so the new content is invisible, to a full scan as much as an incremental one.

Three things do this. Reproducible build tooling pins timestamps on purpose, so byte-different output can land looking identical. `touch -r` copies a timestamp across deliberately. And a writer who wants to hide a change can do both.

The racy-timestamp margin covers accidental same-granule edits. This is the deliberate case, which it does not.

**Reason retained.** Reusing metadata is why a scan does not read every byte of every file, which is most of what makes autobahn fast. rsync, Git's index, and Mutagen all make the same trade.

**Reason to revisit.** Reports of build output that quietly failed to travel, or a deployment whose threat model includes deliberate metadata restoration.

**Possible fix.** `autobahn verify` already forces content reads on the next cycle. Run it on a timer rather than when it occurs to you — cron, a systemd timer, or a launchd job:

```
0 3 * * 0  autobahn verify
```

Weekly bounds how long a change can hide. There is nothing to build for this, and a setting to do it internally would only move the schedule inside the configuration.

Two things to know before relying on it. `verify` turns off checksum reuse and nothing else: what it finds reconciles as an ordinary change, and nothing marks it as having been hidden. So a verification that discovers a *tampered* file propagates it to the other side like any edit — containment is a side effect of not having looked. That is a reason to treat this as a correctness measure for build output, and not as tamper detection.

## See also

- [Invariants](./invariants.md) — the guarantees these are the limits of
- [Safety](../safety.md) — the same ground, for a reader who is not auditing
- [Limitations](../limitations.md) — what is out of scope rather than unresolved
