use std::path::Path;

use fast_glob::glob_match;
use log::{trace, warn};
use nix::unistd::{Gid, Uid};

use crate::layer::Layer;
use crate::util::{get_layer_lock_path, get_layer_pid_path};
use anyhow::Result;

pub fn stop(
    layers_storage_dir: &Path,
    layer_name: &str,
    uid: Uid,
    gid: Gid,
) -> Result<()> {
    trace!("Stopping layer {}", layer_name);
    let (layer, _lock) =
        Layer::get(layers_storage_dir, layer_name, uid, gid, None)?;

    let layer = match layer {
        Some(layer) => layer,
        None => Layer::from_location(layers_storage_dir, layer_name, uid, gid),
    };

    layer.stop()?;
    layer.unmount()?;

    Ok(())
}

pub fn stop_all(
    layers_storage_dir: &Path,
    uid: Uid,
    gid: Gid,
    patterns: &[String],
) -> Result<()> {
    trace!("Stopping all layers in {}", layers_storage_dir.display());

    let layers = layers_storage_dir.read_dir()?;
    for layer in layers {
        let entry = layer?;
        let path = entry.path();
        let filename = path
            .components()
            .next_back()
            .expect("Failed to get filename")
            .as_os_str();
        if filename.to_string_lossy().ends_with(".lock") {
            let layer_name =
                filename.to_string_lossy()[..filename.len() - 5].to_string();

            if !get_layer_lock_path(layers_storage_dir, &layer_name).exists()
                || !get_layer_pid_path(layers_storage_dir, &layer_name).exists()
                || !layers_storage_dir.join(&layer_name).exists()
            {
                continue;
            }

            if patterns.is_empty()
                || patterns.iter().any(|pattern| {
                    let mut pattern = pattern.clone();
                    pattern = format!("*{pattern}*");
                    glob_match(&pattern, &layer_name)
                })
            {
                match stop(layers_storage_dir, &layer_name, uid, gid) {
                    Ok(_) => {}
                    Err(e) => {
                        warn!("Failed to kill layer {}: {}", layer_name, e);
                    }
                }
            }
        }
    }

    Ok(())
}
