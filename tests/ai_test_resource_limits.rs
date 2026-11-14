mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::fs;
use std::path::PathBuf;

const UNAVAILABLE: &str = "Failed to set up resource limits";

fn available_cpus() -> usize {
    std::thread::available_parallelism().unwrap().get()
}

fn write_config(
    layer: &LayerManager,
    prefix: &str,
    contents: &str,
) -> Result<String> {
    let path = format!("{}.toml", layer.test_filename(prefix));
    fs::write(&path, contents)?;
    Ok(path)
}

/// Starts the layer with a CPU limit to find out if this machine lets us
/// manage cgroups, leaving it running. When it doesn't, e.g. because we're
/// running inside a container or another layer, we check that we fail with a
/// helpful error.
fn limits_available(layer: &mut LayerManager) -> bool {
    if layer.hold_with(&["--cpus=1"]).is_ok() {
        return true;
    }
    assert!(
        layer.last_stderr.contains(UNAVAILABLE),
        "{}",
        layer.last_stderr
    );
    eprintln!("cgroup resource limits are not available, skipping");
    false
}

/// Finds our layer's cgroup on the host, if it has one
fn host_cgroup(layer: &LayerManager) -> Option<PathBuf> {
    let suffix = format!("-{}", layer.name);
    fs::read_dir("/sys/fs/cgroup/devlayer")
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| path.to_string_lossy().ends_with(&suffix))
}

/*
 * Configuration
 */

#[rstest]
fn ai_test_limits_unlimited_by_default(mut layer: LayerManager) -> Result<()> {
    // Skip config files, which may set limits
    layer.run(&["--no-config", "config", "cpus", "memory"])?;
    assert!(layer.last_stdout.contains("cpus=unlimited"));
    assert!(layer.last_stdout.contains("memory=unlimited"));

    // Both keys are part of the full config listing
    layer.run(&["--no-config", "config"])?;
    assert!(layer.last_stdout.contains("cpus=unlimited"));
    assert!(layer.last_stdout.contains("memory=unlimited"));
    Ok(())
}

#[rstest]
fn ai_test_limits_cli(mut layer: LayerManager) -> Result<()> {
    layer.run(&["--cpus=2", "config", "cpus"])?;
    assert_eq!(layer.last_stdout.trim(), "2");

    layer.run(&["--cpus=1.5", "config", "cpus"])?;
    assert_eq!(layer.last_stdout.trim(), "1.5");

    // Negative values are relative to the number of available CPUs, and
    // work with or without an equals sign
    let expected = (available_cpus() - 1).to_string();
    layer.run(&["--cpus=-1", "config", "cpus"])?;
    assert_eq!(layer.last_stdout.trim(), expected);
    layer.run(&["--cpus", "-1", "config", "cpus"])?;
    assert_eq!(layer.last_stdout.trim(), expected);

    layer.run(&["--memory=512M", "config", "memory"])?;
    assert_eq!(layer.last_stdout.trim(), (512u64 << 20).to_string());

    layer.run(&["--memory", "1.5G", "config", "memory"])?;
    assert_eq!(layer.last_stdout.trim(), (3u64 << 29).to_string());
    Ok(())
}

#[rstest]
fn ai_test_limits_env(mut layer: LayerManager) -> Result<()> {
    // Config files are skipped so they can't set limits of their own
    let args = ["--no-config", "config"];
    layer.run_with_env(
        &[&args[..], &["cpus"]].concat(),
        "DEVLAYER_CPUS",
        "3",
    )?;
    assert_eq!(layer.last_stdout.trim(), "3");

    layer.run_with_env(
        &[&args[..], &["memory"]].concat(),
        "DEVLAYER_MEMORY",
        "2G",
    )?;
    assert_eq!(layer.last_stdout.trim(), (2u64 << 30).to_string());

    // Empty values are ignored
    layer.run_with_env(
        &[&args[..], &["cpus"]].concat(),
        "DEVLAYER_CPUS",
        "",
    )?;
    assert_eq!(layer.last_stdout.trim(), "unlimited");

    // The CLI takes precedence over the environment
    layer.run_with_env(
        &["--no-config", "--cpus=1", "config", "cpus"],
        "DEVLAYER_CPUS",
        "3",
    )?;
    assert_eq!(layer.last_stdout.trim(), "1");
    Ok(())
}

