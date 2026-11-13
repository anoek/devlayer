mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_cli(mut layer: LayerManager) -> Result<()> {
    assert!(layer.pass(&["--version"]));
    assert!(layer.pass(&["--", "ls"]));
    assert!(layer.xfail(&["--bad-option"]));

    assert!(layer.pass(&["--net", "config"]));
    assert!(layer.last_stdout.contains("net=host"));
    assert!(layer.pass(&["--net=none", "config"]));
    assert!(layer.last_stdout.contains("net=none"));
    assert!(layer.pass(&["--net=host", "config"]));
    assert!(layer.last_stdout.contains("net=host"));
    assert!(layer.xfail(&["--net=foobar", "config"]));

    assert!(layer.pass(&["--name", "test-cli", "config"]));
    assert!(layer.last_stdout.contains("name=test-cli"));
    assert!(layer.pass(&["--name=test-cli", "config"]));
    assert!(layer.last_stdout.contains("name=test-cli"));

    assert!(layer.pass(&["--storage-dir=/tmp/layer", "config"]));
    assert!(layer.last_stdout.contains("storage_dir=/tmp/layer"));

    assert!(layer.pass(&["--storage-dir=/tmp/layer", "config"]));
    assert!(layer.last_stdout.contains("storage_dir=/tmp/layer"));

    // default values check
    layer.no_default_options = true;
    assert!(layer.pass(&["--no-config", "config"]));
    assert!(layer.last_stdout.contains("name=layer"));
    assert!(layer.last_stdout.contains("net=none"));

    // Test config_files display
    assert!(layer.pass(&["--no-config", "config", "config_files"]));
    assert!(layer.last_stdout == "\n"); // Should be empty with --no-config

    Ok(())
}

#[rstest]
fn test_cli_with_config(mut layer: LayerManager) -> Result<()> {
    let parent_config = r#"
    net = "none"
    name = "parent"
    log_level = "trace"
    storage_dir = "/tmp/layer1"
    bind = [".cargo"]
    "#;
    let config = r#"
    net = "host"
    name = "test-cli-with-config"
    log_level = "info"
    storage_dir = "/tmp/layer2"
    bind = [".cargo"]
    "#;

    let _ = std::fs::remove_file(".devlayer.toml");
    let _ = std::fs::remove_file("../.devlayer.conf");

    std::fs::write("../.devlayer.conf", parent_config)?;
    layer.no_default_options = true;
    assert!(layer.epass(&["config"], "DEVLAYER_BIND", ".cargo"));
    assert!(layer.last_stdout.contains("name=parent"));
    assert!(layer.last_stdout.contains("net=none"));
    assert!(layer.last_stdout.contains("log_level=TRACE"));
    assert!(layer.last_stdout.contains("storage_dir=/tmp/layer1"));
    assert!(layer.last_stdout.contains(".cargo"));

    std::fs::write(".devlayer.toml", config)?;
    layer.no_default_options = true;
    assert!(layer.pass(&["config", "--bind=.cargo"]));
    assert!(layer.last_stdout.contains("name=test-cli-with-config"));
    assert!(layer.last_stdout.contains("net=host"));
    println!("{}", layer.last_stdout);
    assert!(layer.last_stdout.contains("log_level=INFO"));
    assert!(layer.last_stdout.contains("storage_dir=/tmp/layer2"));

    std::fs::remove_file(".devlayer.toml")?;
    std::fs::remove_file("../.devlayer.conf")?;

    Ok(())
}

#[rstest]
fn test_cli_with_env(mut layer: LayerManager) -> Result<()> {
    assert!(layer.epass(&["config"], "DEVLAYER_NET", ""));
    assert!(layer.epass(&["config"], "DEVLAYER_NET", "host"));
    assert!(layer.last_stdout.contains("net=host"));
    assert!(layer.epass(&["config"], "DEVLAYER_NET", "none"));
    assert!(layer.last_stdout.contains("net=none"));
    assert!(layer.exfail(&["config"], "DEVLAYER_NET", "foobar"));
    assert!(layer.last_stderr.contains("Invalid network type: foobar"));

    assert!(layer.epass(&["config"], "DEVLAYER_LOG_LEVEL", "trace"));
    assert!(layer.last_stdout.contains("log_level=TRACE"));

    assert!(layer.exfail(&["config"], "DEVLAYER_LOG_LEVEL", "foobar"));
    assert!(layer.last_stderr.contains("Invalid log level: foobar"));

    assert!(layer.epass(&["config"], "DEVLAYER_NAME", ""));
    assert!(layer.epass(&["config"], "DEVLAYER_NAME", "test-cli-with-env"));

    assert!(layer.epass(&["config"], "DEVLAYER_STORAGE_DIR", ""));
    assert!(layer.epass(&["config"], "DEVLAYER_STORAGE_DIR", "/tmp/layer"));
    assert!(layer.last_stdout.contains("storage_dir=/tmp/layer"));

    Ok(())
}

#[rstest]
fn test_cli_resolve_failures(mut layer: LayerManager) -> Result<()> {
    assert!(layer.exfail(&["config"], "TEST_UNABLE_TO_OPEN_PROC_MOUNTS", ""));
    assert!(layer.exfail(&["config"], "TEST_UNABLE_TO_FIND_MOUNT_POINT", ""));
    assert!(layer.exfail(&["config"], "TEST_UNACCEPTABLE_MOUNT_TYPE", ""));
    assert!(layer.xfail(&["--storage-dir=/bad/path", "config"]));
    assert!(layer.last_stderr.contains("Insufficient access"));

    Ok(())
}
