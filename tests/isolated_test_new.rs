mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_new_last_name_with_ms() -> Result<()> {
    let mut layers: Vec<LayerManager> = Vec::new();

    for _ in 0..6 {
        let mut layer: LayerManager = LayerManager::new();
        layer.no_default_options = true;
        layers.push(layer);
    }

    for layer in &mut layers {
        // We spin here to test to ensure we've exercised the code that
        // dedupes names at the ms level. 6 times in case this test case
        // is crossing a second-boundary (the clock second is rolling over that is)

        assert!(layer.epass(
            &["--new", "-v", "true"],
            "TEST_NAME_WITH_MS",
            "devlayer-coverage-new-with-ms"
        ));
        layer.run(&["--last", "config", "name"])?;
        layer.name = layer.last_stdout.trim().to_string();

        println!("layer.name: {}", layer.name);
    }

    Ok(())
}
