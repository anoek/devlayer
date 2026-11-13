mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Runs devlayer with the default layer selection from project directories
/// under the system temp dir, outside the repository, so layers that other
/// tests create in the repository root never match --last. Layers created
/// here are tracked and deleted on drop.
struct Selection {
    layer: LayerManager,
    storage: PathBuf,
    root: PathBuf,
    before: BTreeSet<String>,
}

impl Selection {
    fn new(layer: LayerManager) -> Result<Self> {
        let mut layer = layer;
        // No --join, so the default selection applies
        layer.no_default_options = true;
        let root = std::env::temp_dir()
            .join(format!("devlayer-coverage-sel-{}", rid()));
        std::fs::create_dir_all(&root)?;
        layer.cwd = Some(root.clone());
        layer.run(&["--new", "config", "storage_dir"])?;
        let storage = PathBuf::from(layer.last_stdout.trim());
        let mut sel = Self {
            layer,
            storage,
            root,
            before: BTreeSet::new(),
        };
        sel.before = sel.layers()?;
        Ok(sel)
    }

    /// Creates (if needed) and returns `root/rel`
    fn dir(&self, rel: &str) -> Result<PathBuf> {
        let dir = self.root.join(rel);
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    fn pass(&mut self, cwd: &Path, args: &[&str]) -> bool {
        self.layer.cwd = Some(cwd.to_path_buf());
        self.layer.pass(args)
    }

    fn xfail(&mut self, cwd: &Path, args: &[&str]) -> bool {
        self.layer.cwd = Some(cwd.to_path_buf());
        self.layer.xfail(args)
    }

    fn stdout(&self) -> String {
        self.layer.last_stdout.trim().to_string()
    }

    /// Names of all layers in the storage directory
    fn layers(&self) -> Result<BTreeSet<String>> {
        let mut names = BTreeSet::new();
        for entry in std::fs::read_dir(&self.storage)? {
            let entry = entry?;
            if entry.path().is_dir() {
                names.insert(entry.file_name().to_string_lossy().to_string());
            }
        }
        Ok(names)
    }

    /// Layers created since this Selection was made
    fn created(&self) -> Result<BTreeSet<String>> {
        Ok(self.layers()?.difference(&self.before).cloned().collect())
    }

    /// Runs `true` in `cwd` with the default selection and returns the name
    /// of the layer it created
    fn create_in(&mut self, cwd: &Path) -> Result<String> {
        let before = self.layers()?;
        assert!(self.pass(cwd, &["true"]));
        let after = self.layers()?;
        let created: Vec<_> = after.difference(&before).cloned().collect();
        assert_eq!(created.len(), 1, "expected exactly one new layer");
        // Keep creation times strictly ordered
        std::thread::sleep(std::time::Duration::from_millis(20));
        Ok(created[0].clone())
    }
}

impl Drop for Selection {
    fn drop(&mut self) {
        let root = self.root.clone();
        for name in self.created().unwrap_or_default() {
            let join = format!("--join={}", name);
            let _ = self.pass(&root, &[&join, "delete", "-y"]);
        }
        self.layer.cwd = None;
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[rstest]
fn ai_test_bare_run_creates_new_each_time(layer: LayerManager) -> Result<()> {
    let mut sel = Selection::new(layer)?;
    let dir = sel.dir("project")?;

    let first = sel.create_in(&dir)?;
    let second = sel.create_in(&dir)?;
    assert_ne!(first, second);
    assert!(first.starts_with("ephemeral_"));
    assert!(second.starts_with("ephemeral_"));
    Ok(())
}

#[rstest]
fn ai_test_last_nearest_ancestor_and_newest(layer: LayerManager) -> Result<()> {
    let mut sel = Selection::new(layer)?;
    let dir = sel.dir("project")?;
    let sub = sel.dir("project/sub")?;
    let deeper = sel.dir("project/sub/deeper")?;

    let _l1 = sel.create_in(&dir)?;
    let l2 = sel.create_in(&dir)?;
    let l3 = sel.create_in(&sub)?;

    // Newest of the layers created in this exact directory
    assert!(sel.pass(&dir, &["--last", "config", "name"]));
    assert_eq!(sel.stdout(), l2);

    // Nearest ancestor with layers wins
    assert!(sel.pass(&deeper, &["--last", "config", "name"]));
    assert_eq!(sel.stdout(), l3);

    // A layer created deeper down is never chosen from a parent directory
    let parent = sel.root.clone();
    assert!(sel.xfail(&parent, &["--last", "config", "name"]));
    assert!(sel.layer.last_stderr.contains("No devlayer found"));
    Ok(())
}

#[rstest]
fn ai_test_actions_default_to_last(layer: LayerManager) -> Result<()> {
    let mut sel = Selection::new(layer)?;
    let dir = sel.dir("project")?;
    let outside = sel.root.join("outside-file");
    let outside = outside.to_str().unwrap_or_default().to_string();

    assert!(sel.pass(&dir, &["touch", &outside]));
    let name = sel.created()?.into_iter().next();

    assert!(sel.pass(&dir, &["status", "/"]));
    assert!(sel.layer.last_stdout.contains("outside-file"));

    assert!(sel.pass(&dir, &["config", "name"]));
    assert_eq!(Some(sel.stdout()), name);
    Ok(())
}

#[rstest]
fn ai_test_stop_delete_default_to_last(layer: LayerManager) -> Result<()> {
    let mut sel = Selection::new(layer)?;
    let dir = sel.dir("project")?;
    let other_dir = sel.dir("other")?;
    let name = sel.create_in(&dir)?;
    let other = sel.create_in(&other_dir)?;

    assert!(sel.pass(&dir, &["stop"]));
    assert!(sel.pass(&dir, &["delete", "-y"]));
    let layers = sel.layers()?;
    assert!(!layers.contains(&name));
    assert!(layers.contains(&other));
    Ok(())
}

#[rstest]
fn ai_test_list_clean_sync_no_layer_ok(layer: LayerManager) -> Result<()> {
    let mut sel = Selection::new(layer)?;
    let empty = sel.dir("empty")?;

    assert!(sel.pass(&empty, &["list"]));
    assert!(sel.pass(&empty, &["clean", "-y"]));
    assert!(sel.pass(&empty, &["config"]));
    // sync drops system caches, which may not be allowed inside containers;
    // it must not fail because there is no layer for this directory
    sel.pass(&empty, &["sync"]);
    assert!(!sel.layer.last_stderr.contains("No devlayer found"));

    assert!(sel.created()?.is_empty());
    Ok(())
}

#[rstest]
fn ai_test_explicit_selector_wins(layer: LayerManager) -> Result<()> {
    let mut sel = Selection::new(layer)?;
    let dir = sel.dir("project")?;
    let name = sel.create_in(&dir)?;

    assert!(
        sel.pass(&dir, &["--join=devlayer-coverage-other", "config", "name"])
    );
    assert_eq!(sel.stdout(), "devlayer-coverage-other");

    assert!(sel.pass(&dir, &["--new", "config", "name"]));
    assert!(sel.stdout().starts_with("ephemeral_"));
    assert_ne!(sel.stdout(), name);
    Ok(())
}

#[rstest]
fn ai_test_last_errors_without_layer(layer: LayerManager) -> Result<()> {
    let mut sel = Selection::new(layer)?;
    let empty = sel.dir("empty")?;

    assert!(sel.xfail(&empty, &["status"]));
    assert!(sel.layer.last_stderr.contains("No devlayer found"));
    Ok(())
}
