// Integration tests for environment variable validation
use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

fn get_binary_path() -> &'static str {
    env!("CARGO_BIN_EXE_cni-gesprek")
}

mod environment_validation_tests {
    use super::*;

    #[test]
    fn test_invalid_netns_path_format() {
        let config = r#"{
            "cniVersion": "1.0.0", 
            "name": "test-invalid-netns",
            "type": "cni-gesprek"
        }"#;

        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(config.as_bytes()).unwrap();

        let mut cmd = Command::new(get_binary_path());
        cmd.env("CNI_COMMAND", "ADD")
            .env("CNI_CONTAINERID", "test")
            .env("CNI_NETNS", "/invalid/path") // Invalid netns format
            .env("CNI_IFNAME", "eth0")
            .pipe_stdin(&config_file)
            .unwrap();

        cmd.assert().failure().stderr(predicate::str::contains(
            "Environment variable 'CNI_NETNS' has invalid value '/invalid/path'",
        ));
    }

    #[test]
    fn test_invalid_interface_name_format() {
        let config = r#"{
            "cniVersion": "1.0.0",
            "name": "test-invalid-ifname",
            "type": "cni-gesprek"
        }"#;

        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(config.as_bytes()).unwrap();

        let mut cmd = Command::new(get_binary_path());
        cmd.env("CNI_COMMAND", "ADD")
            .env("CNI_CONTAINERID", "test")
            .env("CNI_NETNS", "/proc/123/ns/net")
            .env("CNI_IFNAME", "invalid interface name") // Invalid ifname (space)
            .pipe_stdin(&config_file)
            .unwrap();

        cmd.assert().failure().stderr(predicate::str::contains(
            "Environment variable 'CNI_IFNAME' has invalid value 'invalid interface name'",
        ));
    }

    #[test]
    fn test_invalid_container_id_format() {
        let config = r#"{
            "cniVersion": "1.0.0",
            "name": "test-invalid-container-id",
            "type": "cni-gesprek"
        }"#;

        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(config.as_bytes()).unwrap();

        let mut cmd = Command::new(get_binary_path());
        cmd.env("CNI_COMMAND", "DEL")
            .env("CNI_CONTAINERID", "invalid@container#id") // Invalid container ID (special chars)
            .env("CNI_NETNS", "/proc/123/ns/net")
            .env("CNI_IFNAME", "eth0")
            .pipe_stdin(&config_file)
            .unwrap();

        cmd.assert().failure().stderr(predicate::str::contains(
            "Environment variable 'CNI_CONTAINERID' has invalid value 'invalid@container#id'",
        ));
    }

    #[test]
    fn test_interface_name_too_long() {
        let config = r#"{
            "cniVersion": "1.0.0",
            "name": "test-ifname-too-long",
            "type": "cni-gesprek"
        }"#;

        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(config.as_bytes()).unwrap();

        let mut cmd = Command::new(get_binary_path());
        cmd.env("CNI_COMMAND", "ADD")
            .env("CNI_CONTAINERID", "test")
            .env("CNI_NETNS", "/proc/123/ns/net")
            .env("CNI_IFNAME", "very-long-interface-name") // 24 chars (too long)
            .pipe_stdin(&config_file)
            .unwrap();

        cmd.assert().failure().stderr(
            predicate::str::contains(
                "Environment variable 'CNI_IFNAME' has invalid value 'very-long-interface-name'",
            )
            .and(predicate::str::contains("maximum 15 characters")),
        );
    }

    #[test]
    fn test_interface_name_starts_with_dot() {
        let config = r#"{
            "cniVersion": "1.0.0",
            "name": "test-ifname-dot",
            "type": "cni-gesprek"
        }"#;

        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(config.as_bytes()).unwrap();

        let mut cmd = Command::new(get_binary_path());
        cmd.env("CNI_COMMAND", "ADD")
            .env("CNI_CONTAINERID", "test")
            .env("CNI_NETNS", "/proc/123/ns/net")
            .env("CNI_IFNAME", ".eth0") // Starts with dot (invalid)
            .pipe_stdin(&config_file)
            .unwrap();

        cmd.assert().failure().stderr(
            predicate::str::contains("Environment variable 'CNI_IFNAME' has invalid value '.eth0'")
                .and(predicate::str::contains("does not start with a dot")),
        );
    }

    #[test]
    fn test_valid_environment_variables() {
        let config = r#"{
            "cniVersion": "1.0.0",
            "name": "test-valid-env",
            "type": "cni-gesprek"
        }"#;

        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(config.as_bytes()).unwrap();

        // Test with all valid environment variables
        let mut cmd = Command::new(get_binary_path());
        cmd.env("CNI_COMMAND", "ADD")
            .env("CNI_CONTAINERID", "test-container-123")
            .env("CNI_NETNS", "/proc/1234/ns/net") // Valid netns format
            .env("CNI_IFNAME", "eth0") // Valid ifname
            .arg("--dry-run") // Avoid actual network operations
            .pipe_stdin(&config_file)
            .unwrap();

        // Should succeed (no validation errors)
        cmd.assert().success();
    }

    #[test]
    fn test_valid_var_run_netns_format() {
        let config = r#"{
            "cniVersion": "1.0.0",
            "name": "test-var-run-netns",
            "type": "cni-gesprek"
        }"#;

        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(config.as_bytes()).unwrap();

        let mut cmd = Command::new(get_binary_path());
        cmd.env("CNI_COMMAND", "ADD")
            .env("CNI_CONTAINERID", "test")
            .env("CNI_NETNS", "/var/run/netns/test-namespace") // Valid /var/run/netns format
            .env("CNI_IFNAME", "veth1234")
            .arg("--dry-run")
            .pipe_stdin(&config_file)
            .unwrap();

        cmd.assert().success();
    }

    #[test]
    fn test_valid_run_netns_format() {
        let config = r#"{
            "cniVersion": "1.0.0",
            "name": "test-run-netns",
            "type": "cni-gesprek"
        }"#;

        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(config.as_bytes()).unwrap();

        let mut cmd = Command::new(get_binary_path());
        cmd.env("CNI_COMMAND", "ADD")
            .env("CNI_CONTAINERID", "test")
            .env("CNI_NETNS", "/run/netns/my-test-ns") // Valid /run/netns format
            .env("CNI_IFNAME", "cni-12345")
            .arg("--dry-run")
            .pipe_stdin(&config_file)
            .unwrap();

        cmd.assert().success();
    }
}