#[rstest]
fn ai_test_limits_config_file(mut layer: LayerManager) -> Result<()> {
    let config =
        write_config(&layer, "limits", "cpus = -1\nmemory = \"4G\"\n")?;
    layer.run(&["--config", &config, "config", "cpus", "memory"])?;
    assert!(
        layer
            .last_stdout
            .contains(&format!("cpus={}", available_cpus() - 1))
    );
    assert!(
        layer
            .last_stdout
            .contains(&format!("memory={}", 4u64 << 30))
    );

    // Numbers and strings are both accepted
    let config = write_config(
        &layer,
        "limits-types",
        "cpus = \"0.5\"\nmemory = 1073741824\n",
    )?;
    layer.run(&["--config", &config, "config", "cpus", "memory"])?;
    assert!(layer.last_stdout.contains("cpus=0.5"));
    assert!(layer.last_stdout.contains("memory=1073741824"));

    // The environment takes precedence over config files
    layer.run_with_env(
        &["--config", &config, "config", "cpus"],
        "DEVLAYER_CPUS",
        "2",
    )?;
    assert_eq!(layer.last_stdout.trim(), "2");
    Ok(())
}

#[rstest]
fn ai_test_limits_invalid(mut layer: LayerManager) -> Result<()> {
    for cpus in ["--cpus=0", "--cpus=abc", "--cpus=0.001"] {
        assert!(layer.xfail(&[cpus, "config", "cpus"]));
        assert!(
            layer.last_stderr.contains("Invalid cpus value"),
            "{}",
            layer.last_stderr
        );
    }

    let all_cpus = format!("--cpus=-{}", available_cpus());
    assert!(layer.xfail(&[&all_cpus, "config", "cpus"]));
    assert!(layer.last_stderr.contains("leaves 0 of the"));

    assert!(layer.xfail(&["--memory=lots", "config", "memory"]));
    assert!(layer.last_stderr.contains("Invalid memory value 'lots'"));

    let config = write_config(&layer, "bad-type", "cpus = true\n")?;
    assert!(layer.xfail(&["--config", &config, "config", "cpus"]));
    Ok(())
}

/*
 * Enforcement
 */

