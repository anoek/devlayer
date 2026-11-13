# Layer Auto-Shutdown Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A layer reaps its orphans and shuts itself down (unmount, cgroup, pid file) once nothing is running in it, keeping its changes.

**Architecture:** The `devlayer` process that starts a layer forks a root supervisor that `clone3`s PID 1 and waits on it. PID 1 reaps orphans and exits when its `/proc` shows only itself. When PID 1 exits for any reason, the supervisor does `stop`'s cleanup under the layer lock.

**Tech Stack:** Rust, nix, libc, clone3, cgroup v2, rstest integration tests.

**Spec:** docs/superpowers/specs/2026-11-13-layer-auto-shutdown-design.md

## Global Constraints

- PID 1 idle check every 1 second; first check 1 second after PID 1 reports ready.
- PID 1 exits with status 0 when idle; the existing SIGTERM handler stays.
- Shutdown never kills a process; a layer with any process besides PID 1 stays up.
- Shutdown keeps the layer's data; it never deletes.
- No retry/race handling beyond taking the layer lock during cleanup.
- The supervisor runs as root (all uids/gids 0), ignores SIGHUP and SIGINT, has no output after PID 1 is ready, and ignores its own cleanup errors.
- The supervisor is forked before the caller joins the layer's cgroup and must never be in it (it would keep the cgroup busy).
- Coverage builds: every instrumented process that exits writes its profraw (see Task 1 step 3).
- New tests go in `tests/ai_test_*.rs`; layer names given in tests start with `devlayer-coverage`.

## Review Focus

1. A layer is restarted (`--join`) before the old supervisor runs its cleanup: the old supervisor must not unmount or remove the pid file of the new instance.
2. `devlayer delete` on a running layer: the supervisor wakes after the lock file is gone and must exit without recreating `<name>.lock` (the layer must not reappear in `list`).
3. A layer with `--cpus`/`--memory`: the cgroup is removed on shutdown, and the supervisor isn't in the cgroup (rmdir would fail with EBUSY).
4. The user's terminal goes away (SIGHUP to the session) while a layer runs: the supervisor survives and still cleans up.
5. Nested layers (devlayer inside a layer): auto-shutdown and cleanup work the same.

---

### Task 1: Supervisor process

**Files:**
- Modify: `src/layer/get_or_create.rs` (`start_layer`, `get_or_create` error path)
- Create: `src/layer/supervisor.rs` (supervisor wait + cleanup), register in `src/layer/mod.rs`
- Modify: `src/util/lock_layer.rs` (open-existing variant)
- Test: `tests/ai_test_auto_shutdown.rs`

**Interfaces:**
- Produces: `Lock::existing_layer(layers_storage_dir: &Path, layer_name: &str) -> Result<Option<Box<Lock>>>` — `Ok(None)` when the lock file doesn't exist; never creates it.
- Produces: `fn supervise(layer: &Layer, pid1: Pid, own_cgroup: bool) -> !` in `src/layer/supervisor.rs`: waits on PID 1, runs cleanup, exits 0.
- Produces (tests): helpers in `tests/ai_test_auto_shutdown.rs`: `fn pid_file(layer: &LayerManager) -> Result<PathBuf>` (`layer.dir()` + `.pid`), `fn layer_pid(layer: &LayerManager) -> Option<i32>`, `fn has_mounts(layer: &LayerManager) -> Result<bool>` (any `/proc/self/mountinfo` mount point under `layer.dir()`), `fn wait_until(timeout: Duration, f: impl FnMut() -> bool) -> bool` (polls every 50 ms), `fn ppid_of(pid: i32) -> Result<i32>` (`PPid:` line of `/proc/<pid>/status`).

- [ ] **Step 1: Write the failing tests**

