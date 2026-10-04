# Closing the path race in the transitioner

A work order. Everything below was read out of the tree at commit `2a1e3fc`; line numbers are from that commit and will drift.

This closes the security half of [accepted risks §2](docs/correctness/accepted-risks.md#2-pathname-toctou-outside-linux-creations). Delete this file when it is done.

## The problem

Everything in `src/endpoint/local.rs` that touches a file resolves a path, checks what it found, and then acts on the path again. The kernel looks the name up a second time, and between the two lookups the answer can change.

Two different exposures live in that one gap.

**Symlink redirection — the security one.** A local process replaces a directory along the path with a symbolic link after the check and before the act, and the operation lands outside the synchronization root. Under `--allow-root` or `experimental.allow_root`, a user who can write the tree can use this to reach anything the daemon can.

**The lost save — a correctness annoyance.** `replace_change` reads metadata, validates it against the recorded digest, then publishes over the path:

```rust
let metadata = fs::symlink_metadata(&target)       // check
self.validate_file(path, &metadata, old_digest)?   // still what we recorded?
self.publish_file(path, &parent, &target, ...)     // act
```

A save inside that window is overwritten. Microseconds wide, and far more likely for files written by programs than by people — a build writing output while a cycle replaces it.

**Only the first is worth the work.** The second needs every leaf operation pinned, which is the whole file; the first needs the *walk* pinned, which is three functions.

## What is already true

The code is not careless. `verify_directory` uses `fs::symlink_metadata`, which does not follow links, so a symlinked parent that is **present when looked at** is already refused — and tested:

```
read_file_refuses_a_symlinked_parent              6949
rename_out_through_a_symlinked_parent_is_refused  7008
rename_in_from_a_symlinked_parent_is_refused      7025
a_supply_path_through_a_symlinked_parent_is_refused 7746
a_mode_change_never_reaches_a_hardlink_outside_the_root 7593
a_planted_symlink_at_a_temporary_name_is_not_followed   8097
a_symlinked_inside_root_staging_directory_is_refused    7155
a_base_behind_a_symlinked_parent_yields_an_empty_signature 7878
an_agent_supplies_nothing_outside_its_root        7916
```

So this is **not adding a guarantee**. It is making an existing one hold across the window. Those tests are the regression net and should all still pass unchanged; if one needs editing, something is wrong.

## The three walks

All in `src/endpoint/local.rs`. Same loop, three copies, differing only in error style and whether missing directories are created.

| | line | returns | errors via | creates? | call sites |
| :-- | :-- | :-- | :-- | :-- | :-- |
| `resolve_parent` | 2646 | `Option<(PathBuf, &str)>` | `self.problem(...)`, returns `None` | no | 2761, 3211, 3605 |
| `resolve_confined` | 3959 | `Result<PathBuf>` | `anyhow!` | no | 1855, 1892 |
| `create_confined_parents` | 3980 | `Result<PathBuf>` | `anyhow!` | yes, with `directory_mode` | 1893 |

The shared body:

```rust
let mut current = root.to_path_buf();
verify_directory(&current)?;                 // fs::symlink_metadata, by name
for component in parent.split('/') {
    current.push(component);
    verify_directory(&current)?;             // again, by name
}
```

Each returns a `PathBuf` and throws away everything the walk learned. The caller then does a third lookup when it acts.

Note `resolve_parent` is a method (it needs `self.problem` and `self.root`); the other two are free functions taking `root`.

## Stage one — one walk, no behaviour change

Collapse the three into one walk. Keep the three entry points as thin wrappers so each caller keeps its current signature and error style.

Suggested shape:

```rust
/// Walks from `root` to the parent directory of `path`, one component at
/// a time, refusing a symbolic link anywhere along the way.
fn walk_to_parent(root: &Path, path: &str, create: Option<u32>)
    -> Result<(PathBuf, &str)>
```

`create: Some(mode)` is `create_confined_parents`; `None` is the other two. `resolve_parent` becomes a method that calls it and converts `Err` into `self.problem(...)` + `None`, preserving its exact message — *"unable to resolve path: {message}"*.

**This is where the risk in the whole job lives.** The three differ in how a failure is reported: a flattening that turns `resolve_parent`'s reported problem into a silent skip would be a real regression and the tests may not catch it. Diff the three bodies carefully first; the empty-path and root cases differ too (`resolve_confined` and `create_confined_parents` `bail!` on an empty path, `resolve_parent` does not).

**Commit this on its own and run the suite.** No descriptors yet.

## Stage two — hold descriptors instead of names

Change `walk_to_parent` to return an owned directory descriptor for the parent plus the leaf name, rather than a `PathBuf`.

- Open the root with `O_DIRECTORY | O_NOFOLLOW`.
- For each component, `openat(parentfd, component, O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)`, then close the parent — two descriptors at a time, whatever the depth.
- `O_NOFOLLOW` makes a symlink fail with `ELOOP` rather than redirect. That is the guarantee.
- On Linux, prefer one `openat2` with `RESOLVE_BENEATH`, which resolves the whole relative path in a single syscall and refuses to leave the starting directory — symlinks, `..` and mount crossings all refused by the kernel. Fall back to the loop on `ENOSYS` (pre-5.6) and on macOS.

Then the operations at the end take `*at` forms against that descriptor: `unlinkat`, `renameat`, `openat`, `mkdirat`, `fchmodat`.

**Use raw `libc`, not a new dependency.** This file already does exactly this: `libc::SYS_renameat2` via `syscall`, `libc::renamex_np`, `libc::statfs`, `libc::mkfifo`, `libc::O_NOFOLLOW`. Follow that. `renameat2` at line ~4186 is the model for the `ENOSYS`/`EOPNOTSUPP`/`EINVAL` fallback, including why it is a raw syscall (musl did not export the wrapper until 1.2.5, and the Linux agents are static musl).

**Wrap descriptors in `std::os::fd::OwnedFd`** so they close on drop. The transitioner has many early returns; a descriptor leaked on an error path becomes "too many open files" weeks later in a long-running supervisor, which is a miserable bug to trace.

## Capacity — not a concern, measured

Descriptors scale with directory **depth**, not file count. Measured locally: this repo 8 levels, `~/.cargo/registry` 11, `target/` 5. `PATH_MAX` (1024 on macOS, 4096 on Linux) caps depth in the low hundreds in the worst case. Closing each parent as you descend needs two at a time; only a recursive descent that keeps ancestors open (`create_children`) costs one per level, so a few dozen on a Chromium-sized tree.

Performance should improve slightly. Today `a/b/c` costs three `symlink_metadata` calls, each resolving from the root — nine component lookups. `openat` resolves one component per call — three. More syscalls, much less path resolution; `openat2` is one syscall for the lot.

## What cannot be tested

The race itself. It is a window of microseconds and a test would have to win a scheduling race reliably. What *can* be asserted:

- every existing confinement test above, unchanged
- a new test that the walk refuses a symlinked intermediate directory (already covered, but assert it against the new code path)
- on Linux, that `openat2` is used when available and the fallback works when it is not

"The race is closed" rests on the mechanism, not on a test demonstrating it. Say so in the commit message rather than claiming more.

## Build and test

Rust 1.98.0 is pinned for the app feature; the library and command build on the default toolchain.

```sh
cargo +1.98.0 fmt --check
cargo +1.98.0 check --all-targets --features app
cargo +1.98.0 test --lib endpoint::local        # the file's own tests
cargo +1.98.0 test --test e2e                   # end to end
cargo +1.98.0 test --test permissions
cargo +1.98.0 test --test root                  # the --allow-root paths
```

Feature combinations that must stay clean: none, `tray`, `app`, `tray,app`.

## When it is done

Split [accepted risks §2](docs/correctness/accepted-risks.md#2-pathname-toctou-outside-linux-creations) in two. The symlink-redirection and privilege-escalation half is resolved and comes out; the lost-save half remains, and should then lead the entry rather than sit under it — it is the only thing left.

Also worth doing while in there: the entry currently opens with the lost save and buries the escalation case under "reason to revisit". For anyone reading the register to find security issues — and §2 is the only one in it — that ordering is backwards.
