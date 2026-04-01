//! E2E tests that require Docker daemon and tmux.
//!
//! These tests are marked `#[ignore]` by default because they create real
//! Docker containers and tmux sessions. Run them explicitly:
//!
//!   cargo test -p palette-cli --test docker_e2e -- --ignored --test-threads=1
//!
//! Test 1 (launch): Verifies container creation, settings injection, prompt
//!   copy, and Claude Code command delivery to tmux panes.
//!
//! Test 2 (claude_responds): Verifies Claude Code actually starts and
//!   produces output. Asserts non-empty output, prints content for visual
//!   inspection.

use anyhow::{Context as _, Result};
use palette_domain::worker::WorkerRole;
use palette_orchestrator::DockerConfig;
use std::path::{Path, PathBuf};
use std::process::Command;

const SESSION_NAME: &str = "palette-test";

/// Minimal config struct for loading test configuration.
#[derive(serde::Deserialize)]
struct TestConfig {
    docker: DockerConfig,
}

impl TestConfig {
    fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
}

/// Resolve a path relative to the workspace root (two levels up from palette-cli).
fn workspace_path(relative: &str) -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("../..").join(relative)
}

struct TestGuard {
    containers: Vec<String>,
}

impl TestGuard {
    fn new() -> Self {
        Self {
            containers: Vec::new(),
        }
    }

    fn track(&mut self, name: &str) {
        self.containers.push(name.to_string());
    }
}

impl Drop for TestGuard {
    fn drop(&mut self) {
        for name in &self.containers {
            let _ = Command::new("docker").args(["rm", "-f", name]).output();
        }
        let _ = Command::new("tmux")
            .args(["kill-session", "-t", SESSION_NAME])
            .output();
    }
}

