mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::fs;
use std::path::{Path, PathBuf};

/// Host audio paths that are bound into the layer by default, limited to the
/// ones that exist on this machine
fn host_audio_paths() -> Vec<String> {
    let mut paths = vec![];
    if Path::new("/dev/snd").exists() {
        paths.push("/dev/snd".to_string());
    }
    if let Ok(xdg_runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        for name in ["pipewire-0", "pulse"] {
            let path = Path::new(&xdg_runtime_dir).join(name);
            if path.exists() {
                paths.push(path.to_string_lossy().to_string());
            }
        }
    }
    paths
}

/// Creates a fake XDG_RUNTIME_DIR containing a `pulse` directory and returns
/// its absolute path
fn fake_runtime_dir(layer: &LayerManager) -> Result<PathBuf> {
    let dir = std::env::current_dir()?.join(layer.test_filename("xdg"));
    fs::create_dir_all(dir.join("pulse"))?;
    Ok(dir)
}

#[rstest]
fn ai_test_audio_on_by_default(mut layer: LayerManager) -> Result<()> {
    let xdg = fake_runtime_dir(&layer)?;
    layer.run_with_env(
        &["--no-config", "config", "bind"],
        "XDG_RUNTIME_DIR",
        xdg.to_str().unwrap_or_default(),
    )?;
    assert!(
        layer
            .last_stdout
            .contains(&format!("{}/pulse", xdg.display()))
    );
    Ok(())
}

#[rstest]
fn ai_test_audio_off_with_no_default_binds(
    mut layer: LayerManager,
) -> Result<()> {
    let xdg = fake_runtime_dir(&layer)?;
    layer.run_with_env(
        &["--no-config", "--no-default-binds", "config", "bind"],
        "XDG_RUNTIME_DIR",
        xdg.to_str().unwrap_or_default(),
    )?;
    assert!(!layer.last_stdout.contains("/pulse"));
    assert!(!layer.last_stdout.contains("/dev/snd"));
    Ok(())
}

#[rstest]
fn ai_test_audio_flag_removed(mut layer: LayerManager) -> Result<()> {
    assert!(layer.xfail(&["--no-config", "--audio", "config"]));
    assert!(layer.xfail(&["--no-config", "config", "audio"]));
    assert!(layer.last_stderr.contains("Unknown key: audio"));
    Ok(())
}

#[rstest]
fn ai_test_audio_mounted_in_layer(mut layer: LayerManager) -> Result<()> {
    let paths = host_audio_paths();

    // Without audio devices or sockets, e.g. on CI runners, there's nothing
    // to bind, so there's nothing to check
    if paths.is_empty() {
        eprintln!("No audio devices or sockets found, skipping");
        return Ok(());
    }

    for path in &paths {
        assert!(layer.pass(&["--no-config", "test", "-e", path]));
    }

    // Parent directories created for the sockets mirror the host's ownership
    // so that clients like pactl will trust XDG_RUNTIME_DIR
    if let Ok(xdg_runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        if Path::new(&xdg_runtime_dir).join("pulse").exists() {
            layer.run(&[
                "--no-config",
                "stat",
                "-c",
                "%u %a",
                &xdg_runtime_dir,
            ])?;
            let host = fs::metadata(&xdg_runtime_dir)?;
            use std::os::unix::fs::MetadataExt;
            assert_eq!(
                layer.last_stdout.trim(),
                format!("{} {:o}", host.uid(), host.mode() & 0o7777)
            );
        }
    }

    // Changing the default binds of a running layer requires a restart
    layer.run(&["stop"])?;
    for path in &paths {
        assert!(layer.xfail(&[
            "--no-config",
            "--no-default-binds",
            "test",
            "-e",
            path
        ]));
    }

    Ok(())
}

#[rstest]
fn ai_test_audio_without_xdg_runtime_dir(
    mut layer: LayerManager,
) -> Result<()> {
    // Without XDG_RUNTIME_DIR we can't find the sockets, but /dev/snd is
    // still bound if it exists
    layer.run_with_env(
        &["--no-config", "config", "bind"],
        "XDG_RUNTIME_DIR",
        "",
    )?;
    assert!(!layer.last_stdout.contains("pipewire-0"));
    assert!(!layer.last_stdout.contains("/pulse"));
    assert_eq!(
        layer.last_stdout.contains("/dev/snd"),
        Path::new("/dev/snd").exists()
    );

    Ok(())
}
