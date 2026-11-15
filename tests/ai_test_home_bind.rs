mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::path::Path;

/// A layer manager that runs from the home directory with the cwd bind on,
/// as a user starting devlayer from `~` would
fn from_home(mut layer: LayerManager) -> Result<LayerManager> {
    layer.bind_cwd_in(Path::new(&std::env::var("HOME")?));
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

/// `config`, which doesn't start a layer, reports that a layer created in
/// the home directory would bind it
#[rstest]
fn ai_test_home_config_reports_cwd_bind(layer: LayerManager) -> Result<()> {
    let mut layer = from_home(layer)?;
    layer.run(&["config", "bind_cwd"])?;
    assert_eq!(layer.last_stdout.trim(), "true");
    layer.run(&["config", "created_in"])?;
    let home = Path::new(&std::env::var("HOME")?).canonicalize()?;
    assert_eq!(layer.last_stdout.trim(), home.display().to_string());
    Ok(())
}
