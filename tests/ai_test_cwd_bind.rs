mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::path::{Path, PathBuf};

/// Creates an absolute project directory for this test and returns it
fn project_dir(layer: &LayerManager) -> Result<PathBuf> {
    let dir = std::env::current_dir()?.join(layer.test_filename("project"));
    std::fs::create_dir_all(dir.join("sub"))?;
    Ok(dir)
}

/// A layer manager that runs from `dir` and leaves the cwd bind at its
/// default (on)
fn layer_in(dir: &Path) -> LayerManager {
    let mut layer = LayerManager::new();
    layer.set_bind_cwd(true);
    layer.cwd = Some(dir.to_path_buf());
    layer
}

#[rstest]
fn ai_test_cwd_bound_by_default(mut layer: LayerManager) -> Result<()> {
    let dir = project_dir(&layer)?;
    layer.set_bind_cwd(true);
    layer.cwd = Some(dir.clone());

    layer.run(&["touch", "cwd-bound-file"])?;

    // The write went straight to the host, so it is not a layer change
    assert!(dir.join("cwd-bound-file").exists());
    layer.run(&["status", "/"])?;
    assert!(!layer.last_stdout.contains("cwd-bound-file"));
    Ok(())
}

#[rstest]
fn ai_test_no_bind_cwd_flag_env_config(layer: LayerManager) -> Result<()> {
    let dir = project_dir(&layer)?;

    let mut by_flag = layer_in(&dir);
    by_flag.run(&["--no-bind-cwd", "touch", "by-flag"])?;
    assert!(!dir.join("by-flag").exists());

    let mut by_env = layer_in(&dir);
    by_env.run_with_env(
        &["touch", "by-env"],
        "DEVLAYER_NO_BIND_CWD",
        "true",
    )?;
    assert!(!dir.join("by-env").exists());

    let mut by_config = layer_in(&dir);
    let config = dir.join("no-bind-cwd.toml");
    std::fs::write(&config, "no_bind_cwd = true\n")?;
    let config_arg = format!("--config={}", config.display());
    by_config.run(&[&config_arg, "touch", "by-config"])?;
    assert!(!dir.join("by-config").exists());
    Ok(())
}

#[rstest]
fn ai_test_refuse_root(mut layer: LayerManager) -> Result<()> {
    layer.set_bind_cwd(true);
    layer.cwd = Some(PathBuf::from("/"));

    assert!(layer.xfail(&["true"]));
    assert!(layer.last_stderr.contains(
        "Refusing to bind / into the layer; run from a project directory or pass --no-bind-cwd"
    ));

    assert!(layer.pass(&["--no-bind-cwd", "true"]));

    // A layer created in / is an ancestor of every directory, so it would be
    // picked by --last everywhere; delete it right away
    layer.run(&["delete", "-y"])?;
    Ok(())
}

#[rstest]
fn ai_test_home_allowed(mut layer: LayerManager) -> Result<()> {
    let home = PathBuf::from(std::env::var("HOME")?);
    layer.set_bind_cwd(true);
    layer.cwd = Some(home.clone());

    layer.run(&["config", "bind_cwd"])?;
    assert_eq!(layer.last_stdout.trim(), "true");
    layer.run(&["config", "created_in"])?;
    assert_eq!(
        layer.last_stdout.trim(),
        home.canonicalize()?.display().to_string()
    );
    Ok(())
}

#[rstest]
fn ai_test_cwd_bind_reenter_from_subdir(mut layer: LayerManager) -> Result<()> {
    let dir = project_dir(&layer)?;
    layer.set_bind_cwd(true);
    layer.cwd = Some(dir.clone());
    layer.run(&["true"])?;

    // Entering the running layer from a subdirectory keeps the original bind
    layer.cwd = Some(dir.join("sub"));
    layer.run(&["config", "created_in"])?;
    assert_eq!(layer.last_stdout.trim(), dir.display().to_string());
    layer.run(&["touch", "from-sub"])?;
    assert!(dir.join("sub/from-sub").exists());
    Ok(())
}

#[rstest]
fn ai_test_restart_keeps_origin(mut layer: LayerManager) -> Result<()> {
    let dir = project_dir(&layer)?;
    layer.set_bind_cwd(true);
    layer.cwd = Some(dir.clone());
    layer.run(&["true"])?;
    layer.run(&["stop"])?;

    layer.cwd = Some(dir.join("sub"));
    layer.run(&["true"])?;
    layer.run(&["config", "created_in"])?;
    assert_eq!(layer.last_stdout.trim(), dir.display().to_string());
    layer.run(&["config", "bind_cwd"])?;
    assert_eq!(layer.last_stdout.trim(), "true");
    Ok(())
}

#[rstest]
fn ai_test_no_bind_cwd_persists(mut layer: LayerManager) -> Result<()> {
    let dir = project_dir(&layer)?;
    layer.set_bind_cwd(true);
    layer.cwd = Some(dir.clone());
    layer.run(&["--no-bind-cwd", "true"])?;

    // Re-entering without the flag keeps the layer's original choice
    layer.run(&["config", "bind_cwd"])?;
    assert_eq!(layer.last_stdout.trim(), "false");
    layer.run(&["touch", "still-layered"])?;
    assert!(!dir.join("still-layered").exists());
    Ok(())
}
