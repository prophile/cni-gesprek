use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::env;
use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;

fn get_binary_path() -> PathBuf {
    let mut binary_path = env::current_exe().unwrap();
    binary_path.pop(); // remove test binary name
    if binary_path.ends_with("deps") {
        binary_path.pop(); // remove deps directory
    }
    binary_path.push("cni-gesprek");
    binary_path
}

#[test]
fn test_version_command() {
    let mut cmd = Command::new(get_binary_path());
    cmd.env("CNI_COMMAND", "VERSION");

    cmd.assert()
        .success()
        .stdout(predicate::function(|output: &str| {
            // Parse JSON and verify structure
            if let Ok(json) = serde_json::from_str::<Value>(output) {
                json.get("cniVersion").is_some()
                    && json.get("supportedVersions").is_some()
                    && json["cniVersion"] == "1.0.0"
            } else {
                false
            }
        }));
}

#[test]
fn test_add_command_dry_run() {
    let config = r#"{
        "cniVersion": "1.0.0",
        "name": "test-network",
        "type": "cni-gesprek",
        "master": "eth0",
        "podCIDR": "2001:db8::/64"
    }"#;

    let mut config_file = NamedTempFile::new().unwrap();
    config_file.write_all(config.as_bytes()).unwrap();

    let mut cmd = Command::new(get_binary_path());
    cmd.env("CNI_COMMAND", "ADD")
        .env("CNI_CONTAINERID", "test-container")
        .env("CNI_NETNS", "/proc/123/ns/net")
        .env("CNI_IFNAME", "eth0")
        .arg("--dry-run")
        .pipe_stdin(&config_file)
        .unwrap();

    cmd.assert()
        .success()
        .stderr(predicate::str::contains("[DRY-RUN]"))
        .stderr(predicate::str::contains("ip link add"))
        .stderr(predicate::str::contains("ipvlan"))
        .stdout(predicate::function(|output: &str| {
            // Parse JSON result
            if let Ok(json) = serde_json::from_str::<Value>(output) {
                json.get("interfaces").is_some()
                    && json.get("ips").is_some()
                    && json["cniVersion"] == "1.0.0"
            } else {
                false
            }
        }));
}

#[test]
fn test_add_command_with_dns_dry_run() {
    let config = r#"{
        "cniVersion": "1.0.0",
        "name": "test-network-dns",
        "type": "cni-gesprek",
        "master": "eth1",
        "podCIDR": "2001:db8:1::/64",
        "dns": {
            "nameservers": ["2001:db8::53"],
            "domain": "example.com"
        }
    }"#;

    let mut config_file = NamedTempFile::new().unwrap();
    config_file.write_all(config.as_bytes()).unwrap();

    let mut cmd = Command::new(get_binary_path());
    cmd.env("CNI_COMMAND", "ADD")
        .env("CNI_CONTAINERID", "test-container-dns")
        .env("CNI_NETNS", "/proc/456/ns/net")
        .env("CNI_IFNAME", "net1")
        .arg("--dry-run")
        .pipe_stdin(&config_file)
        .unwrap();

    cmd.assert()
        .success()
        .stdout(predicate::function(|output: &str| {
            // Parse JSON and verify DNS is preserved
            if let Ok(json) = serde_json::from_str::<Value>(output) {
                json.get("dns").is_some()
                    && json["dns"]["nameservers"].as_array().unwrap().len() > 0
                    && json["dns"]["domain"] == "example.com"
            } else {
                false
            }
        }));
}

#[test]
fn test_add_command_auto_detect_interface() {
    let config = r#"{
        "cniVersion": "1.0.0",
        "name": "test-auto",
        "type": "cni-gesprek",
        "podCIDR": "2001:db8:2::/64"
    }"#;

    let mut config_file = NamedTempFile::new().unwrap();
    config_file.write_all(config.as_bytes()).unwrap();

    let mut cmd = Command::new(get_binary_path());
    cmd.env("CNI_COMMAND", "ADD")
        .env("CNI_CONTAINERID", "test-container-auto")
        .env("CNI_NETNS", "/proc/789/ns/net")
        .env("CNI_IFNAME", "eth0")
        .arg("--dry-run")
        .pipe_stdin(&config_file)
        .unwrap();

    cmd.assert()
        .success()
        .stderr(predicate::str::contains("Inferring default interface"));
}

