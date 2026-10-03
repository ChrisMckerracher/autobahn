# Accepted risks

This document records unresolved risks associated with [the invariants](./invariants.md). Each entry explains the limitation, why it remains, evidence that can justify further work, and a possible fix.

## 1. Mounts that were never observed mounted

**Risk.** Scans record mount boundaries. With `ignore_mounts = false`, a recorded mount that disappears or empties over ancestor content halts the session at any size. With `ignore_mounts = true`, recorded paths remain excluded on both sides after unmount.

An unobserved mount has no recorded identity. Below the root, its disappearance follows ordinary reconciliation unless `guard_dir_deletes_over` applies. Content below the count threshold, including one large file, receives no additional guard.

The whole-root emptying guard requires two ancestor entries. A missing primary has a separate refusal.

**Reason retained.** An unseen unmount and deliberate deletion have the same tree shape. Byte thresholds also interrupt intentional large-file deletion and cannot establish mount identity.

**Reason to revisit.** Actual losses from mounts that appear and disappear between scans, or a requirement to declare mounts before initial scanning.

**Possible fix.** Add expected-mount configuration or OS mount-event tracking. Test unplugged-at-startup and mounted-between-scans cases.

## 2. Pathname TOCTOU outside Linux creations

**Risk.** Linux `RENAME_NOREPLACE` and macOS `RENAME_EXCL` provide atomic creation without replacement. Replacements and removals still validate and then act by pathname.

A save within that short check/use window can be lost. A local process can replace a checked directory with a symlink and redirect an operation outside the root. Other platforms and unsupported-flag fallbacks also retain the creation race.

**Reason retained.** Closing the race requires descriptor-relative traversal through `openat2` or `openat` with `O_NOFOLLOW`. Directory descriptors must remain open through relative rename and unlink operations.

This changes the transitioner’s core and needs a separate design and review. The save race is brief. Symlink redirection requires a local writer, within the current single-user threat model.

**Reason to revisit.** A privileged daemon synchronizing user-writable trees turns this race into privilege escalation. Significant transitioner work also provides an opportunity for the refactor.

Root is refused by default. Controllers require `--allow-root` or `experimental.allow_root`. Agents accept root only for sessions with `default_owner` or `default_group`. Root with another user’s `$HOME` remains forbidden.

**Possible fix.** Implement descriptor-relative operations and explicit `ENOSYS`/`EOPNOTSUPP` handling. Use the staging and transition fault harness to validate the refactor.

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

## 5. Forged timestamps beyond the verify verb

**Risk.** A same-length rewrite with a restored mtime and unchanged inode evades metadata detection, including full scans. Racy-timestamp handling covers accidental same-granule edits, not deliberate restoration through `touch -r`, reproducible builds, or hostile writes.

**Reason retained.** Metadata reuse avoids reading every byte on each scan, as in rsync, Git’s index, and Mutagen. `autobahn verify` forces content reads and logs detected mismatches.

**Reason to revisit.** Verification logs that show real divergence, or a deployment whose threat model includes deliberate metadata restoration.

**Possible fix.** Add scheduled background verification that gradually rehashes files during idle cycles. This can bound digest age without hashing the whole tree on every scan.

## See also

- [Invariants](./invariants.md) — the guarantees these are the limits of
- [Safety](../safety.md) — the same ground, for a reader who is not auditing
- [Limitations](../limitations.md) — what is out of scope rather than unresolved
