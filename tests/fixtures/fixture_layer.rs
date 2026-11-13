use anyhow::Result;
use log::{error, warn};
use rand::Rng;
use rstest::*;
use std::path::Path;
use std::process::Child;
use std::{path::PathBuf, process::Command};
//use uuid::Uuid;

const COVERAGE_TEST_DATA_DIR: &str = "generated-test-data";

pub fn rid() -> String {
    let mut rng = rand::rng();
    let rid: String = (0..10)
        .map(|_| rng.sample(rand::distr::Alphanumeric) as char)
        .collect();
    rid
}

pub fn get_devlayer_bin() -> String {
    // Get the original working directory when the tests start
    // This is needed because tests may change directories
    let cargo_manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| {
            // Fallback: try to find the project root by looking for Cargo.toml
            let mut current = std::env::current_dir().unwrap();
            loop {
                if current.join("Cargo.toml").exists() {
                    break current.to_string_lossy().to_string();
                }
                if let Some(parent) = current.parent() {
                    current = parent.to_path_buf();
                } else {
                    panic!("Could not find project root");
                }
            }
        });

    let project_root = Path::new(&cargo_manifest_dir);

    let bin = if let Ok(current_exe) = std::env::current_exe() {
        if current_exe.to_string_lossy().contains("/coverage/") {
            "target/coverage/devlayer"
        } else {
            "target/debug/devlayer"
        }
    } else {
        // Fallback if we can't determine the path
        "target/debug/devlayer"
    };

    let path = project_root.join(bin);
    // Convert to absolute path
    let absolute_path = path.canonicalize().unwrap_or_else(|_| path.clone());
    println!("Path: {}", absolute_path.to_string_lossy());
    absolute_path.to_string_lossy().to_string()
}

pub struct LayerManager {
    pub name: String,
    pub last_stdout: String,
    pub last_stderr: String,
    pub all_stdout: String,
    pub all_stderr: String,
    pub debug_mode: bool,
    pub no_sudo: bool,
    /* we store the layer binary path so we can chdir as necessary */
    pub devlayer_bin: String,
    pub no_default_options: bool,
    pub ignored: bool, // Controls whether --ignored is automatically added
    /// When false (the default), --no-bind-cwd is added so that tests run
    /// from the repository root do not write straight to the checkout
    pub bind_cwd: bool,
    /// Directory to run commands from, defaults to the test process cwd
    pub cwd: Option<PathBuf>,
}

impl LayerManager {
    pub fn new() -> Self {
        let name = format!("devlayer-coverage-test-{}", rid());

        #[allow(clippy::panic)]
        match std::fs::create_dir_all(
            Path::new(COVERAGE_TEST_DATA_DIR).join(&name),
        ) {
            Ok(_) => (),
            Err(e) => {
                panic!(
                    "Failed to create {} dir: {}",
                    COVERAGE_TEST_DATA_DIR, e
                );
            }
        }

        Self {
            name,
            last_stdout: String::new(),
            last_stderr: String::new(),
            all_stdout: String::new(),
            all_stderr: String::new(),
            debug_mode: false,
            no_sudo: false,
            devlayer_bin: get_devlayer_bin(),
            no_default_options: false,
            ignored: true, // Default to true to maintain existing behavior
            bind_cwd: false,
            cwd: None,
        }
    }

    /* Controls whether --no-bind-cwd is automatically added to commands */
    #[allow(dead_code)]
    pub fn set_bind_cwd(&mut self, bind_cwd: bool) {
        self.bind_cwd = bind_cwd;
    }

    /// Adds --no-bind-cwd unless the test opted in to the cwd bind or passes
    /// the flag itself
    fn add_no_bind_cwd(&self, cmd: &mut Command, args: &[&str]) {
        if !self.bind_cwd && !args.contains(&"--no-bind-cwd") {
            cmd.args(["--no-bind-cwd"]);
        }
    }

    /// Applies the configured working directory to a command. A relative
    /// LLVM_PROFILE_FILE is made absolute so coverage data from commands run
    /// elsewhere still lands in this repository.
    fn apply_cwd(&self, cmd: &mut Command) {
        if let Some(cwd) = &self.cwd {
            cmd.current_dir(cwd);
            if let (Ok(profile), Ok(here)) =
                (std::env::var("LLVM_PROFILE_FILE"), std::env::current_dir())
                && Path::new(&profile).is_relative()
            {
                cmd.env("LLVM_PROFILE_FILE", here.join(profile));
            }
        }
    }

    /* When debug mode is on, the layer will not be cleaned up when the fixture is dropped */
    #[allow(dead_code)]
    pub fn set_debug_mode(&mut self, debug_mode: bool) {
        self.debug_mode = debug_mode;
    }

    /* Controls whether --ignored is automatically added to commands */
    #[allow(dead_code)]
    pub fn set_ignored(&mut self, ignored: bool) {
        self.ignored = ignored;
    }

