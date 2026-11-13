# Layer auto-shutdown design

## Intent

Every bare `devlayer <cmd>` now creates a new layer, and every layer keeps
running (PID 1, overlay mounts, cgroup, pid file) until `devlayer stop` or a
reboot. Layers pile up. Separately, a layer's PID 1 never reaps processes, so
anything orphaned inside a layer (a background job whose shell exited) stays
`<defunct>` until the layer is stopped.

A layer should shut itself down once nothing is running in it any more, and
clean up after itself, so idle layers don't accumulate.

## Goals

- PID 1 reaps orphaned processes in its layer, so there are no zombies.
- When the last process in a layer exits (anything other than PID 1, not just
  `devlayer` commands), the layer shuts down within about a second.
- Shutting down does what `devlayer stop` does today: overlay and bind mounts
  are unmounted, the layer's cgroup (if it has one) is removed, and the pid
  file is removed.
- The layer's changes are kept. `status`, `diff`, `accept`, `reject` work as
  they do on any stopped layer, and running a command in it again
  (`--last`, `--join`) restarts it as today.

## Non-goals

- Deleting layers (auto-clean of stopped layers) — a later iteration.
- An opt-out flag to keep idle layers running.
- Handling a command that joins a layer at the instant it shuts down. The
  user ruled this out of scope: it is not a concern in practice. Taking the
  existing layer lock during cleanup is enough; no retry logic.

## Anti-goals

- Killing anything still running. A layer with a background process left in
  it (a dev server started with `&`, a tmux server, a daemon) stays up until
  that process exits or the user runs `devlayer stop`.

## Approaches

1. **Host-side supervisor + PID 1 idle check (chosen).** A small root process
   stays on the host as PID 1's parent. PID 1 reaps orphans and checks its own
   `/proc` once a second; when only PID 1 is left it exits. The supervisor sees
   that exit immediately (it is the parent) and does the cleanup. Covers
   orphans that outlive every command, and cleans up even if the user's
   terminal is killed. Cost: one idle process per running layer, up to ~1s
   delay.
2. **The waiting `devlayer` command cleans up.** The `devlayer` process that
   waits on the user's command keeps root and stops the layer when its command
   exits and nothing else is running. Rejected: a background process that
   outlives every command leaves the layer up forever (violates the goal), no
   cleanup if that process is killed, and it leaves a root process inside the
   layer's mount namespace.
3. **Lazy cleanup.** PID 1 exits when idle; the next `devlayer` invocation
   unmounts layers whose PID 1 is gone. Rejected: mounts and cgroups linger
   until the user happens to run `devlayer` again.

## Design

### Process structure

Today `start_layer` (src/layer/get_or_create.rs) is called by the user's
`devlayer` process (call it the caller). It `clone3`s PID 1, writes the pid
file, waits on a pipe for PID 1 to report it is ready, and then goes on to
`exec` the user's command and wait on it. PID 1's parent is the caller, and
once the caller exits PID 1 is reparented to the host's init.

New structure:

- The caller forks a **supervisor** before `clone3`. The supervisor calls
  `setsid` and does the `clone3` of PID 1, so it is PID 1's parent.
- *(Builder-check call)* The supervisor keeps stdin/stdout/stderr open until
  PID 1 has reported ready or failed, so PID 1's setup errors still reach the
  terminal (`tests/isolated_test_pivot_root_failure.rs` relies on that). Then
  it closes them, as PID 1 does today, so the test harness and terminals
  don't wait on it.
- *(Builder-check call)* `clone3` must set `exit_signal` to `SIGCHLD`.
  `Clone3Args` leaves it at 0 today, and a plain `waitpid` never reports the
  exit of a child that doesn't signal its parent.
- *(Builder-check call)* The supervisor sets all its uids and gids to root
  (as PID 1 already does) so the user can't signal it, and ignores SIGHUP and
  SIGINT. If it died, PID 1 would still exit when idle but the mounts would be
  left behind.
- The caller still learns PID 1's pid and whether setup succeeded, and still
  returns an error exactly as today when setup fails. How the pid and the
  ready/failed byte get back to the caller (e.g. the existing pipe plus the
  pid file, or a second pipe) is left to the builder.
- If setup fails, the supervisor exits without leaving anything running; the
  caller's existing error path (removing the cgroup) still runs.
- The supervisor stays root, stays in the host mount namespace, and does
  nothing but `waitpid` on PID 1.

### PID 1