fn tmux_run(args: &[&str]) -> Result<String> {
    let output = Command::new("tmux")
        .args(args)
        .output()
        .context("failed to run tmux")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn tmux_capture(target: &str) -> Result<String> {
    tmux_run(&["capture-pane", "-t", target, "-p"])
}

fn docker_exec(container: &str, cmd: &str) -> Result<String> {
    let output = Command::new("docker")
        .args(["exec", container, "sh", "-c", cmd])
        .output()
        .context("failed to docker exec")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Test 1: Verify the full launch sequence up to Claude Code command delivery.
///
/// - Creates tmux session and windows
/// - Creates and starts Docker containers (leader + member)
/// - Injects settings.json with correct palette_url and worker_id
/// - Copies prompt files into containers
/// - Sends `docker exec -it ... claude` command to tmux panes
/// - Verifies all of the above via assertions
#[test]
#[ignore]
fn launch() -> Result<()> {
    let mut guard = TestGuard::new();

    // --- Setup tmux ---
    let _ = Command::new("tmux")
        .args(["kill-session", "-t", SESSION_NAME])
        .output();
    let output = Command::new("tmux")
        .args(["new-session", "-d", "-s", SESSION_NAME])
        .output()?;
    assert!(output.status.success(), "failed to create tmux session");

    // Create windows
    let _ = Command::new("tmux")
        .args(["new-window", "-t", SESSION_NAME, "-n", "leader"])
        .output()?;
    let _ = Command::new("tmux")
        .args(["new-window", "-t", SESSION_NAME, "-n", "member-a"])
        .output()?;

    // --- Setup Docker containers ---
    let config = TestConfig::load(&workspace_path("config/palette.toml"))?;
    let docker = palette_docker::DockerManager::new(config.docker.palette_url.clone());

    let leader_id = docker.create_container(
        "test-leader",
        &config.docker.leader_image,
        WorkerRole::Leader,
        SESSION_NAME,
        None,
        None,
        None,
    )?;
    guard.track("palette-test-leader");
    docker.start_container(&leader_id)?;

    let member_id = docker.create_container(
        "test-member-a",
        &config.docker.member_image,
        WorkerRole::Member,
        SESSION_NAME,
        None,
        None,
        None,
    )?;
    guard.track("palette-test-member-a");
    docker.start_container(&member_id)?;

    // --- Verify containers are running ---
    let leader_status = Command::new("docker")
        .args(["inspect", "-f", "{{.State.Running}}", leader_id.as_ref()])
        .output()?;
    assert_eq!(
        String::from_utf8_lossy(&leader_status.stdout).trim(),
        "true",
        "leader container should be running"
    );

    let member_status = Command::new("docker")
        .args(["inspect", "-f", "{{.State.Running}}", member_id.as_ref()])
        .output()?;
    assert_eq!(
        String::from_utf8_lossy(&member_status.stdout).trim(),
        "true",
        "member container should be running"
    );

    // --- Write settings.json ---
    let template_path = workspace_path(&config.docker.settings_template);
    docker.write_settings(&leader_id, &template_path, "leader-1")?;
    docker.write_settings(&member_id, &template_path, "member-a")?;

    // Verify settings.json content inside containers
    let leader_settings = docker_exec(
        &format!("palette-{}", "test-leader"),
        "cat /home/agent/.claude/settings.json",
    )?;
    let expected_leader_stop = format!(
        "{}/hooks/stop?worker_id=leader-1",
        config.docker.palette_url
    );
    let expected_leader_notif = format!(
        "{}/hooks/notification?worker_id=leader-1",
        config.docker.palette_url
    );
    assert!(
        leader_settings.contains(&expected_leader_stop),
        "leader settings should contain stop hook with worker_id.\nActual:\n{leader_settings}"
    );
    assert!(
        leader_settings.contains(&expected_leader_notif),
        "leader settings should contain notification hook with worker_id"
    );

    let member_settings = docker_exec(
        &format!("palette-{}", "test-member-a"),
        "cat /home/agent/.claude/settings.json",
    )?;
    let expected_member_stop = format!(
        "{}/hooks/stop?worker_id=member-a",
        config.docker.palette_url
    );
    assert!(
        member_settings.contains(&expected_member_stop),
        "member settings should contain stop hook with worker_id"
    );

    // --- Copy prompt files ---
    palette_docker::DockerManager::copy_file_to_container(
        &leader_id,
        &workspace_path(&config.docker.leader_prompt),
        "/home/agent/prompt.md",
    )?;
    palette_docker::DockerManager::copy_file_to_container(
        &member_id,
        &workspace_path(&config.docker.crafter_prompt),
        "/home/agent/prompt.md",
    )?;

    // Verify prompt files exist
    let leader_prompt = docker_exec("palette-test-leader", "cat /home/agent/prompt.md")?;
    assert!(
        leader_prompt.contains("Leader Agent"),
        "leader prompt should contain 'Leader Agent'"
    );

    let crafter_prompt = docker_exec("palette-test-member-a", "cat /home/agent/prompt.md")?;
    assert!(
        crafter_prompt.contains("Crafter Agent"),
        "crafter prompt should contain 'Crafter Agent'"
    );

    // --- Send Claude Code command to tmux panes ---
    let leader_cmd = palette_docker::DockerManager::claude_exec_command(
        &leader_id,
        "/home/agent/prompt.md",
        WorkerRole::Leader,
        None,
    );
    let leader_target = format!("{SESSION_NAME}:leader");
    let output = Command::new("tmux")
        .args(["send-keys", "-t", &leader_target, "-l", &leader_cmd])
        .output()?;
    assert!(output.status.success(), "failed to send keys to leader");
    // Send Enter
    let _ = Command::new("tmux")
        .args(["send-keys", "-t", &leader_target, "Enter"])
        .output()?;

    let member_cmd = palette_docker::DockerManager::claude_exec_command(
        &member_id,
        "/home/agent/prompt.md",
        WorkerRole::Member,
        None,
    );
    let member_target = format!("{SESSION_NAME}:member-a");
    let output = Command::new("tmux")
        .args(["send-keys", "-t", &member_target, "-l", &member_cmd])
        .output()?;
    assert!(output.status.success(), "failed to send keys to member");
    let _ = Command::new("tmux")
        .args(["send-keys", "-t", &member_target, "Enter"])
        .output()?;

    // Brief wait for tmux to register the keys
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Verify tmux panes contain the claude command
    let leader_pane = tmux_capture(&leader_target)?;
    assert!(
        leader_pane.contains("docker exec -it") && leader_pane.contains("claude"),
        "leader pane should show claude exec command.\nActual:\n{leader_pane}"
    );

    let member_pane = tmux_capture(&member_target)?;
    assert!(
        member_pane.contains("docker exec -it") && member_pane.contains("claude"),
        "member pane should show claude exec command.\nActual:\n{member_pane}"
    );

    // --- Verify container labels ---
    let labels = Command::new("docker")
        .args([
            "inspect",
            "-f",
            "{{index .Config.Labels \"palette.managed\"}} {{index .Config.Labels \"palette.role\"}}",
            "palette-test-leader",
        ])
        .output()?;
    let label_str = String::from_utf8_lossy(&labels.stdout).trim().to_string();
    assert_eq!(label_str, "true leader", "leader labels mismatch");

    let labels = Command::new("docker")
        .args([
            "inspect",
            "-f",
            "{{index .Config.Labels \"palette.managed\"}} {{index .Config.Labels \"palette.role\"}}",
            "palette-test-member-a",
        ])
        .output()?;
    let label_str = String::from_utf8_lossy(&labels.stdout).trim().to_string();
    assert_eq!(label_str, "true member", "member labels mismatch");

    println!("=== Test 1 (launch) passed: all assertions verified ===");
    Ok(())
}

/// Test 2: Verify Claude Code actually starts and produces output.
///
/// This test launches Claude Code in a container and waits for it to
/// produce output. It asserts that the output is non-empty, then prints
/// the captured content for visual inspection.
///
/// Requires valid Claude credentials mounted into the container.
#[test]
#[ignore]
fn claude_responds() -> Result<()> {
    let mut guard = TestGuard::new();

    // --- Setup tmux ---
    let _ = Command::new("tmux")
        .args(["kill-session", "-t", SESSION_NAME])
        .output();
    let output = Command::new("tmux")
        .args(["new-session", "-d", "-s", SESSION_NAME])
        .output()?;
    assert!(output.status.success(), "failed to create tmux session");

    let _ = Command::new("tmux")
        .args(["new-window", "-t", SESSION_NAME, "-n", "claude-test"])
        .output()?;

    // --- Setup container ---
    let config = TestConfig::load(&workspace_path("config/palette.toml"))?;
    let docker = palette_docker::DockerManager::new(config.docker.palette_url.clone());

    let container_id = docker.create_container(
        "test-claude",
        &config.docker.leader_image,
        WorkerRole::Leader,
        SESSION_NAME,
        None,
        None,
        None,
    )?;
    guard.track("palette-test-claude");
    docker.start_container(&container_id)?;

    // Inject settings and prompt
    docker.write_settings(
        &container_id,
        &workspace_path(&config.docker.settings_template),
        "test-claude",
    )?;
    palette_docker::DockerManager::copy_file_to_container(
        &container_id,
        &workspace_path(&config.docker.leader_prompt),
        "/home/agent/prompt.md",
    )?;

    // --- Launch Claude Code ---
    let target = format!("{SESSION_NAME}:claude-test");
    let cmd = palette_docker::DockerManager::claude_exec_command(
        &container_id,
        "/home/agent/prompt.md",
        WorkerRole::Leader,
        None,
    );
    let _ = Command::new("tmux")
        .args(["send-keys", "-t", &target, "-l", &cmd])
        .output()?;
    let _ = Command::new("tmux")
        .args(["send-keys", "-t", &target, "Enter"])
        .output()?;

    // Wait for Claude Code to start (it takes a few seconds to initialize)
    println!("Waiting for Claude Code to start (up to 30 seconds)...");
    let mut captured = String::new();
    for i in 0..6 {
        std::thread::sleep(std::time::Duration::from_secs(5));
        captured = tmux_capture(&target)?;

        // Check if Claude has produced output beyond just the command itself
        let lines: Vec<&str> = captured
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter(|l| !l.contains("docker exec"))
            .collect();

        if !lines.is_empty() {
            println!("Claude produced output after {} seconds", (i + 1) * 5);
            break;
        }
    }

    // Filter out the command line itself
    let output_lines: Vec<&str> = captured
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter(|l| !l.contains("docker exec"))
        .collect();

    println!("\n=== Captured tmux pane output ===");
    println!("{captured}");
    println!("=== End of captured output ===\n");

    assert!(
        !output_lines.is_empty(),
        "Claude Code should produce some output.\nFull pane:\n{captured}"
    );

    println!("=== Test 2 (claude_responds) passed: output is non-empty ===");
    println!("=== Please visually verify the output above ===");
    Ok(())
}