#[test]
fn test_add_command_auto_detect_subnet() {
    let config = r#"{
        "cniVersion": "1.0.0",
        "name": "test-subnet-detect",
        "type": "cni-gesprek",
        "master": "eth2"
    }"#;

    let mut config_file = NamedTempFile::new().unwrap();
    config_file.write_all(config.as_bytes()).unwrap();

    let mut cmd = Command::new(get_binary_path());
    cmd.env("CNI_COMMAND", "ADD")
        .env("CNI_CONTAINERID", "test-container-subnet")
        .env("CNI_NETNS", "/proc/101112/ns/net")
        .env("CNI_IFNAME", "net0")
        .arg("--dry-run")
        .pipe_stdin(&config_file)
        .unwrap();

    cmd.assert()
        .success()
        .stderr(predicate::str::contains("Checking subnet"))
        .stderr(predicate::str::contains("Checking gateway"));
}

#[test]
fn test_del_command() {
    let config = r#"{
        "cniVersion": "1.0.0",
        "name": "test-delete",
        "type": "cni-gesprek"
    }"#;

    let mut config_file = NamedTempFile::new().unwrap();
    config_file.write_all(config.as_bytes()).unwrap();

    let mut cmd = Command::new(get_binary_path());
    cmd.env("CNI_COMMAND", "DEL")
        .env("CNI_CONTAINERID", "test-container-del")
        .env("CNI_NETNS", "/proc/131415/ns/net")
        .env("CNI_IFNAME", "eth0")
        .pipe_stdin(&config_file)
        .unwrap();

    cmd.assert()
        .success()
        .stdout(predicate::function(|output: &str| {
            if let Ok(json) = serde_json::from_str::<Value>(output) {
                json["cniVersion"] == "1.0.0"
            } else {
                false
            }
        }));
}

#[test]
fn test_check_command() {
    let config = r#"{
        "cniVersion": "1.0.0",
        "name": "test-check",
        "type": "cni-gesprek"
    }"#;

    let mut config_file = NamedTempFile::new().unwrap();
    config_file.write_all(config.as_bytes()).unwrap();

    let mut cmd = Command::new(get_binary_path());
    cmd.env("CNI_COMMAND", "CHECK")
        .env("CNI_CONTAINERID", "test-container-check")
        .env("CNI_NETNS", "/proc/161718/ns/net")
        .env("CNI_IFNAME", "eth0")
        .pipe_stdin(&config_file)
        .unwrap();

    cmd.assert()
        .success()
        .stdout(predicate::function(|output: &str| {
            if let Ok(json) = serde_json::from_str::<Value>(output) {
                json["cniVersion"] == "1.0.0"
            } else {
                false
            }
        }));
}

#[test]
fn test_status_command() {
    let config = r#"{
        "cniVersion": "1.0.0",
        "name": "test-status",
        "type": "cni-gesprek",
        "master": "eth0"
    }"#;

    let mut config_file = NamedTempFile::new().unwrap();
    config_file.write_all(config.as_bytes()).unwrap();

    let mut cmd = Command::new(get_binary_path());
    cmd.env("CNI_COMMAND", "STATUS")
        .env("CNI_CONTAINERID", "test-container-status")
        .env("CNI_NETNS", "/proc/192021/ns/net")
        .env("CNI_IFNAME", "eth0")
        .arg("--dry-run")
        .pipe_stdin(&config_file)
        .unwrap();

    cmd.assert().success().stderr(predicate::str::contains(
        "Checking if interface eth0 exists",
    ));
}

#[test]
fn test_missing_environment_variables() {
    let config = r#"{
        "cniVersion": "1.0.0",
        "name": "test-missing-env",
        "type": "cni-gesprek"
    }"#;

    let mut config_file = NamedTempFile::new().unwrap();
    config_file.write_all(config.as_bytes()).unwrap();

    let mut cmd = Command::new(get_binary_path());
    cmd.env("CNI_COMMAND", "ADD")
        // Missing CNI_NETNS, CNI_IFNAME, etc.
        .pipe_stdin(&config_file)
        .unwrap();

    cmd.assert().failure();
}

#[test]
fn test_invalid_json_config() {
    let invalid_config = r#"{ invalid json"#;

    let mut config_file = NamedTempFile::new().unwrap();
    config_file.write_all(invalid_config.as_bytes()).unwrap();

    let mut cmd = Command::new(get_binary_path());
    cmd.env("CNI_COMMAND", "ADD")
        .env("CNI_CONTAINERID", "test-container")
        .env("CNI_NETNS", "/proc/123/ns/net")
        .env("CNI_IFNAME", "eth0")
        .pipe_stdin(&config_file)
        .unwrap();

    cmd.assert().failure();
}

#[test]
fn test_unknown_command() {
    let mut cmd = Command::new(get_binary_path());
    cmd.env("CNI_COMMAND", "UNKNOWN");

    cmd.assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Unknown CNI_COMMAND"));
}