Replace the sleep-forever loop at the end of `setup_layer_and_sleep_forever`:

- Every 1 second:
  1. Reap every exited child with `waitpid(-1, WNOHANG)` until none are left.
  2. List the numeric entries of `/proc` (PID 1's own procfs, which only shows
     this layer's processes). If the only entry is `1`, exit with status 0.
- The first check happens 1 second after PID 1 reports ready. That gives the
  caller time to join with the user's command; no other startup handling.
- The existing SIGTERM handler (exit with the signal number) stays.
- Coverage: keep dumping profraw as the current loop does.

### Supervisor cleanup

When `waitpid` reports PID 1 has exited (idle exit, `devlayer stop`, or
anything else):

1. Take the layer lock. *(Builder-check call)* Open the existing lock file
   without creating it: `Lock::layer` creates a missing one, and after
   `devlayer delete` that would bring the deleted layer back in `list`. If the
   lock file is gone, the layer was deleted (which already stopped and
   unmounted it); just exit.
2. Remove the pid file if it still names this PID 1 (a restarted layer has
   written a new pid; leave it alone).
3. Remove the layer's cgroup, but only if this layer created its own (the
   `create_cgroup_or_share_outer` result was `Some`) and it still exists:
   `Layer::cgroup_path()`. `remove_cgroup` retries for 5 seconds on any
   error, including "not found", so check first. Note that
   `running_cgroup()` can't be used here because it reads PID 1's
   `/proc/<pid>/cgroup`, which is gone.
4. Unmount the layer's mounts (`Layer::unmount`).
5. Exit.

Every step must be harmless when `devlayer stop` already did it (the common
case for an explicit stop): no mounts, no pid file, no cgroup. The supervisor
has no stdout/stderr, so failures are ignored rather than reported.

Skip steps 2–4 when the layer was restarted in the meantime: if, after taking
the lock, the pid file names a different, running PID 1, the supervisor must
not unmount that layer's mounts; it just exits.

### What doesn't change

- `devlayer stop` keeps its current behavior; the supervisor's cleanup after it
  is a no-op.
- `get_running_layer_pid` and `list` keep using the pid file; a shut-down layer
  shows as stopped.
- Nested layers work the same way (the supervisor runs inside the outer layer).

## Constraints

- The binary is setuid root; the supervisor runs as root on the host. It must
  not run any user-supplied code or read user-controlled paths beyond what
  `stop` already does.
- `fork` inherits the caller's flock fd for the layer lock. The caller's
  `Lock` drop does an explicit unlock today, so the inherited fd doesn't keep
  it locked, but the builder should close it in the supervisor anyway.
- Coverage instrumentation: the supervisor is a long-lived instrumented
  process. Like PID 1 and the exec waiter, it must write its profraw to
  `coverage/profraw/` via `__llvm_profile_set_filename` with a path that works
  from its cwd (or an absolute one), and dump before exiting.
- Project rule: very high test coverage; new tests go in `tests/ai_test_*.rs`.

## Testing

New tests (in a new `tests/ai_test_auto_shutdown.rs`):

- After `devlayer true`, the layer stops within a few seconds: the pid file
  goes away, `list` shows it stopped, and no mounts under the layer dir remain.
  Poll with a timeout rather than sleeping a fixed time.
- A layer with a background process left running (`sh -c 'sleep 30 &'`) is
  still running after a few seconds; it stops once that process is killed.
- An orphan inside a running layer is reaped (no `Z` state process in the
  layer).
- Changes survive shutdown: write a file, wait for shutdown, `status` shows it.
- With `--cpus`/`--memory`, the layer's cgroup is removed on shutdown.
- Rejoining a shut-down layer with `--join` restarts it.
- `devlayer stop` still works and the supervisor's cleanup afterwards doesn't
  break anything (layer can be restarted and deleted).

Existing tests that assume a layer stays running between two `devlayer`
invocations (e.g. limits tests that look up the cgroup after `true`, tests of
"running layer with different settings" errors, `list` running checks) will
become timing-dependent. They must keep the layer up explicitly, for example
with a test-fixture helper that starts a background `sleep` in the layer, rather
than relying on running within the 1-second window.

## Left to the builder

- How the supervisor passes PID 1's pid and the ready/failed status back to
  the caller.
- The exact fixture helper for keeping a layer alive in tests, and which
  existing tests need it (find them by running the suite).
- Whether to name the supervisor process (e.g. via `prctl(PR_SET_NAME)`) for
  readability in `ps`.