    #[allow(dead_code)]
    pub fn dir(&self) -> Result<PathBuf> {
        let mut cmd = Command::new("sudo");
        cmd.args(["-E", &self.devlayer_bin]);

        if self.no_sudo {
            cmd = Command::new(&self.devlayer_bin);
        }

        cmd.args([format!("--join={}", &self.name)]);
        self.add_no_bind_cwd(&mut cmd, &[]);
        cmd.args(["config", "layer_dir"]);
        self.apply_cwd(&mut cmd);
        let output = cmd.output()?;
        let base = String::from_utf8_lossy(&output.stdout).to_string();
        let base = base.trim();
        Ok(PathBuf::from(base))
    }

    #[allow(dead_code)]
    pub fn test_filename(&self, prefix: &str) -> String {
        format!(
            "{}/{}/{}-{}",
            COVERAGE_TEST_DATA_DIR,
            self.name,
            prefix,
            rid()
        )
    }

    #[allow(dead_code)]
    pub fn test_filename_no_rid(&self, prefix: &str) -> String {
        format!("{}/{}/{}", COVERAGE_TEST_DATA_DIR, self.name, prefix,)
    }

    pub fn run(&mut self, args: &[&str]) -> Result<std::process::Output> {
        self.run_with_env(args, "", "")
    }

    #[allow(dead_code)]
    pub fn run_with_stdin(
        &mut self,
        args: &[&str],
        stdin_input: &str,
    ) -> Result<std::process::Output> {
        use std::io::Write;
        use std::process::Stdio;

        let mut cmd = Command::new("sudo");
        cmd.args(["-E", &self.devlayer_bin]);

        if self.no_sudo {
            cmd = Command::new(&self.devlayer_bin);
        }

        if !self.no_default_options
            && !args.iter().any(|arg| {
                arg.starts_with("--log_level") || arg.starts_with("-v")
            })
        {
            cmd.args(["-v"]);
        }

        if self.ignored {
            cmd.args(["--ignored"]);
        }

        cmd.args(["--no-config"]);

        if !self.no_default_options
            && !args.iter().any(|arg| arg.starts_with("--join"))
        {
            cmd.args([format!("--join={}", &self.name)]);
        }
        self.add_no_bind_cwd(&mut cmd, args);
        cmd.args(args);

        // Set up stdin
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        println!(
            "Running command with stdin: {} {}",
            cmd.get_program().to_string_lossy(),
            cmd.get_args()
                .map(|c| c.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ")
        );

        self.apply_cwd(&mut cmd);
        let mut child = cmd
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn command: {:?}", e))?;

        // Write to stdin
        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(stdin_input.as_bytes()).map_err(|e| {
                anyhow::anyhow!("Failed to write to stdin: {:?}", e)
            })?;
        }

        // Wait for the command to complete
        let output = child.wait_with_output().map_err(|e| {
            anyhow::anyhow!("Failed to wait for command: {:?}", e)
        })?;

        self.last_stdout = String::from_utf8_lossy(&output.stdout).to_string();
        self.last_stderr = String::from_utf8_lossy(&output.stderr).to_string();
        self.all_stdout += &self.last_stdout;
        self.all_stderr += &self.last_stderr;

        if let Some(code) = output.status.code() {
            if code != 0 {
                return Err(anyhow::anyhow!(
                    "Command returned non-zero exit code: {}\nstdout: {}\nstderr: {}",
                    code,
                    self.last_stdout,
                    self.last_stderr
                ));
            }
        } else {
            return Err(anyhow::anyhow!(
                "Command did not return a valid exit code\nstdout: {}\nstderr: {}",
                self.last_stdout,
                self.last_stderr
            ));
        }

        Ok(output)
    }