#[rstest]
fn ai_test_limits_applied(mut layer: LayerManager) -> Result<()> {
    if !limits_available(&mut layer) {
        return Ok(());
    }
    layer.run(&["stop"])?;

    let args = ["--cpus=2", "--memory=256M"];
    let run = |layer: &mut LayerManager, command: &[&str]| {
        let mut full = args.to_vec();
        full.extend_from_slice(command);
        layer
            .run(&full)
            .map(|_| layer.last_stdout.trim().to_string())
    };

    // Our limits are visible inside the layer, whose cgroup namespace is
    // rooted at the layer's cgroup
    assert_eq!(
        run(&mut layer, &["cat", "/sys/fs/cgroup/cpu.max"])?,
        "200000 100000"
    );
    assert_eq!(
        run(&mut layer, &["cat", "/sys/fs/cgroup/memory.max"])?,
        (256u64 << 20).to_string()
    );
    assert_eq!(run(&mut layer, &["cat", "/proc/self/cgroup"])?, "0::/");

    // Processes that join the running layer are in its cgroup too
    layer.hold_with(&args)?;
    let cgroup = host_cgroup(&layer).expect("layer cgroup should exist");
    let mut child =
        layer.run_in_background(&[args[0], args[1], "sleep", "30"])?;
    let mut joined = false;
    for _ in 0..100 {
        let procs = fs::read_to_string(cgroup.join("cgroup.procs"))?;
        if procs.lines().count() > 1 {
            joined = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(joined, "sleep should have joined the layer cgroup");

    // Stopping the layer removes its cgroup
    layer.run(&["stop"])?;
    let _ = child.wait();
    assert!(!cgroup.exists());
    Ok(())
}

#[rstest]
fn ai_test_limits_negative_cpus(mut layer: LayerManager) -> Result<()> {
    if !limits_available(&mut layer) {
        return Ok(());
    }
    layer.run(&["stop"])?;

    layer.run(&["--cpus=-1", "cat", "/sys/fs/cgroup/cpu.max"])?;
    assert_eq!(
        layer.last_stdout.trim(),
        format!("{} 100000", (available_cpus() - 1) * 100000)
    );
    Ok(())
}

#[rstest]
fn ai_test_limits_memory_enforced(mut layer: LayerManager) -> Result<()> {
    if !limits_available(&mut layer) {
        return Ok(());
    }
    layer.run(&["stop"])?;

    // tail has to buffer the whole newline-free input, which won't fit.
    // The devlayer command waiting on the shell is in the cgroup too, so tail
    // raises its own oom_score_adj to always be the OOM killer's first
    // choice; otherwise the kernel may kill devlayer instead and we get no
    // exit status. memory.events is read in the same command so we see this
    // cgroup's counters.
    layer.run(&[
        "--memory=32M",
        "sh",
        "-c",
        "head -c 256M /dev/zero \
         | sh -c 'echo 1000 > /proc/self/oom_score_adj && exec tail' \
         > /dev/null; \
         echo \"tail exited with $?\"; \
         cat /sys/fs/cgroup/memory.events",
    ])?;
    let output = layer.last_stdout.clone();
    assert!(output.contains("tail exited with 137"), "{}", output);
    let oom_kills: u64 = output
        .lines()
        .find_map(|line| line.strip_prefix("oom_kill "))
        .expect("memory.events should have an oom_kill entry")
        .parse()?;
    assert!(oom_kills >= 1, "{}", output);
    Ok(())
}

#[rstest]
fn ai_test_limits_changed(mut layer: LayerManager) -> Result<()> {
    if !limits_available(&mut layer) {
        return Ok(());
    }

    // Restart without config files, which may set limits of their own
    layer.run(&["stop"])?;
    layer.hold_with(&["--no-config", "--cpus=1"])?;

    // The layer is running with --cpus=1, so other limits are rejected
    assert!(layer.xfail(&["--no-config", "true"]));
    assert!(layer.last_stderr.contains("Resource limits have changed"));
    assert!(
        layer
            .last_stderr
            .contains("Existing: cpus=1 memory=unlimited")
    );
    assert!(
        layer
            .last_stderr
            .contains("Current: cpus=unlimited memory=unlimited")
    );

    assert!(layer.xfail(&["--no-config", "--cpus=1", "--memory=1G", "true"]));
    assert!(
        layer
            .last_stderr
            .contains(&format!("Current: cpus=1 memory={} bytes", 1u64 << 30))
    );

    // The same limits are fine, and stopping lets us change them
    layer.run(&["--no-config", "--cpus=1", "true"])?;
    layer.run(&["stop"])?;
    layer.run(&["--no-config", "--cpus=2", "true"])?;
    Ok(())
}

#[rstest]
fn ai_test_limits_stale_cgroup(mut layer: LayerManager) -> Result<()> {
    if !limits_available(&mut layer) {
        return Ok(());
    }

    // Kill the layer and its supervisor without cleaning up, leaving an
    // empty cgroup behind
    let cgroup = host_cgroup(&layer).expect("layer cgroup should exist");
    layer.run(&["config", "layer_dir"])?;
    let pid_file = format!("{}.pid", layer.last_stdout.trim());
    let pid = fs::read_to_string(&pid_file)?;
    let pid = pid.split_whitespace().next().unwrap_or_default();
    let status = fs::read_to_string(format!("/proc/{}/status", pid))?;
    let supervisor = status
        .lines()
        .find_map(|line| line.strip_prefix("PPid:"))
        .unwrap_or_default()
        .trim()
        .to_string();
    std::process::Command::new("sudo")
        .args(["kill", "-9", &supervisor, pid.trim()])
        .status()?;
    for _ in 0..100 {
        if fs::read_to_string(cgroup.join("cgroup.procs"))?.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(cgroup.exists());

    // Starting again replaces the stale cgroup
    layer.run(&["--cpus=2", "cat", "/sys/fs/cgroup/cpu.max"])?;
    assert_eq!(layer.last_stdout.trim(), "200000 100000");
    Ok(())
}

#[rstest]
fn ai_test_limits_cgroup_in_use(mut layer: LayerManager) -> Result<()> {
    if !limits_available(&mut layer) {
        return Ok(());
    }

    // A layer with the same name in another storage directory would
    // need the same cgroup, which is in use
    let storage_dir =
        std::env::current_dir()?.join(layer.test_filename("storage"));
    fs::create_dir_all(&storage_dir)?;
    let storage_arg = format!("--storage-dir={}", storage_dir.display());
    assert!(layer.xfail(&[&storage_arg, "--cpus=1", "true"]));
    assert!(
        layer
            .last_stderr
            .contains("from a different storage directory")
    );
    Ok(())
}

#[rstest]
fn ai_test_limits_setup_failures(mut layer: LayerManager) -> Result<()> {
    assert!(layer.exfail(&["--cpus=1", "true"], "TEST_NO_CGROUP2", "1"));
    assert!(layer.last_stderr.contains(UNAVAILABLE));
    assert!(
        layer
            .last_stderr
            .contains("requires cgroup v2, which is not mounted")
    );

    if !limits_available(&mut layer) {
        return Ok(());
    }
    layer.run(&["stop"])?;

    // Failing to apply limits cleans up the cgroup
    for (file, args) in [
        ("cpu.max", vec!["--cpus=1", "true"]),
        ("memory.max", vec!["--memory=1G", "true"]),
    ] {
        assert!(layer.exfail(&args, "TEST_CGROUP_WRITE_FAILURE", file));
        assert!(layer.last_stderr.contains(UNAVAILABLE));
        assert!(layer.last_stderr.contains("Failed to write"));
        assert!(host_cgroup(&layer).is_none());
    }

    // As does failing to start the layer
    assert!(layer.exfail(
        &["--cpus=1", "true"],
        "TEST_START_LAYER_FAILURE",
        "1"
    ));
    assert!(layer.last_stderr.contains("Failed to start layer"));
    assert!(host_cgroup(&layer).is_none());
    Ok(())
}

#[rstest]
fn ai_test_limits_new_parent_cgroup(mut layer: LayerManager) -> Result<()> {
    if !limits_available(&mut layer) {
        return Ok(());
    }
    layer.run(&["stop"])?;

    // The first layer with limits creates the parent cgroup and enables
    // the controllers we need in it
    let parent = format!("{}-parent", layer.name);
    let parent_path = PathBuf::from("/sys/fs/cgroup").join(&parent);
    assert!(!parent_path.exists());
    layer.run_with_env(
        &["--cpus=1", "--memory=64M", "cat", "/sys/fs/cgroup/cpu.max"],
        "TEST_CGROUP_PARENT",
        &parent,
    )?;
    assert_eq!(layer.last_stdout.trim(), "100000 100000");
    let enabled =
        fs::read_to_string(parent_path.join("cgroup.subtree_control"))?;
    assert!(enabled.split_whitespace().any(|c| c == "cpu"));
    assert!(enabled.split_whitespace().any(|c| c == "memory"));

    layer.run_with_env(&["stop"], "TEST_CGROUP_PARENT", &parent)?;
    assert_eq!(
        fs::read_dir(&parent_path)?
            .filter(|e| { e.as_ref().is_ok_and(|e| e.path().is_dir()) })
            .count(),
        0
    );
    std::process::Command::new("sudo")
        .args(["rmdir", &parent_path.to_string_lossy()])
        .status()?;
    Ok(())
}

#[rstest]
fn ai_test_limits_no_swap_accounting(mut layer: LayerManager) -> Result<()> {
    if !limits_available(&mut layer) {
        return Ok(());
    }
    layer.run(&["stop"])?;

    // Only memory is limited (config files are skipped since they may limit
    // cpus too). Without swap accounting we can limit memory but not swap.
    // The layer is left running, so the commands after the first one join
    // the layer started without swap accounting
    let memory_only = ["--no-config", "--memory=64M", "cat"];
    layer.run_with_env(
        &[
            "--no-config",
            "--memory=64M",
            "sh",
            "-c",
            "cat /sys/fs/cgroup/memory.max; sleep 3600 >/dev/null 2>&1 &",
        ],
        "TEST_NO_SWAP_ACCOUNTING",
        "1",
    )?;
    assert_eq!(layer.last_stdout.trim(), (64u64 << 20).to_string());
    layer.run(&[&memory_only[..], &["/sys/fs/cgroup/cpu.max"]].concat())?;
    assert_eq!(layer.last_stdout.trim(), "max 100000");
    layer.run(
        &[&memory_only[..], &["/sys/fs/cgroup/memory.swap.max"]].concat(),
    )?;
    assert_eq!(layer.last_stdout.trim(), "max");
    layer.run(&["stop"])?;

    // Normally swap is disabled
    layer.run(
        &[&memory_only[..], &["/sys/fs/cgroup/memory.swap.max"]].concat(),
    )?;
    assert_eq!(layer.last_stdout.trim(), "0");
    Ok(())
}

#[rstest]
fn ai_test_limits_join_failure(mut layer: LayerManager) -> Result<()> {
    if !limits_available(&mut layer) {
        return Ok(());
    }

    // The layer is running with --cpus=1, joining it requires moving
    // into its cgroup
    assert!(layer.exfail(
        &["--cpus=1", "true"],
        "TEST_CGROUP_WRITE_FAILURE",
        "cgroup.procs"
    ));
    assert!(layer.last_stderr.contains("Failed to write"));
    assert!(layer.last_stderr.contains("cgroup.procs"));
    Ok(())
}

#[rstest]
fn ai_test_limits_stop_cgroup_busy(mut layer: LayerManager) -> Result<()> {
    if !limits_available(&mut layer) {
        return Ok(());
    }

    // A child cgroup keeps the layer's cgroup from being removed, so stop
    // retries for a while and then gives up without failing
    let cgroup = host_cgroup(&layer).expect("layer cgroup should exist");
    let child = cgroup.join("busy");
    let sudo =
        |args: &[&str]| std::process::Command::new("sudo").args(args).status();
    sudo(&["mkdir", &child.to_string_lossy()])?;

    let started = std::time::Instant::now();
    layer.run(&["stop"])?;
    assert!(started.elapsed() >= std::time::Duration::from_secs(5));
    assert!(cgroup.exists());

    sudo(&["rmdir", &child.to_string_lossy()])?;
    sudo(&["rmdir", &cgroup.to_string_lossy()])?;
    assert!(!cgroup.exists());
    Ok(())
}

#[rstest]
fn ai_test_limits_unlimited_override(mut layer: LayerManager) -> Result<()> {
    // "unlimited" removes a limit set by a less specific source
    let config =
        write_config(&layer, "limited", "cpus = 2\nmemory = \"4G\"\n")?;
    layer.run(&[
        "--config",
        &config,
        "--cpus=unlimited",
        "--memory=Unlimited",
        "config",
        "cpus",
        "memory",
    ])?;
    assert!(layer.last_stdout.contains("cpus=unlimited"));
    assert!(layer.last_stdout.contains("memory=unlimited"));

    layer.run_with_env(
        &["--config", &config, "config", "cpus"],
        "DEVLAYER_CPUS",
        "unlimited",
    )?;
    assert_eq!(layer.last_stdout.trim(), "unlimited");

    let unlimited =
        write_config(&layer, "unlimited", "memory = \"unlimited\"\n")?;
    layer.run(&["--config", &unlimited, "config", "memory"])?;
    assert_eq!(layer.last_stdout.trim(), "unlimited");

    // Without any limits the layer doesn't get a cgroup
    if limits_available(&mut layer) {
        layer.run(&["stop"])?;
        layer.run(&[
            "--config",
            &config,
            "--cpus=unlimited",
            "--memory=unlimited",
            "true",
        ])?;
        assert!(host_cgroup(&layer).is_none());
    }
    Ok(())
}

#[rstest]
fn ai_test_limits_nested(mut layer: LayerManager) -> Result<()> {
    if !limits_available(&mut layer) {
        return Ok(());
    }
    layer.run(&["stop"])?;

    // A layer inside another one can't manage cgroups, so instead of its
    // own limits it shares the limits of the layer it's running in
    let inner = get_devlayer_bin() + "-setuid";
    layer.run(&[
        "--no-config",
        "--cpus=2",
        &inner,
        "--no-config",
        "--cpus=1",
        "cat",
        "/sys/fs/cgroup/cpu.max",
    ])?;
    assert_eq!(layer.last_stdout.trim(), "200000 100000");
    assert!(
        layer.last_stderr.contains(
            "shares the limits of the layer it's running in (cpus=2 memory=unlimited)"
        ),
        "{}",
        layer.last_stderr
    );

    // The warning makes it clear when that means no limits at all
    layer.run(&["stop"])?;
    layer.run(&["--no-config", &inner, "--no-config", "--cpus=1", "true"])?;
    assert!(
        layer.last_stderr.contains(
            "shares the limits of the layer it's running in (cpus=unlimited memory=unlimited)"
        ),
        "{}",
        layer.last_stderr
    );
    Ok(())
}

#[rstest]
fn ai_test_limits_delete_removes_cgroup(mut layer: LayerManager) -> Result<()> {
    if !limits_available(&mut layer) {
        return Ok(());
    }

    // delete stops layers without knowing which user started them, so
    // it has to find the cgroup from the running layer
    let cgroup = host_cgroup(&layer).expect("layer cgroup should exist");
    let name = layer.name.clone();
    layer.run(&["delete", "-y", &name])?;
    assert!(!cgroup.exists());
    Ok(())
}
