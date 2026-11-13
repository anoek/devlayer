mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::path::PathBuf;

/// ~/.claude on this machine, if it exists. These tests use the real home
/// directory because the binary resolves it from the invoking user.
fn claude_dir() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let dir = PathBuf::from(home).join(".claude");
    dir.exists().then(|| dir.display().to_string())
}

fn binds_contain(stdout: &str, path: &str) -> bool {
    stdout.trim().split(',').any(|b| b == path)
}

#[rstest]
fn ai_test_llm_binds_default_on(mut layer: LayerManager) -> Result<()> {
    let Some(claude) = claude_dir() else {
        eprintln!("No ~/.claude on this machine, skipping");
        return Ok(());
    };
    layer.run(&["--no-config", "config", "bind"])?;
    assert!(binds_contain(&layer.last_stdout, &claude));
    Ok(())
}

#[rstest]
fn ai_test_no_default_llm_binds_flag(mut layer: LayerManager) -> Result<()> {
    let Some(claude) = claude_dir() else {
        return Ok(());
    };
    layer.run(&["--no-config", "--no-default-llm-binds", "config", "bind"])?;
    assert!(!binds_contain(&layer.last_stdout, &claude));
    Ok(())
}

#[rstest]
fn ai_test_no_default_llm_binds_env(mut layer: LayerManager) -> Result<()> {
    let Some(claude) = claude_dir() else {
        return Ok(());
    };
    layer.run_with_env(
        &["--no-config", "config", "bind"],
        "DEVLAYER_NO_DEFAULT_LLM_BINDS",
        "true",
    )?;
    assert!(!binds_contain(&layer.last_stdout, &claude));

    assert!(layer.exfail(
        &["--no-config", "config", "bind"],
        "DEVLAYER_NO_DEFAULT_LLM_BINDS",
        "yes"
    ));
    assert!(
        layer
            .last_stderr
            .contains("Invalid value for DEVLAYER_NO_DEFAULT_LLM_BINDS")
    );
    Ok(())
}

#[rstest]
fn ai_test_no_default_llm_binds_config(mut layer: LayerManager) -> Result<()> {
    let Some(claude) = claude_dir() else {
        return Ok(());
    };
    let config = format!("{}.toml", layer.test_filename("llm"));
    std::fs::write(&config, "no_default_llm_binds = true\n")?;
    let config_arg = format!("--config={}", config);
    layer.run(&[&config_arg, "config", "bind"])?;
    assert!(!binds_contain(&layer.last_stdout, &claude));

    layer.run(&[&config_arg, "config", "no_default_llm_binds"])?;
    assert_eq!(layer.last_stdout.trim(), "true");
    Ok(())
}

#[rstest]
fn ai_test_llm_binds_independent_of_no_default_binds(
    mut layer: LayerManager,
) -> Result<()> {
    let Some(claude) = claude_dir() else {
        return Ok(());
    };
    layer.run(&["--no-config", "--no-default-binds", "config", "bind"])?;
    assert!(binds_contain(&layer.last_stdout, &claude));
    Ok(())
}