```rust
/// When PID 1 dies, the supervisor unmounts and removes the pid file
#[rstest]
fn ai_test_supervisor_cleans_up_after_pid1_exit(mut layer: LayerManager) -> Result<()> {
    layer.run(&["true"])?;
    let pid = layer_pid(&layer).expect("layer should be running");
    assert!(has_mounts(&layer)?);
    Command::new("sudo").args(["kill", "-KILL", &pid.to_string()]).status()?;
    assert!(wait_until(Duration::from_secs(5), || !pid_file(&layer).unwrap().exists()));
    assert!(wait_until(Duration::from_secs(5), || !has_mounts(&layer).unwrap()));
    Ok(())
}

/// The user can't signal the supervisor
#[rstest]
fn ai_test_supervisor_not_killable_by_user(mut layer: LayerManager) -> Result<()> {
    layer.run(&["true"])?;
    let pid = layer_pid(&layer).expect("layer should be running");
    let ppid = ppid_of(pid)?; // PPid: line of /proc/<pid>/status
    let status = Command::new("kill").args(["-TERM", &ppid.to_string()]).status()?;
    assert!(!status.success());
    Ok(())
}

/// After an explicit stop, the supervisor's cleanup is harmless: the layer
/// restarts and deletes normally, and delete doesn't leave a lock file behind
#[rstest]
fn ai_test_supervisor_after_stop_and_delete(mut layer: LayerManager) -> Result<()> {
    layer.run(&["true"])?;
    layer.run(&["stop"])?;
    layer.run(&["true"])?;
    let lock = layer.dir()?.with_extension("lock");
    layer.run(&["delete", "-y"])?;
    std::thread::sleep(Duration::from_secs(2));
    assert!(!lock.exists());
    layer.run(&["list"])?;
    assert!(!layer.last_stdout.contains(&layer.name));
    Ok(())
}
```

Note for the first test: until Task 2, PID 1 doesn't exit by itself, so the layer stays running between `true` and the kill.

- [ ] **Step 2: Run tests to verify they fail**

