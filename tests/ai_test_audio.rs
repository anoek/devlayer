mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::fs;
use std::path::Path;

/// Host paths that --audio should bind into the layer, limited to the ones
/// that exist on this machine
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

/// Writes a config file with the given contents and returns its path
fn write_config(
    layer: &LayerManager,
    prefix: &str,
    contents: &str,
) -> Result<String> {
    let path = format!("{}.toml", layer.test_filename(prefix));
    fs::write(&path, contents)?;
    Ok(path)
}

#[rstest]
fn ai_test_audio_disabled_by_default(mut layer: LayerManager) -> Result<()> {
    layer.run(&["--no-config", "config", "audio"])?;
    assert_eq!(layer.last_stdout.trim(), "false");

    layer.run(&["--no-config", "config", "bind"])?;
    for path in host_audio_paths() {
        assert!(!layer.last_stdout.contains(&path));
    }

    Ok(())
}

#[rstest]
fn ai_test_audio_flag(mut layer: LayerManager) -> Result<()> {
    layer.run(&["--no-config", "--audio", "config", "audio"])?;
    assert_eq!(layer.last_stdout.trim(), "true");

    layer.run(&["--no-config", "--audio", "config", "bind"])?;
    for path in host_audio_paths() {
        assert!(layer.last_stdout.contains(&path));
    }

    Ok(())
}

#[rstest]
fn ai_test_audio_env(mut layer: LayerManager) -> Result<()> {
    layer.run_with_env(
        &["--no-config", "config", "audio"],
        "DEVLAYER_AUDIO",
        "true",
    )?;
    assert_eq!(layer.last_stdout.trim(), "true");

    layer.run_with_env(
        &["--no-config", "config", "audio"],
        "DEVLAYER_AUDIO",
        "false",
    )?;
    assert_eq!(layer.last_stdout.trim(), "false");

    assert!(layer.exfail(
        &["--no-config", "config", "audio"],
        "DEVLAYER_AUDIO",
        "yes"
    ));
    assert!(
        layer
            .last_stderr
            .contains("Invalid value for DEVLAYER_AUDIO")
    );

    Ok(())
}

#[rstest]
fn ai_test_audio_config_file(mut layer: LayerManager) -> Result<()> {
    let config = write_config(&layer, "audio", "audio = true")?;

    layer.run(&["--config", &config, "config", "audio"])?;
    assert_eq!(layer.last_stdout.trim(), "true");

    // The environment takes precedence over config files
    layer.run_with_env(
        &["--config", &config, "config", "audio"],
        "DEVLAYER_AUDIO",
        "false",
    )?;
    assert_eq!(layer.last_stdout.trim(), "false");

    Ok(())
}

#[rstest]
fn ai_test_audio_with_no_default_binds(mut layer: LayerManager) -> Result<()> {
    layer.run(&[
        "--no-config",
        "--no-default-binds",
        "--audio",
        "config",
        "bind",
    ])?;
    assert!(!layer.last_stdout.contains("/dev/fuse"));
    for path in host_audio_paths() {
        assert!(layer.last_stdout.contains(&path));
    }

    Ok(())
}

#[rstest]
fn ai_test_audio_mounted_in_layer(mut layer: LayerManager) -> Result<()> {
    let paths = host_audio_paths();

    // Without audio devices or sockets, e.g. on CI runners, --audio has
    // nothing to bind, so there's nothing to check
    if paths.is_empty() {
        eprintln!("No audio devices or sockets found, skipping");
        return Ok(());
    }

    for path in &paths {
        assert!(layer.xfail(&["--no-config", "test", "-e", path]));
    }

    // Changing audio for a running layer requires a restart
    assert!(layer.xfail(&["--no-config", "--audio", "true"]));
    layer.run(&["stop"])?;

    for path in &paths {
        assert!(layer.pass(&["--no-config", "--audio", "test", "-e", path]));
    }

    // Parent directories created for the sockets mirror the host's ownership
    // so that clients like pactl will trust XDG_RUNTIME_DIR
    if let Ok(xdg_runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        if Path::new(&xdg_runtime_dir).join("pulse").exists() {
            layer.run(&[
                "--no-config",
                "--audio",
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

    Ok(())
}

#[rstest]
fn ai_test_audio_without_xdg_runtime_dir(
    mut layer: LayerManager,
) -> Result<()> {
    // Without XDG_RUNTIME_DIR we can't find the sockets, but /dev/snd is
    // still bound if it exists
    layer.run_with_env(
        &["--no-config", "--audio", "config", "bind"],
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
