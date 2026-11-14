mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::path::PathBuf;
use std::process::Command;

/// Writes `contents` to the layer's pid file, as a stale or mistaken pid
/// file would have them
fn write_pid_file(layer: &LayerManager, contents: &str) -> Result<()> {
    let pid_file = format!("{}.pid", layer.dir()?.display());
    std::fs::write(PathBuf::from(pid_file), contents)?;
    Ok(())
}

/// A pid file naming a process outside of any layer, as a stale one can
/// after a reboot, doesn't get a command run in that process's namespaces
#[rstest]
fn ai_test_pid_file_naming_host_process(mut layer: LayerManager) -> Result<()> {
    let mut sleep = Command::new("sleep").arg("30").spawn()?;
    layer.run(&["true"])?;
    layer.run(&["stop"])?;
    write_pid_file(&layer, &sleep.id().to_string())?;

    layer.run(&["readlink", "/proc/self/ns/mnt"])?;
    let host_ns = std::fs::read_link("/proc/self/ns/mnt")?;
    let result = layer.last_stdout.trim() != host_ns.to_string_lossy();
    let sleep_alive = sleep.try_wait()?.is_none();
    sleep.kill()?;
    sleep.wait()?;
    assert!(result, "the command ran outside of a layer");
    assert!(sleep_alive);
    Ok(())
}

/// A pid file naming a layer's PID 1 but recording a different start time
/// names a process that has since exited, so the layer isn't running
#[rstest]
fn ai_test_pid_file_with_wrong_start_time(
    mut layer: LayerManager,
) -> Result<()> {
    let mut other = LayerManager::new();
    other.hold()?;
    let other_pid_file = format!("{}.pid", other.dir()?.display());
    let contents = std::fs::read_to_string(other_pid_file)?;
    let other_pid = contents.split_whitespace().next().unwrap_or_default();

    layer.run(&["true"])?;
    layer.run(&["stop"])?;
    write_pid_file(&layer, &format!("{} 1", other_pid))?;
    layer.run(&["list"])?;
    let stopped = layer.last_stdout.split("Stopped layers:").nth(1);
    assert!(
        stopped.is_some_and(|stopped| stopped.contains(&layer.name)),
        "{}",
        layer.last_stdout
    );
    Ok(())
}