Run: `TEST=ai_test_supervisor make quick-test` (or `cargo test --profile coverage --features coverage --test ai_test_auto_shutdown` with the coverage env from the Makefile)
Expected: `ai_test_supervisor_cleans_up_after_pid1_exit` FAILS (pid file and mounts remain); `ai_test_supervisor_not_killable_by_user` FAILS (PID 1's parent is the caller, already exited, so PPid is the host init or the kill hits a user-owned process — either way not the intended supervisor); `ai_test_supervisor_after_stop_and_delete` may already pass — keep it as a guard.

- [ ] **Step 3: Implement**

In `start_layer`: create the ready pipe, then `fork()`. The caller (parent) drops the write end, reads the ready byte as today, then reads PID 1's pid from the pid file (the supervisor writes it before PID 1 can report ready), and returns `Pid` or the existing `"Failed to setup layer"` error. The supervisor (child):
- `setsid()`; `setresuid(0,0,0)`/`setresgid(0,0,0)`; ignore SIGHUP and SIGINT; close the inherited layer-lock fd.
- `clone3` PID 1 with the existing args plus `exit_signal: libc::SIGCHLD as u64`; write the pid file; keep stdio open.
- Wait for PID 1's ready/failed byte on a second pipe from PID 1 (or forward the existing one); forward it to the caller; on failure exit 1. On success close stdin/stdout/stderr, then `supervise(...)`.

`supervise`: `waitpid(pid1)`; then `Lock::existing_layer` — `None` → exit; if the pid file names a different PID that `get_running_layer_pid` reports running → exit; otherwise remove the pid file if it names `pid1`, remove `layer.cgroup_path()` with `remove_cgroup` only if `own_cgroup` and the path exists, `layer.unmount()`, exit 0 via `std::process::exit` (the profile runtime writes the supervisor's profraw at exit; with `LLVM_PROFILE_FILE` absolute this lands in `coverage/profraw/`; confirm a supervisor profraw appears after a test run, and if not, set the filename explicitly as `exec.rs` does).

`own_cgroup` is `cgroup.is_some()` from `create_cgroup_or_share_outer` in `get_or_create`, passed into `start_layer`.

- [ ] **Step 4: Run tests to verify they pass**

Run: same as step 2, plus `tests/isolated_test_pivot_root_failure.rs` and `tests/test_layer_failures.rs` (setup errors still reach stderr).
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src tests/ai_test_auto_shutdown.rs
git commit -m "Supervise layer PID 1 and clean up when it exits"
```

### Task 2: PID 1 reaps orphans and exits when idle

**Files:**
- Modify: `src/layer/get_or_create.rs` (end of `setup_layer_and_sleep_forever`; rename to `setup_layer_and_wait`)
- Modify: `tests/fixtures/fixture_layer.rs` (keep-alive helper)
- Modify: existing tests that need a layer to stay running between commands (found by running the suite; likely candidates: `ai_test_resource_limits.rs`, `ai_test_layer_settings.rs`, `test_list.rs`, `isolated_test_clean_running.rs`, `isolated_test_stop_*.rs`, `test_mount_unmount_kill.rs`, `ai_test_delete.rs`)
- Modify: `README.adoc` (layer lifetime: auto-shutdown, `stop` still available)
- Test: `tests/ai_test_auto_shutdown.rs`

**Interfaces:**
- Consumes: Task 1's supervisor and test helpers.
- Produces: `LayerManager::hold(&mut self) -> Result<()>` — runs `sh -c "sleep 3600 >/dev/null 2>&1 &"` in the layer so it stays running until `stop`.

- [ ] **Step 1: Write the failing tests**

```rust
#[rstest]
fn ai_test_idle_layer_shuts_down(mut layer: LayerManager) -> Result<()> {
    layer.run(&["touch", "/tmp/devlayer-coverage-idle-file"])?;
    assert!(wait_until(Duration::from_secs(5), || !pid_file(&layer).unwrap().exists()));
    assert!(!has_mounts(&layer)?);
    layer.run(&["list"])?; // shows the layer as stopped
    layer.run(&["status", "/tmp"])?; // changes survive
    assert!(layer.last_stdout.contains("devlayer-coverage-idle-file"));
    layer.run(&["true"])?; // rejoining restarts it
    assert!(layer_pid(&layer).is_some());
    Ok(())
}

#[rstest]
fn ai_test_background_process_keeps_layer_up(mut layer: LayerManager) -> Result<()> {
    layer.hold()?;
    std::thread::sleep(Duration::from_secs(3));
    assert!(layer_pid(&layer).is_some());
    layer.run(&["pkill", "-f", "sleep 3600"])?;
    assert!(wait_until(Duration::from_secs(5), || !pid_file(&layer).unwrap().exists()));
    Ok(())
}

#[rstest]
fn ai_test_orphans_reaped(mut layer: LayerManager) -> Result<()> {
    layer.hold()?;
    // The shell exits right away, orphaning a short-lived child
    layer.run(&["sh", "-c", "(sleep 0.2 &)"])?;
    std::thread::sleep(Duration::from_secs(2));
    layer.run(&["sh", "-c", "grep -l '^State:.*Z' /proc/[0-9]*/status || true"])?;
    assert_eq!(layer.last_stdout.trim(), "");
    Ok(())
}

#[rstest]
fn ai_test_idle_shutdown_removes_cgroup(mut layer: LayerManager) -> Result<()> {
    // Skip like ai_test_resource_limits.rs's limits_available when cgroups
    // can't be managed here
    if !layer.pass(&["--cpus=1", "true"]) { return Ok(()); }
    let cgroup = PathBuf::from("/sys/fs/cgroup/devlayer")
        .join(format!("{}-{}", nix::unistd::getuid(), layer.name));
    assert!(wait_until(Duration::from_secs(5), || !cgroup.exists()));
    Ok(())
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test ... --test ai_test_auto_shutdown` (coverage env)
Expected: `ai_test_idle_layer_shuts_down`, `ai_test_idle_shutdown_removes_cgroup` FAIL (layer stays up); `ai_test_orphans_reaped` FAILS (a `Z` process is listed); `ai_test_background_process_keeps_layer_up` fails on its last assertion.

- [ ] **Step 3: Implement**

Replace PID 1's sleep loop: sleep 1 s, reap with `waitpid(Pid::from_raw(-1), Some(WaitPidFlag::WNOHANG))` until `StillAlive`/`ECHILD`, then if `/proc`'s numeric entries are exactly `["1"]`, dump coverage (as the current loop does) and `exit(0)`. Keep the periodic profraw dump. Add `LayerManager::hold`.

- [ ] **Step 4: Run tests to verify they pass**

Run: same as step 2.
Expected: PASS.

- [ ] **Step 5: Fix existing tests that relied on a layer staying up**

Run the full suite (`make test` on a host; nested, compare per-file failures against a baseline run of the branch tip before this task). For each new failure caused by the layer shutting down between commands, call `layer.hold()?` after the layer's first command. Don't change any assertion.
Expected: no failures beyond the baseline.

- [ ] **Step 6: Document and commit**

README: in the section on layer lifetime/`stop`, say a layer stops by itself once nothing is running in it, keeps its changes, and restarts when a command runs in it again; `devlayer stop` stops it right away and kills what's running.

```bash
git add src tests README.adoc
git commit -m "Reap orphans in layers and shut layers down when idle"
```
