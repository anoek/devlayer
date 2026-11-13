mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::path::Path;

/// `--join` creates the named layer if needed, and later runs with the same
/// name reuse it.
#[rstest]
fn ai_test_join_creates_then_reuses(mut layer: LayerManager) -> Result<()> {
    let join = format!("--join={}", layer.name);
    let file = layer.test_filename("joined");

    layer.run(&[&join, "touch", &file])?;
    layer.run(&[&join, "test", "-f", &file])?;

    // The file only exists in the layer, not on the host
    assert!(!Path::new(&file).exists());
    Ok(())
}

/// `--name` no longer exists.
#[rstest]
fn ai_test_name_flag_removed(mut layer: LayerManager) -> Result<()> {
    layer.no_default_options = true;
    let name = format!("--name={}-name", layer.name);
    assert!(layer.xfail(&[&name, "config", "name"]));
    assert!(layer.last_stderr.contains("--name"));
    Ok(())
}

/// DEVLAYER_JOIN and the `join` config key select the layer like --join.
#[rstest]
fn ai_test_join_env_and_config(mut layer: LayerManager) -> Result<()> {
    layer.no_default_options = true;
    let name = layer.name.clone();

    layer.run_with_env(&["config", "name"], "DEVLAYER_JOIN", &name)?;
    assert_eq!(layer.last_stdout.trim(), name);

    let config = layer.test_filename("join.toml");
    std::fs::write(&config, format!("join = \"{}\"\n", name))?;
    let config_arg = format!("--config={}", config);
    layer.run(&[&config_arg, "config", "name"])?;
    assert_eq!(layer.last_stdout.trim(), name);
    Ok(())
}

/// --join cannot be combined with --new or --last, from the CLI or env.
#[rstest]
fn ai_test_join_conflicts_with_new_last(mut layer: LayerManager) -> Result<()> {
    layer.no_default_options = true;
    let join = format!("--join={}", layer.name);

    assert!(layer.xfail(&[&join, "--new", "config", "name"]));
    assert!(layer.xfail(&[&join, "--last", "config", "name"]));

    assert!(layer.exfail(&["--new", "config", "name"], "DEVLAYER_JOIN", "x"));
    assert!(layer.last_stderr.contains("Cannot use --new or --last"));
    Ok(())
}
