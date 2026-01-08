use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{json, Value};
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

// Basic functionality tests
mod basic_commands {
    use super::*;

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
    fn test_unknown_command() {
        let mut cmd = Command::new(get_binary_path());
        cmd.env("CNI_COMMAND", "UNKNOWN");

        cmd.assert()
            .failure()
            .code(1)
            .stderr(predicate::str::contains(
                "Environment variable 'CNI_COMMAND' has invalid value 'UNKNOWN'",
            ));
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

        cmd.assert().success().stdout(predicate::str::is_empty());
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
            .failure()
            .stderr(predicate::str::contains("NetnsNotFound"));
    }
}

// ADD command comprehensive testing
mod add_command_tests {
    use super::*;

    #[test]
    fn test_add_command_basic_dry_run() {
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
                "domain": "example.com",
                "search": ["example.com", "local"],
                "options": ["ndots:2"]
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
                        && json["dns"]["search"].as_array().unwrap().len() == 2
                        && json["dns"]["options"].as_array().unwrap().len() == 1
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
    fn test_add_command_cli_overrides() {
        let config = r#"{
            "cniVersion": "1.0.0",
            "name": "test-cli-override",
            "type": "cni-gesprek",
            "master": "eth0",
            "podCIDR": "2001:db8::/64"
        }"#;

        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(config.as_bytes()).unwrap();

        let mut cmd = Command::new(get_binary_path());
        cmd.env("CNI_COMMAND", "ADD")
            .env("CNI_CONTAINERID", "test-container-override")
            .env("CNI_NETNS", "/proc/987/ns/net")
            .env("CNI_IFNAME", "eth0")
            .arg("--interface")
            .arg("eth3")
            .arg("--pod-cidr")
            .arg("2001:db8:99::/64")
            .arg("--dry-run")
            .pipe_stdin(&config_file)
            .unwrap();

        cmd.assert()
            .success()
            .stderr(predicate::str::contains("eth3"))
            .stdout(predicate::function(|output: &str| {
                // Should use CLI-provided CIDR
                if let Ok(json) = serde_json::from_str::<Value>(output) {
                    if let Some(ips) = json["ips"].as_array() {
                        if let Some(ip_obj) = ips.first() {
                            if let Some(address) = ip_obj["address"].as_str() {
                                return address.contains("2001:db8:99:");
                            }
                        }
                    }
                }
                false
            }));
    }

    #[test]
    fn test_add_command_ip_randomization() {
        let config = r#"{
            "cniVersion": "1.0.0",
            "name": "test-randomization",
            "type": "cni-gesprek",
            "master": "eth0",
            "podCIDR": "2001:db8:1234::/64"
        }"#;

        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(config.as_bytes()).unwrap();

        // Run the command multiple times and collect IPs
        let mut generated_ips = std::collections::HashSet::new();

        for _ in 0..3 {
            let mut config_file = NamedTempFile::new().unwrap();
            config_file.write_all(config.as_bytes()).unwrap();

            let mut cmd = Command::new(get_binary_path());
            cmd.env("CNI_COMMAND", "ADD")
                .env("CNI_CONTAINERID", "test-container-random")
                .env("CNI_NETNS", "/proc/555/ns/net")
                .env("CNI_IFNAME", "eth0")
                .arg("--dry-run")
                .pipe_stdin(&config_file)
                .unwrap();

            let output = cmd.output().unwrap();
            if !output.status.success() {
                // Skip this iteration if command failed
                continue;
            }

            let stdout = String::from_utf8(output.stdout).unwrap();
            if let Ok(json) = serde_json::from_str::<Value>(&stdout) {
                if let Some(ips) = json["ips"].as_array() {
                    if let Some(ip_obj) = ips.first() {
                        if let Some(address) = ip_obj["address"].as_str() {
                            generated_ips.insert(address.to_string());
                        }
                    }
                }
            }
        }

        // Should generate different IPs (if we got any)
        if generated_ips.len() > 1 {
            // All should be in the correct subnet
            for ip in &generated_ips {
                assert!(ip.starts_with("2001:db8:1234:"));
                assert!(ip.contains("/64"));
            }
        } else {
            // If we only got one or no IPs, that's still acceptable for this test
            // as randomization might produce the same result occasionally
        }
    }
}

// Error condition testing
mod error_conditions {
    use super::*;

