// Tests for error and recovery paths that ordinary usage doesn't reach

mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::fs;
use std::path::PathBuf;

#[rstest]
fn ai_test_bind_source_missing(mut layer: LayerManager) -> Result<()> {
    let missing = format!("/does-not-exist-{}", rid());
    let bind = format!("--bind={}", missing);
    assert!(layer.xfail(&[&bind, "config", "bind"]));
    assert!(
        layer.last_stderr.contains(&format!(
            "Failed to canonicalize source path: {}",
            missing
        )),
        "{}",
        layer.last_stderr
    );
    Ok(())
}

#[rstest]
fn ai_test_config_file_unreadable(mut layer: LayerManager) -> Result<()> {
    // The path exists, but it's a directory so it can't be read
    let dir = layer.test_filename("config-dir");
    fs::create_dir_all(&dir)?;
    assert!(layer.xfail(&["--config", &dir, "config"]));
    assert!(
        layer
            .last_stderr
            .contains(&format!("Failed to read config file {}", dir)),
        "{}",
        layer.last_stderr
    );
    Ok(())
}

#[rstest]
fn ai_test_new_with_unusable_storage_dir(
    mut layer: LayerManager,
) -> Result<()> {
    // --new looks for an unused name in the storage directory, which fails
    // when the storage directory can't be used. --new conflicts with the
    // --name the fixture normally adds.
    layer.no_default_options = true;
    let storage_dir =
        format!("--storage-dir=/proc/devlayer-coverage-{}", rid());
    assert!(layer.xfail(&["--new", &storage_dir, "config", "name"]));
    assert!(
        layer
            .last_stderr
            .contains("Failed to find a suitable storage directory"),
        "{}",
        layer.last_stderr
    );
    Ok(())
}

#[rstest]
fn ai_test_running_layer_without_settings(
    mut layer: LayerManager,
) -> Result<()> {
    layer.run(&["true"])?;

    // Layers started before settings were recorded don't have a
    // settings file, so there is nothing to validate the config against
    layer.run(&["config", "layer_dir"])?;
    let settings =
        PathBuf::from(layer.last_stdout.trim()).join("data/settings.json");
    assert!(settings.exists());
    let removed = std::process::Command::new("sudo")
        .args(["rm", &settings.to_string_lossy()])
        .status()?;
    assert!(removed.success());

    layer.run(&["echo", "still running"])?;
    assert_eq!(layer.last_stdout.trim(), "still running");
    Ok(())
}
