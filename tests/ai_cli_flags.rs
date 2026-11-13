mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_flags_before_action(mut layer: LayerManager) -> Result<()> {
    // Disable default -v flag to avoid conflicts
    layer.no_default_options = true;
    let name = layer.name.clone();

    // Test flags before action - traditional style
    assert!(layer.pass(&["--name", &name, "config"]));
    assert!(
        layer
            .last_stdout
            .contains(format!("name={}", name).as_str())
    );

    assert!(layer.pass(&["--name", &name, "--net=host", "config"]));
    assert!(layer.last_stdout.contains("net=host"));

    assert!(layer.pass(&["--name", &name, "-v", "config"]));
    assert!(layer.last_stdout.contains("log_level=TRACE"));

    assert!(layer.pass(&["--name", &name, "--log-level=debug", "config"]));
    assert!(layer.last_stdout.contains("log_level=DEBUG"));

    assert!(layer.pass(&["--name", &name, "--json", "config"]));
    // JSON output should be valid JSON
    assert!(
        serde_json::from_str::<serde_json::Value>(&layer.last_stdout).is_ok()
    );

    // Test a simple action without extra flags
    assert!(layer.pass(&["--name", &name, "status"]));

    Ok(())
}

#[rstest]
fn test_flags_after_action(mut layer: LayerManager) -> Result<()> {
    // Disable default -v flag to avoid conflicts
    layer.no_default_options = true;
    let name = layer.name.clone();

    // Test flags after action - new flexible style
    assert!(layer.pass(&["config", "--name", "test-after"]));
    assert!(layer.last_stdout.contains("name=test-after"));

    assert!(layer.pass(&["--name", &name, "config", "--net=host"]));
    assert!(layer.last_stdout.contains("net=host"));

    assert!(layer.pass(&["--name", &name, "config", "-v"]));
    assert!(layer.last_stdout.contains("log_level=TRACE"));

    assert!(layer.pass(&["--name", &name, "config", "--log-level=debug"]));
    assert!(layer.last_stdout.contains("log_level=DEBUG"));

    assert!(layer.pass(&["--name", &name, "config", "--json"]));
    // JSON output should be valid JSON
    assert!(
        serde_json::from_str::<serde_json::Value>(&layer.last_stdout).is_ok()
    );

    // Test a simple action without extra flags
    assert!(layer.pass(&["--name", &name, "status"]));

    Ok(())
}

#[rstest]
fn test_flags_mixed_position(mut layer: LayerManager) -> Result<()> {
    // Test mixing flags before and after action
    assert!(layer.pass(&["--name", "test-mixed", "config", "--net=host"]));
    assert!(layer.last_stdout.contains("name=test-mixed"));
    assert!(layer.last_stdout.contains("net=host"));

    assert!(layer.pass(&["-v", "config", "--net=none"]));
    assert!(layer.last_stdout.contains("log_level=TRACE"));
    assert!(layer.last_stdout.contains("net=none"));

    assert!(layer.pass(&["--json", "config", "--name=json-test", "-v"]));
    let json: serde_json::Value = serde_json::from_str(&layer.last_stdout)?;
    assert_eq!(json["name"], "json-test");
    assert_eq!(json["log_level"], "TRACE");

    Ok(())
}

#[rstest]
fn test_flags_with_complex_actions(mut layer: LayerManager) -> Result<()> {
    // Create a test file to test actions
    layer.run(&["sh", "-c", "echo test > testfile.txt"])?;

    // Test flags with various actions that take arguments
    assert!(layer.pass(&["status", "testfile.txt", "--json"]));
    assert!(
        serde_json::from_str::<serde_json::Value>(&layer.last_stdout).is_ok()
    );

    assert!(layer.pass(&["--json", "status", "testfile.txt"]));
    assert!(
        serde_json::from_str::<serde_json::Value>(&layer.last_stdout).is_ok()
    );

    // Test with list action instead of diff (diff doesn't support JSON)
    assert!(layer.pass(&["list", "--json"]));
    assert!(
        serde_json::from_str::<serde_json::Value>(&layer.last_stdout).is_ok()
    );

    assert!(layer.pass(&["--json", "list"]));
    assert!(
        serde_json::from_str::<serde_json::Value>(&layer.last_stdout).is_ok()
    );

    Ok(())
}

#[rstest]
fn test_flag_errors_in_different_positions(
    mut layer: LayerManager,
) -> Result<()> {
    // Test invalid flag values in different positions
    assert!(layer.xfail(&["--net=invalid", "config"]));
    assert!(layer.xfail(&["config", "--net=invalid"]));
    assert!(layer.xfail(&["--bind-fuse=invalid", "config"]));
    assert!(layer.xfail(&["config", "--bind-fuse=invalid"]));

    // Test unknown flags in different positions
    assert!(layer.xfail(&["--unknown-flag", "config"]));
    assert!(layer.xfail(&["config", "--unknown-flag"]));

    Ok(())
}

#[rstest]
fn test_boolean_flags_positioning(mut layer: LayerManager) -> Result<()> {
    // Disable default ignored flag to avoid conflicts
    layer.set_ignored(false);

    // Test boolean flags without values in different positions
    assert!(layer.pass(&["--ignored", "config"]));
    assert!(layer.last_stdout.contains("ignored=true"));

    layer.set_ignored(false);
    assert!(layer.pass(&["config", "--ignored"]));
    assert!(layer.last_stdout.contains("ignored=true"));

    layer.set_ignored(false);
    assert!(layer.pass(&["--json", "config", "--ignored"]));
    let json: serde_json::Value = serde_json::from_str(&layer.last_stdout)?;
    assert_eq!(json["ignored"], "true");

    Ok(())
}

#[rstest]
fn test_short_flags_positioning(mut layer: LayerManager) -> Result<()> {
    // Test short flags in different positions
    assert!(layer.pass(&["-v", "config"]));
    assert!(layer.last_stdout.contains("log_level=TRACE"));

    assert!(layer.pass(&["config", "-v"]));
    assert!(layer.last_stdout.contains("log_level=TRACE"));

    // Test combining with other flags
    assert!(layer.pass(&["-v", "config", "--json"]));
    let json: serde_json::Value = serde_json::from_str(&layer.last_stdout)?;
    assert_eq!(json["log_level"], "TRACE");

    Ok(())
}

#[rstest]
fn test_no_config_flag_positioning(mut layer: LayerManager) -> Result<()> {
    layer.no_default_options = true;

    // Test --no-config flag in different positions
    assert!(layer.pass(&["--no-config", "config"]));
    // Should show defaults when no config is loaded
    assert!(layer.last_stdout.contains("net=none"));

    assert!(layer.pass(&["config", "--no-config"]));
    assert!(layer.last_stdout.contains("net=none"));

    // Test with other flags
    assert!(layer.pass(&["--no-config", "--net=host", "config"]));
    assert!(layer.last_stdout.contains("net=host"));

    assert!(layer.pass(&["config", "--no-config", "--net=host"]));
    assert!(layer.last_stdout.contains("net=host"));

    Ok(())
}