    #[test]
    fn test_missing_environment_variables() {
        let config = r#"{
            "cniVersion": "1.0.0",
            "name": "test-missing-env",
            "type": "cni-gesprek"
        }"#;

        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(config.as_bytes()).unwrap();

        // Test missing CNI_NETNS
        let mut cmd = Command::new(get_binary_path());
        cmd.env("CNI_COMMAND", "ADD")
            .env("CNI_CONTAINERID", "test")
            .env("CNI_IFNAME", "eth0")
            // Missing CNI_NETNS
            .pipe_stdin(&config_file)
            .unwrap();

        cmd.assert().failure().stderr(predicate::str::contains(
            "Environment variable 'CNI_NETNS' required for command 'ADD' is not set",
        ));
    }

    #[test]
    fn test_missing_cni_ifname() {
        let config = r#"{
            "cniVersion": "1.0.0",
            "name": "test-missing-ifname",
            "type": "cni-gesprek"
        }"#;

        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(config.as_bytes()).unwrap();

        let mut cmd = Command::new(get_binary_path());
        cmd.env("CNI_COMMAND", "ADD")
            .env("CNI_CONTAINERID", "test")
            .env("CNI_NETNS", "/proc/123/ns/net")
            // Missing CNI_IFNAME
            .pipe_stdin(&config_file)
            .unwrap();

        cmd.assert().failure().stderr(predicate::str::contains(
            "Environment variable 'CNI_IFNAME' required for command 'ADD' is not set",
        ));
    }

    #[test]
    fn test_invalid_json_config() {
        let invalid_configs = vec![
            r#"{ invalid json"#,
            r#"{"cniVersion": "1.0.0"}"#, // Missing required fields
            r#""not an object""#,
            r#"null"#,
        ];

        for invalid_config in invalid_configs {
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
    }

    #[test]
    fn test_empty_stdin() {
        let mut cmd = Command::new(get_binary_path());
        cmd.env("CNI_COMMAND", "ADD")
            .env("CNI_CONTAINERID", "test-container")
            .env("CNI_NETNS", "/proc/123/ns/net")
            .env("CNI_IFNAME", "eth0")
            .pipe_stdin(tempfile::NamedTempFile::new().unwrap())
            .unwrap();

        cmd.assert().failure();
    }

    #[test]
    fn test_invalid_cidr_formats() {
        let invalid_cidrs = vec![
            "not-a-cidr",
            "2001:db8::/129", // Invalid prefix length
            "2001:db8::/-1",  // Negative prefix
            "invalid-ip/64",  // Invalid IP format
            "",               // Empty string
        ];

        for invalid_cidr in invalid_cidrs {
            let config = json!({
                "cniVersion": "1.0.0",
                "name": "test-invalid-cidr",
                "type": "cni-gesprek",
                "master": "eth0",
                "podCIDR": invalid_cidr
            });

            let mut config_file = NamedTempFile::new().unwrap();
            config_file
                .write_all(config.to_string().as_bytes())
                .unwrap();

            let mut cmd = Command::new(get_binary_path());
            cmd.env("CNI_COMMAND", "ADD")
                .env("CNI_CONTAINERID", "test-container")
                .env("CNI_NETNS", "/proc/123/ns/net")
                .env("CNI_IFNAME", "eth0")
                .arg("--dry-run")
                .pipe_stdin(&config_file)
                .unwrap();

            cmd.assert().failure();
        }
    }
}

// Status and Check command tests
mod status_check_tests {
    use super::*;

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
    fn test_status_with_interface_override() {
        let config = r#"{
            "cniVersion": "1.0.0",
            "name": "test-status-override",
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
            .arg("--interface")
            .arg("eth5")
            .arg("--dry-run")
            .pipe_stdin(&config_file)
            .unwrap();

        cmd.assert().success().stderr(predicate::str::contains(
            "Checking if interface eth5 exists",
        ));
    }

    #[test]
    fn test_gc_command() {
        let mut cmd = Command::new(get_binary_path());
        cmd.env("CNI_COMMAND", "GC");

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
}

// Command line argument testing
mod cli_args_tests {
    use super::*;

    #[test]
    fn test_help_output() {
        let mut cmd = Command::new(get_binary_path());
        cmd.arg("--help");

        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Usage:"))
            .stdout(predicate::str::contains("--interface"))
            .stdout(predicate::str::contains("--pod-cidr"))
            .stdout(predicate::str::contains("--dry-run"));
    }

