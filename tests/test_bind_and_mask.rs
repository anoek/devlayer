mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_mask_no_mask(mut layer: LayerManager) -> Result<()> {
    let test_dir = layer.test_filename("not_masked");
    std::fs::create_dir(&test_dir)?;
    let test_file = format!("{}/should-not-be-masked", test_dir);
    std::fs::write(&test_file, "test")?;

    layer.run(&["ls", "-l", &test_dir])?;
    println!("{}", layer.last_stdout);
    assert!(layer.last_stdout.contains("should-not-be-masked"));
    Ok(())
}

#[rstest]
fn test_mask_mask_cli(mut layer: LayerManager) -> Result<()> {
    let test_dir = layer.test_filename("mask_from_env");
    std::fs::create_dir(&test_dir)?;
    let test_file = format!("{}/should-be-masked", test_dir);
    std::fs::write(&test_file, "test")?;
    layer.run(&["--mask", &test_dir, "ls", "-l", &test_dir])?;
    println!("{}", layer.last_stdout);
    assert!(!layer.last_stdout.contains("should-be-masked"));
    Ok(())
}

#[rstest]
fn test_mask_mask_cli_no_config(mut layer: LayerManager) -> Result<()> {
    let test_dir = layer.test_filename("mask_from_env");
    std::fs::create_dir(&test_dir)?;
    let test_file = format!("{}/should-be-masked", test_dir);
    std::fs::write(&test_file, "test")?;
    layer.run(&["--no-config", "--mask", &test_dir, "ls", "-l", &test_dir])?;
    println!("{}", layer.last_stdout);
    assert!(!layer.last_stdout.contains("should-be-masked"));
    Ok(())
}

#[rstest]
fn test_mask_config(mut layer: LayerManager) -> Result<()> {
    let config_file1 = layer.test_filename("config.toml");
    let config_file2 = layer.test_filename("config2.toml");
    std::fs::write(&config_file1, "mask = [\"/tmp\"]")?;
    std::fs::write(&config_file2, "mask = [\"/run\"]")?;

    layer.run(&[
        "--config",
        &config_file1,
        "--config",
        &config_file2,
        "config",
        "config_files",
    ])?;
    println!("{}", layer.last_stdout);
    println!("{}", layer.last_stderr);
    assert!(layer.last_stdout.contains(&config_file1));
    assert!(layer.last_stdout.contains(&config_file2));

    layer.run(&[
        "--config",
        &config_file1,
        "--config",
        &config_file2,
        "config",
        "mask",
    ])?;
    println!("{}", layer.last_stdout);
    println!("{}", layer.last_stderr);
    assert!(layer.last_stdout.contains("/tmp"));
    assert!(layer.last_stdout.contains("/run"));

    Ok(())
}

#[rstest]
fn test_mask_env(mut layer: LayerManager) -> Result<()> {
    let test_dir = layer.test_filename("mask_from_env");
    std::fs::create_dir(&test_dir)?;
    let test_file = format!("{}/should-be-masked", test_dir);
    std::fs::write(&test_file, "test")?;
    layer.run_with_env(
        &["--no-config", "ls", "-l", &test_dir],
        "DEVLAYER_MASK",
        &test_dir,
    )?;
    println!("{}", layer.last_stdout);
    assert!(!layer.last_stdout.contains("should-be-masked"));
    Ok(())
}

#[rstest]
fn test_mask_empty_env(mut layer: LayerManager) -> Result<()> {
    let test_dir = layer.test_filename("mask_from_env");
    std::fs::create_dir(&test_dir)?;
    let test_file = format!("{}/should-not-be-masked", test_dir);
    std::fs::write(&test_file, "test")?;
    layer.run_with_env(
        &["--no-config", "ls", "-l", &test_dir],
        "DEVLAYER_MASK",
        "",
    )?;
    println!("{}", layer.last_stdout);
    assert!(layer.last_stdout.contains("should-not-be-masked"));
    Ok(())
}

#[rstest]
fn test_mask_bind_mask(mut layer: LayerManager) -> Result<()> {
    let test_dir = layer.test_filename("mask_from_env");
    std::fs::create_dir(&test_dir)?;
    let test_file = format!("{}/should-be-masked", test_dir);
    std::fs::write(&test_file, "test")?;
    layer.run_with_env(
        &["ls", "-l", &test_dir],
        "DEVLAYER_BIND",
        &format!("{}:{}:mask", test_dir, test_dir),
    )?;
    println!("{}", layer.last_stdout);
    assert!(!layer.last_stdout.contains("should-be-masked"));
    Ok(())
}

#[rstest]
fn test_mask_file(mut layer: LayerManager) -> Result<()> {
    let test_dir = layer.test_filename("mask_from_env");
    std::fs::create_dir(&test_dir)?;
    let test_file = format!("{}/should-be-masked", test_dir);
    std::fs::write(&test_file, "test")?;
    layer.run_with_env(
        &["cat", &test_file],
        "DEVLAYER_BIND",
        &format!("{}:{}:mask", test_file, test_file),
    )?;
    // file exists but is masked so won't contain the test we wrote
    assert!(!layer.last_stdout.contains("test"));

    layer.run_with_env(
        &["ls", &test_file],
        "DEVLAYER_BIND",
        &format!("{}:{}:mask", test_file, test_file),
    )?;
    println!("{}", layer.last_stdout);
    // we should see it with ls
    assert!(layer.last_stdout.contains("should-be-masked"));

    Ok(())
}

#[rstest]
fn test_mask_combined(mut layer: LayerManager) -> Result<()> {
    layer.run_with_env(
        &["--mask", "/tmp", "config"],
        "DEVLAYER_MASK",
        "/run",
    )?;
    assert!(layer.last_stdout.contains("/tmp"));
    assert!(layer.last_stdout.contains("/run"));

    Ok(())
}

#[rstest]
fn test_bind_conflict(mut layer: LayerManager) -> Result<()> {
    assert!(layer.xfail(&["--bind", "/tmp::ro,/tmp::rw"]));
    Ok(())
}
