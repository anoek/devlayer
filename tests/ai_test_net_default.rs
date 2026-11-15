mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

/// With no flag, env var or config, layers use the host network.
#[rstest]
fn ai_test_net_defaults_to_host(mut layer: LayerManager) -> Result<()> {
    layer.run(&["--no-config", "config", "net"])?;
    assert_eq!(layer.last_stdout.trim(), "host");
    assert!(layer.last_stderr.contains("net=host set from default"));
    Ok(())
}

/// `--net=none` still disables networking.
#[rstest]
fn ai_test_net_none_disables(mut layer: LayerManager) -> Result<()> {
    layer.run(&["--no-config", "--net=none", "config", "net"])?;
    assert_eq!(layer.last_stdout.trim(), "none");
    Ok(())
}