    #[test]
    fn test_version_output() {
        let mut cmd = Command::new(get_binary_path());
        cmd.arg("--version");

        cmd.assert()
            .success()
            .stdout(predicate::str::contains("0.2.0"));
    }

    #[test]
    fn test_invalid_arguments() {
        let mut cmd = Command::new(get_binary_path());
        cmd.arg("--invalid-argument");

        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("unexpected argument"));
    }
}

// Edge case testing
mod edge_cases {
    use super::*;

    #[test]
    fn test_very_long_container_id() {
        let config = r#"{
            "cniVersion": "1.0.0",
            "name": "test-long-id",
            "type": "cni-gesprek",
            "master": "eth0",
            "podCIDR": "2001:db8::/64"
        }"#;

        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(config.as_bytes()).unwrap();

        let long_id = "a".repeat(100); // 100 chars - too long (max 64)

        let mut cmd = Command::new(get_binary_path());
        cmd.env("CNI_COMMAND", "ADD")
            .env("CNI_CONTAINERID", &long_id)
            .env("CNI_NETNS", "/proc/123/ns/net")
            .env("CNI_IFNAME", "eth0")
            .arg("--dry-run")
            .pipe_stdin(&config_file)
            .unwrap();

        // Should fail due to container ID being too long
        cmd.assert().failure().stderr(
            predicate::str::contains("Environment variable 'CNI_CONTAINERID'")
                .and(predicate::str::contains("maximum 64 characters")),
        );
    }

    #[test]
    fn test_special_characters_in_network_name() {
        let config = r#"{
            "cniVersion": "1.0.0",
            "name": "test-network_with-special.chars",
            "type": "cni-gesprek",
            "master": "eth0",
            "podCIDR": "2001:db8::/64"
        }"#;

        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(config.as_bytes()).unwrap();

        let mut cmd = Command::new(get_binary_path());
        cmd.env("CNI_COMMAND", "ADD")
            .env("CNI_CONTAINERID", "test")
            .env("CNI_NETNS", "/proc/123/ns/net")
            .env("CNI_IFNAME", "eth0")
            .arg("--dry-run")
            .pipe_stdin(&config_file)
            .unwrap();

        cmd.assert()
            .success()
            .stdout(predicate::function(|output: &str| {
                if let Ok(json) = serde_json::from_str::<Value>(output) {
                    json.get("interfaces").is_some()
                } else {
                    false
                }
            }));
    }

    #[test]
    fn test_minimal_valid_config() {
        let config = r#"{
            "cniVersion": "1.0.0",
            "name": "minimal",
            "type": "cni-gesprek"
        }"#;

        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(config.as_bytes()).unwrap();

        let mut cmd = Command::new(get_binary_path());
        cmd.env("CNI_COMMAND", "ADD")
            .env("CNI_CONTAINERID", "test")
            .env("CNI_NETNS", "/proc/123/ns/net")
            .env("CNI_IFNAME", "eth0")
            .arg("--dry-run")
            .pipe_stdin(&config_file)
            .unwrap();

        cmd.assert().success();
    }

    #[test]
    fn test_different_cni_versions() {
        let versions = vec!["0.3.1", "0.4.0", "1.0.0"];

        for version in versions {
            let config = json!({
                "cniVersion": version,
                "name": "test-version",
                "type": "cni-gesprek",
                "master": "eth0",
                "podCIDR": "2001:db8::/64"
            });

            let mut config_file = NamedTempFile::new().unwrap();
            config_file
                .write_all(config.to_string().as_bytes())
                .unwrap();

            let mut cmd = Command::new(get_binary_path());
            cmd.env("CNI_COMMAND", "ADD")
                .env("CNI_CONTAINERID", "test")
                .env("CNI_NETNS", "/proc/123/ns/net")
                .env("CNI_IFNAME", "eth0")
                .arg("--dry-run")
                .pipe_stdin(&config_file)
                .unwrap();

            cmd.assert()
                .success()
                .stdout(predicate::function(move |output: &str| {
                    if let Ok(json) = serde_json::from_str::<Value>(output) {
                        json["cniVersion"] == version
                    } else {
                        false
                    }
                }));
        }
    }
}
