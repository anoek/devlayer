use crate::actions::list::get_layers;
use crate::config::Config;
use crate::layer::Layer;
use crate::util::Lock;
use crate::{outln, util::set_json_output};
use anyhow::Result;
use colored::{Color, Colorize};
use log::trace;
use serde_json::Value;
use std::io::{self, Write};
use std::path::Path;

struct LayerEntry {
    name: String,
    stopped: bool,
    layer: Layer,
    _lock: Box<Lock>,
}

impl LayerEntry {
    pub fn new(
        layers_storage_dir: &Path,
        layer_name: &str,
        stopped: bool,
    ) -> Result<Self> {
        let (layer, lock) = Layer::get(
            layers_storage_dir,
            layer_name,
            nix::unistd::Uid::from_raw(0),
            nix::unistd::Gid::from_raw(0),
            None,
        )?;

        let layer = match layer {
            Some(layer) => layer,
            None => Layer::from_location(
                layers_storage_dir,
                layer_name,
                nix::unistd::Uid::from_raw(0),
                nix::unistd::Gid::from_raw(0),
            ),
        };

        Ok(Self {
            name: layer_name.to_string(),
            stopped,
            layer,
            _lock: lock,
        })
    }
}

pub fn delete(
    config: &Config,
    layers_storage_dir: &Path,
    patterns: &[String],
    force: bool,
) -> Result<()> {
    trace!("Preparing to delete layers");

    let [running_layers, stopped_layers] = if patterns.is_empty() {
        if Layer::from_location(
            layers_storage_dir,
            &config.name,
            nix::unistd::Uid::from_raw(0),
            nix::unistd::Gid::from_raw(0),
        )
        .exists()
        {
            // serves as a check to see if the layer is running or not
            if Layer::get(
                layers_storage_dir,
                &config.name,
                nix::unistd::Uid::from_raw(0),
                nix::unistd::Gid::from_raw(0),
                None,
            )?
            .0
            .is_some()
            {
                [vec![config.name.clone()], vec![]]
            } else {
                [vec![], vec![config.name.clone()]]
            }
        } else {
            [vec![], vec![]]
        }
    } else {
        get_layers(layers_storage_dir, patterns)?
    };
    /*
    let mut layer_entries = Vec::new();
    for layer_name in running_layers {
        layer_entries.push(LayerEntry::new(
            layers_storage_dir,
            layer_name.as_str(),
            false,
        )?);
    }
    for layer_name in stopped_layers {
        layer_entries.push(LayerEntry::new(
            layers_storage_dir,
            layer_name.as_str(),
            true,
        )?);
    }
    */

    if stopped_layers.is_empty() && running_layers.is_empty() {
        if patterns.is_empty() {
            outln!(
                "No layer by the name of '{}' found to delete.",
                config.name
            );
        } else {
            outln!("No layers found matching the specified patterns.");
        }
        return Ok(());
    }

    delete_layers(
        config,
        layers_storage_dir,
        &running_layers,
        &stopped_layers,
        force,
    )
}

/// Lists the given layers, asks for confirmation unless `force` is set,
/// then deletes them.
pub fn delete_layers(
    config: &Config,
    layers_storage_dir: &Path,
    running_layers: &[String],
    stopped_layers: &[String],
    force: bool,
) -> Result<()> {
    // Show layers that will be deleted and ask for confirmation
    if !force {
        outln!("The following layers will be deleted:");
        for (list, stopped) in [(running_layers, false), (stopped_layers, true)]
        {
            for name in list {
                let layer_entry = LayerEntry::new(
                    layers_storage_dir,
                    name.as_str(),
                    stopped,
                )?;

                let modifications =
                    layer_entry.layer.count_upper_entries(config)?;
                let color = if modifications.not_ignored > 0 {
                    Color::Red
                } else if !layer_entry.stopped {
                    Color::Green
                } else {
                    Color::White
                };

                outln!(
                    "  {} {} ({} entries, {} ignored)",
                    if layer_entry.stopped {
                        "stopped"
                    } else {
                        "running"
                    },
                    layer_entry.name.color(color),
                    modifications.not_ignored,
                    modifications.ignored
                );
            }
        }

        // Use eprint! for the prompt since print! is not allowed
        eprint!("\nAre you sure you want to delete these layers? [y/N] ");
        let _ = io::stderr().flush();

        let mut response = String::new();
        io::stdin().read_line(&mut response)?;

        if !response.trim().eq_ignore_ascii_case("y") {
            outln!("Delete operation cancelled.");
            return Ok(());
        }
    }

    // Delete each layer
    let mut deleted_layers = Vec::new();
    let mut errors = Vec::new();

    for (list, stopped) in [(running_layers, false), (stopped_layers, true)] {
        for name in list {
            let layer_entry =
                LayerEntry::new(layers_storage_dir, name.as_str(), stopped)?;

            trace!("Deleting layer: {}", layer_entry.name);
            match layer_entry.layer.delete() {
                Ok(()) => {
                    outln!("Deleted layer: {}", layer_entry.name.green());
                    deleted_layers
                        .push(Value::String(layer_entry.name.clone()));
                }
                Err(e) => {
                    outln!("Error deleting layer: {}", e);
                    errors.push(Value::String(e.to_string()));
                }
            }
        }
    }

    outln!("{} layers deleted", deleted_layers.len());

    // Set JSON output
    set_json_output("status", &Value::String("success".to_string()));
    set_json_output("deleted", &Value::Array(deleted_layers));
    set_json_output("errors", &Value::Array(errors));

    Ok(())
}
