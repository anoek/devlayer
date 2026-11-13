mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_cli(mut sandbox: SandboxManager) -> Result<()> {
    assert!(sandbox.pass(&["--version"]));
    assert!(sandbox.pass(&["--", "ls"]));
    assert!(sandbox.xfail(&["--bad-option"]));

    assert!(sandbox.pass(&["--net", "config"]));
    assert!(sandbox.last_stdout.contains("net=host"));
    assert!(sandbox.pass(&["--net=none", "config"]));
    assert!(sandbox.last_stdout.contains("net=none"));
    assert!(sandbox.pass(&["--net=host", "config"]));
    assert!(sandbox.last_stdout.contains("net=host"));
    assert!(sandbox.xfail(&["--net=foobar", "config"]));

    assert!(sandbox.pass(&["--name", "test-cli", "config"]));
    assert!(sandbox.last_stdout.contains("name=test-cli"));
    assert!(sandbox.pass(&["--name=test-cli", "config"]));
    assert!(sandbox.last_stdout.contains("name=test-cli"));

    assert!(sandbox.pass(&["--storage-dir=/tmp/layer", "config"]));
    assert!(sandbox.last_stdout.contains("storage_dir=/tmp/layer"));

    assert!(sandbox.pass(&["--storage-dir=/tmp/layer", "config"]));
    assert!(sandbox.last_stdout.contains("storage_dir=/tmp/layer"));

    // default values check
    sandbox.no_default_options = true;
    assert!(sandbox.pass(&["--no-config", "config"]));
    assert!(sandbox.last_stdout.contains("name=layer"));
    assert!(sandbox.last_stdout.contains("net=none"));

    // Test config_files display
    assert!(sandbox.pass(&["--no-config", "config", "config_files"]));
    assert!(sandbox.last_stdout == "\n"); // Should be empty with --no-config

    Ok(())
}

#[rstest]
fn test_cli_with_config(mut sandbox: SandboxManager) -> Result<()> {
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
    sandbox.no_default_options = true;
    assert!(sandbox.epass(&["config"], "DEVLAYER_BIND", ".cargo"));
    assert!(sandbox.last_stdout.contains("name=parent"));
    assert!(sandbox.last_stdout.contains("net=none"));
    assert!(sandbox.last_stdout.contains("log_level=TRACE"));
    assert!(sandbox.last_stdout.contains("storage_dir=/tmp/layer1"));
    assert!(sandbox.last_stdout.contains(".cargo"));

    std::fs::write(".devlayer.toml", config)?;
    sandbox.no_default_options = true;
    assert!(sandbox.pass(&["config", "--bind=.cargo"]));
    assert!(sandbox.last_stdout.contains("name=test-cli-with-config"));
    assert!(sandbox.last_stdout.contains("net=host"));
    println!("{}", sandbox.last_stdout);
    assert!(sandbox.last_stdout.contains("log_level=INFO"));
    assert!(sandbox.last_stdout.contains("storage_dir=/tmp/layer2"));

    std::fs::remove_file(".devlayer.toml")?;
    std::fs::remove_file("../.devlayer.conf")?;

    Ok(())
}

#[rstest]
fn test_cli_with_env(mut sandbox: SandboxManager) -> Result<()> {
    assert!(sandbox.epass(&["config"], "DEVLAYER_NET", ""));
    assert!(sandbox.epass(&["config"], "DEVLAYER_NET", "host"));
    assert!(sandbox.last_stdout.contains("net=host"));
    assert!(sandbox.epass(&["config"], "DEVLAYER_NET", "none"));
    assert!(sandbox.last_stdout.contains("net=none"));
    assert!(sandbox.exfail(&["config"], "DEVLAYER_NET", "foobar"));
    assert!(sandbox.last_stderr.contains("Invalid network type: foobar"));

    assert!(sandbox.epass(&["config"], "DEVLAYER_LOG_LEVEL", "trace"));
    assert!(sandbox.last_stdout.contains("log_level=TRACE"));

    assert!(sandbox.exfail(&["config"], "DEVLAYER_LOG_LEVEL", "foobar"));
    assert!(sandbox.last_stderr.contains("Invalid log level: foobar"));

    assert!(sandbox.epass(&["config"], "DEVLAYER_NAME", ""));
    assert!(sandbox.epass(&["config"], "DEVLAYER_NAME", "test-cli-with-env"));

    assert!(sandbox.epass(&["config"], "DEVLAYER_STORAGE_DIR", ""));
    assert!(sandbox.epass(&["config"], "DEVLAYER_STORAGE_DIR", "/tmp/layer"));
    assert!(sandbox.last_stdout.contains("storage_dir=/tmp/layer"));

    Ok(())
}

#[rstest]
fn test_cli_resolve_failures(mut sandbox: SandboxManager) -> Result<()> {
    assert!(sandbox.exfail(&["config"], "TEST_UNABLE_TO_OPEN_PROC_MOUNTS", ""));
    assert!(sandbox.exfail(&["config"], "TEST_UNABLE_TO_FIND_MOUNT_POINT", ""));
    assert!(sandbox.exfail(&["config"], "TEST_UNACCEPTABLE_MOUNT_TYPE", ""));
    assert!(sandbox.xfail(&["--storage-dir=/bad/path", "config"]));
    assert!(sandbox.last_stderr.contains("Insufficient access"));

    Ok(())
}
