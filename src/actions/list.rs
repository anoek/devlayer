use anyhow::Result;
use colored::*;
use fast_glob::glob_match;
use log::trace;
use nix::{sys::signal::kill, unistd::Pid};
use serde_json::Value;
use std::path::Path;

use crate::{
    outln,
    util::{get_layer_pid_path, set_json_output},
};

pub fn list(layers_storage_dir: &Path, patterns: &[String]) -> Result<()> {
    trace!("Listing layers");

    let [running_layers, stopped_layers] =
        get_layers(layers_storage_dir, patterns)?;

    if !running_layers.is_empty() {
        outln!("Running layers:");
        for layer in &running_layers {
            outln!("{}", layer);
        }
    }

    if !running_layers.is_empty() && !stopped_layers.is_empty() {
        outln!("");
    }

    if !stopped_layers.is_empty() {
        outln!("{}", "Stopped layers:".dimmed());
        for layer in &stopped_layers {
            outln!("{}", layer.dimmed());
        }
    }

    set_json_output(
        "running_layers",
        &Value::Array(running_layers.into_iter().map(Value::String).collect()),
    );
    set_json_output(
        "stopped_layers",
        &Value::Array(stopped_layers.into_iter().map(Value::String).collect()),
    );

    Ok(())
}

pub fn get_layers(
    layers_storage_dir: &Path,
    patterns: &[String],
) -> Result<[Vec<String>; 2]> {
    let mut running_layers: Vec<String> = Vec::new();
    let mut stopped_layers: Vec<String> = Vec::new();

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

            if patterns.is_empty()
                || patterns
                    .iter()
                    .any(|pattern| glob_match(pattern, &layer_name))
            {
                let pid_file =
                    get_layer_pid_path(layers_storage_dir, &layer_name);
                if pid_file.exists() {
                    let pid = std::fs::read_to_string(pid_file)?;

                    match pid.parse::<i32>() {
                        Ok(pid) => {
                            let pid = Pid::from_raw(pid);
                            if kill(pid, None).is_ok() {
                                running_layers.push(layer_name);
                            } else {
                                stopped_layers.push(layer_name);
                            }
                        }
                        Err(_) => {
                            stopped_layers.push(layer_name);
                        }
                    }
                } else {
                    stopped_layers.push(layer_name);
                }
            }
        }
    }

    running_layers.sort();
    stopped_layers.sort();

    Ok([running_layers, stopped_layers])
}