    pub fn run_with_env(
        &mut self,
        args: &[&str],
        env_key: &str,
        env_value: &str,
    ) -> Result<std::process::Output> {
        let mut cmd = Command::new("sudo");
        if !env_key.is_empty() {
            println!("Setting env var: {}={}", env_key, env_value);
            cmd.env(env_key, env_value);
        }
        cmd.args(["-E", &self.devlayer_bin]);

        if self.no_sudo {
            cmd = Command::new(&self.devlayer_bin);
        }

        if !self.no_default_options
            && !args.iter().any(|arg| {
                arg.starts_with("--log_level") || arg.starts_with("-v")
            })
        {
            cmd.args(["-v"]);
        }

        if self.ignored {
            cmd.args(["--ignored"]);
        }

        if !self.no_default_options
            && !args.iter().any(|arg| arg.starts_with("--join"))
        {
            cmd.args([format!("--join={}", &self.name)]);
        }
        self.add_no_bind_cwd(&mut cmd, args);
        cmd.args(args);
        println!(
            "Running command: {} {}",
            cmd.get_program().to_string_lossy(),
            cmd.get_args()
                .map(|c| c.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ")
        );
        self.apply_cwd(&mut cmd);
        match cmd.output() {
            Ok(output) => {
                self.last_stdout =
                    String::from_utf8_lossy(&output.stdout).to_string();
                self.last_stderr =
                    String::from_utf8_lossy(&output.stderr).to_string();
                self.all_stdout += &self.last_stdout;
                self.all_stderr += &self.last_stderr;

                if let Some(code) = output.status.code() {
                    if code != 0 {
                        return Err(anyhow::anyhow!(
                            "Command returned non-zero exit code: {}\nstdout: {}\nstderr: {}",
                            code,
                            self.last_stdout,
                            self.last_stderr
                        ));
                    }
                } else {
                    return Err(anyhow::anyhow!(
                        "Command did not return a valid exit code\nstdout: {}\nstderr: {}",
                        self.last_stdout,
                        self.last_stderr
                    ));
                }

                Ok(output)
            }
            Err(e) => Err(anyhow::anyhow!("Command failed: {:?}", e)),
        }
    }

    /// Leaves a process running in the layer so it doesn't shut itself down
    /// once a command finishes. The layer then stays up until it's stopped.
    #[allow(dead_code)]
    pub fn hold(&mut self) -> Result<()> {
        self.hold_with(&[])
    }

    /// Like `hold`, passing these options to devlayer, e.g. the resource
    /// limits a layer was started with, which joining it requires
    #[allow(dead_code)]
    pub fn hold_with(&mut self, options: &[&str]) -> Result<()> {
        let mut args = options.to_vec();
        args.extend(["sh", "-c", "sleep 3600 >/dev/null 2>&1 &"]);
        self.run(&args)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn run_in_background(&mut self, args: &[&str]) -> Result<Child> {
        let mut cmd = Command::new("sudo");
        cmd.args(["-E", &self.devlayer_bin]);

        if self.no_sudo {
            cmd = Command::new(&self.devlayer_bin);
        }

        if !self.no_default_options {
            cmd.args([format!("--join={}", &self.name)]);
        }
        self.add_no_bind_cwd(&mut cmd, args);
        cmd.args(args);

        println!(
            "Running command in background: {} {}",
            cmd.get_program().to_string_lossy(),
            cmd.get_args()
                .map(|c| c.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ")
        );

        self.apply_cwd(&mut cmd);
        Ok(cmd.spawn()?)
    }

    #[allow(dead_code)]
    pub fn exfail(
        &mut self,
        args: &[&str],
        env_key: &str,
        env_value: &str,
    ) -> bool {
        if self.run_with_env(args, env_key, env_value).is_err() {
            return true;
        }
        println!("last_stderr: {}", self.last_stderr);
        println!("last_stdout: {}", self.last_stdout);
        false
    }

    #[allow(dead_code)]
    pub fn epass(
        &mut self,
        args: &[&str],
        env_key: &str,
        env_value: &str,
    ) -> bool {
        if let Ok(output) = self.run_with_env(args, env_key, env_value) {
            return output.status.code().unwrap() == 0;
        }
        println!("last_stderr: {}", self.last_stderr);
        println!("last_stdout: {}", self.last_stdout);
        false
    }

    #[allow(dead_code)]
    pub fn pass(&mut self, args: &[&str]) -> bool {
        if let Ok(output) = self.run(args) {
            return output.status.code().unwrap() == 0;
        }
        println!("last_stderr: {}", self.last_stderr);
        println!("last_stdout: {}", self.last_stdout);
        false
    }

    #[allow(dead_code)]
    pub fn xfail(&mut self, args: &[&str]) -> bool {
        if let Ok(output) = self.run(args) {
            return output.status.code().unwrap() != 0;
        }
        println!("last_stderr: {}", self.last_stderr);
        println!("last_stdout: {}", self.last_stdout);
        true
    }
}

impl Drop for LayerManager {
    fn drop(&mut self) {
        // Reset flags to default state for cleanup commands
        self.no_default_options = false;
        self.ignored = true;
        self.cwd = None;

        if let Err(e) = self.run(&["stop"]) {
            error!("Failed to kill layer: {}", e);
            error!("last_stderr: {}", self.last_stderr);
            error!("last_stdout: {}", self.last_stdout);
            return;
        }
        if let Err(e) = self.run(&["accept", "**/*.profraw"]) {
            error!("Failed to accept profraw files: {}", e);
            error!("last_stderr: {}", self.last_stderr);
            error!("last_stdout: {}", self.last_stdout);
            return;
        }
        let dirname = Path::new(COVERAGE_TEST_DATA_DIR).join(&self.name);
        if !self.debug_mode {
            if let Err(e) = std::fs::remove_dir_all(&dirname) {
                // Only warn if the error is not "file not found" - the directory
                // may have already been cleaned up by the delete functionality
                if e.kind() != std::io::ErrorKind::NotFound {
                    warn!("Failed to remove {} dir: {}", dirname.display(), e);
                }
            }
            let layer_name = self.name.clone();
            match self.run(&["delete", "-y", layer_name.as_str()]) {
                Ok(_) => (),
                Err(e) => {
                    warn!("Failed to delete layer: {}", e);
                    warn!("last_stderr: {}", self.last_stderr);
                    warn!("last_stdout: {}", self.last_stdout);
                }
            }
        } else {
            warn!("Debug mode is on, *NOT* cleaning up {}", dirname.display());
        }
    }
}

#[fixture]
pub fn layer() -> LayerManager {
    LayerManager::new()
}
