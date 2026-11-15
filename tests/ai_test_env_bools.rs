mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

/// A true/false environment variable with any other value is an error, and
/// an empty one is ignored like other empty environment variables
#[rstest]
#[case("DEVLAYER_IGNORED")]
#[case("DEVLAYER_NO_DEFAULT_BINDS")]
#[case("DEVLAYER_NO_DEFAULT_LLM_BINDS")]
#[case("DEVLAYER_NO_BIND_CWD")]
fn ai_test_env_bool_values(
    mut layer: LayerManager,
    #[case] var: &str,
) -> Result<()> {
    assert!(layer.exfail(&["config", "name"], var, "cow"));
    assert!(
        layer
            .last_stderr
            .contains(&format!("Invalid value for {}: cow", var)),
        "{}",
        layer.last_stderr
    );
    assert!(layer.epass(&["config", "name"], var, ""));
    Ok(())
}
