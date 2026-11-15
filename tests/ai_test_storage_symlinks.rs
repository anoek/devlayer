mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

/// Root doesn't follow a symlink where it writes one of a layer's files in
/// the storage directory, which the user owns. `file` is relative to the
/// storage directory, with `{}` standing for the layer's name.
#[rstest]
#[case("{}.lock")]
#[case("{}.pid")]
#[case("{}/data/settings.json")]
#[case("{}/origin.json")]
fn ai_test_storage_symlinks_not_followed(
    mut layer: LayerManager,
    #[case] file: &str,
) -> Result<()> {
    let layer_dir = layer.dir()?;
    let link = layer_dir
        .parent()
        .expect("layer dir has a parent")
        .join(file.replace("{}", &layer.name));
    if let Some(dir) = link.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let target =
        std::env::current_dir()?.join(layer.test_filename("symlink-target"));
    std::os::unix::fs::symlink(&target, &link)?;

    assert!(layer.xfail(&["true"]));
    assert!(
        !target.exists(),
        "followed {} to {}",
        file,
        target.display()
    );
    std::fs::remove_file(&link)?;
    Ok(())
}
