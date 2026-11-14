mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::path::PathBuf;

/// A layer manager that runs from the home directory with the cwd bind on,
/// as a user starting devlayer from `~` would
fn from_home(mut layer: LayerManager) -> Result<LayerManager> {
    layer.set_bind_cwd(true);
    layer.cwd = Some(PathBuf::from(std::env::var("HOME")?));
    Ok(layer)
}

/// Starting a layer from the home directory doesn't bind all of it
#[rstest]
fn ai_test_home_not_bound_by_default(layer: LayerManager) -> Result<()> {
    let mut layer = from_home(layer)?;
    assert!(layer.xfail(&["true"]));
    assert!(
        layer
            .last_stderr
            .contains("Refusing to bind your home directory"),
        "{}",
        layer.last_stderr
    );
    Ok(())
}

/// Asking for the home directory with --bind=. binds it anyway
#[rstest]
fn ai_test_home_bound_when_asked(layer: LayerManager) -> Result<()> {
    let mut layer = from_home(layer)?;
    // Lists the mounts in the layer at the home directory
    layer.run(&[
        "--bind=.",
        "sh",
        "-c",
        "awk -v home=\"$HOME\" '$5 == home' /proc/self/mountinfo",
    ])?;
    assert!(!layer.last_stdout.trim().is_empty(), "home isn't bound");
    Ok(())
}

/// Without the cwd bind, starting a layer from the home directory is fine
#[rstest]
fn ai_test_home_without_cwd_bind(layer: LayerManager) -> Result<()> {
    let mut layer = from_home(layer)?;
    assert!(layer.pass(&["--no-bind-cwd", "true"]));
    Ok(())
}
