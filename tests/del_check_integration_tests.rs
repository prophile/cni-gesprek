use assert_cmd::Command;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_del_command_successful_cleanup() {
    let mut cni_config_file = NamedTempFile::new().unwrap();
    writeln!(
        cni_config_file,
        r#"{{
            "cniVersion": "1.0.0",
            "name": "test-del-network",
            "type": "ipvlan",
            "master": "eth0",
            "ipam": {{
                "type": "host-local",
                "subnet": "2001:db8::/64",
                "gateway": "2001:db8::1"
            }}
        }}"#
    )
    .unwrap();

    let config_contents = std::fs::read_to_string(cni_config_file.path()).unwrap();
    let mut cmd = Command::cargo_bin("cni-gesprek").unwrap();
    cmd.env("CNI_COMMAND", "DEL")
        .env("CNI_CONTAINERID", "test-container-del-123")
        .env("CNI_NETNS", "/proc/1234/ns/net")
        .env("CNI_IFNAME", "eth0")
        .env("CNI_PATH", "/usr/lib/cni")
        .write_stdin(config_contents);

    let output = cmd.assert().success();

    // DEL command should return empty response for successful cleanup
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);

    // According to CNI spec, DEL should return empty response on success
    assert_eq!(stdout.trim(), "");
}

#[test]
fn test_del_command_nonexistent_interface() {
    let mut cni_config_file = NamedTempFile::new().unwrap();
    writeln!(
        cni_config_file,
        r#"{{
            "cniVersion": "1.0.0",
            "name": "test-del-nonexistent",
            "type": "ipvlan",
            "master": "eth0",
            "ipam": {{
                "type": "host-local",
                "subnet": "2001:db8::/64"
            }}
        }}"#
    )
    .unwrap();

    let config_contents = std::fs::read_to_string(cni_config_file.path()).unwrap();
    let mut cmd = Command::cargo_bin("cni-gesprek").unwrap();
    cmd.env("CNI_COMMAND", "DEL")
        .env("CNI_CONTAINERID", "nonexistent-container-123")
        .env("CNI_NETNS", "/proc/9999/ns/net")
        .env("CNI_IFNAME", "nonexistent0")
        .env("CNI_PATH", "/usr/lib/cni")
        .write_stdin(config_contents);

    // DEL should succeed even if interface doesn't exist (idempotent)
    let output = cmd.assert().success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert_eq!(stdout.trim(), "");
}

#[test]
fn test_del_command_missing_containerid() {
    let mut cni_config_file = NamedTempFile::new().unwrap();
    writeln!(
        cni_config_file,
        r#"{{
            "cniVersion": "1.0.0",
            "name": "test-del-missing-id",
            "type": "ipvlan",
            "master": "eth0",
            "ipam": {{
                "type": "host-local",
                "subnet": "2001:db8::/64"
            }}
        }}"#
    )
    .unwrap();

    let config_contents = std::fs::read_to_string(cni_config_file.path()).unwrap();
    let mut cmd = Command::cargo_bin("cni-gesprek").unwrap();
    cmd.env("CNI_COMMAND", "DEL")
        // CNI_CONTAINERID is missing
        .env("CNI_NETNS", "/proc/1234/ns/net")
        .env("CNI_IFNAME", "eth0")
        .env("CNI_PATH", "/usr/lib/cni")
        .write_stdin(config_contents);

    // Should fail with appropriate error for missing containerid
    cmd.assert().failure();
}

#[test]
fn test_check_command_valid_configuration() {
    let mut cni_config_file = NamedTempFile::new().unwrap();
    writeln!(
        cni_config_file,
        r#"{{
            "cniVersion": "1.0.0",
            "name": "test-check-valid",
            "type": "ipvlan",
            "master": "eth0",
            "ipam": {{
                "type": "host-local",
                "subnet": "2001:db8::/64",
                "gateway": "2001:db8::1"
            }}
        }}"#
    )
    .unwrap();

    let config_contents = std::fs::read_to_string(cni_config_file.path()).unwrap();
    let mut cmd = Command::cargo_bin("cni-gesprek").unwrap();
    cmd.env("CNI_COMMAND", "CHECK")
        .env("CNI_CONTAINERID", "test-container-check-123")
        .env("CNI_NETNS", "/proc/self/ns/net")
        .env("CNI_IFNAME", "eth0")
        .env("CNI_PATH", "/usr/lib/cni")
        .env("DRY_RUN", "true") // Use dry-run mode for testing
        .write_stdin(config_contents);

    let output = cmd.assert().success();

    // CHECK command should return empty response for valid configuration
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);

    // According to CNI spec, CHECK should return empty response on success
    assert_eq!(stdout.trim(), "");
}

#[test]
fn test_check_command_missing_interface() {
    let mut cni_config_file = NamedTempFile::new().unwrap();
    writeln!(
        cni_config_file,
        r#"{{
            "cniVersion": "1.0.0",
            "name": "test-check-missing",
            "type": "ipvlan",
            "master": "eth0",
            "ipam": {{
                "type": "host-local",
                "subnet": "2001:db8::/64"
            }}
        }}"#
    )
    .unwrap();

    let config_contents = std::fs::read_to_string(cni_config_file.path()).unwrap();
    let mut cmd = Command::cargo_bin("cni-gesprek").unwrap();
    cmd.env("CNI_COMMAND", "CHECK")
        .env("CNI_CONTAINERID", "missing-interface-container")
        .env("CNI_NETNS", "/proc/9999/ns/net")
        .env("CNI_IFNAME", "missing_interface")
        .env("CNI_PATH", "/usr/lib/cni")
        .write_stdin(config_contents);

    // Should fail when interface is missing in the specified netns
    cmd.assert().failure();
}

#[test]
fn test_check_command_invalid_cni_version() {
    let mut cni_config_file = NamedTempFile::new().unwrap();
    writeln!(
        cni_config_file,
        r#"{{
            "cniVersion": "0.3.0",
            "name": "test-check-invalid-version",
            "type": "ipvlan",
            "master": "eth0",
            "ipam": {{
                "type": "host-local",
                "subnet": "2001:db8::/64"
            }}
        }}"#
    )
    .unwrap();

    let config_contents = std::fs::read_to_string(cni_config_file.path()).unwrap();
    let mut cmd = Command::cargo_bin("cni-gesprek").unwrap();
    cmd.env("CNI_COMMAND", "CHECK")
        .env("CNI_CONTAINERID", "test-version-check")
        .env("CNI_NETNS", "/proc/1234/ns/net")
        .env("CNI_IFNAME", "eth0")
        .env("CNI_PATH", "/usr/lib/cni")
        .write_stdin(config_contents);

    // Should fail with unsupported CNI version
    cmd.assert().failure();
}

#[test]
fn test_check_command_missing_required_env_vars() {
    let mut cni_config_file = NamedTempFile::new().unwrap();
    writeln!(
        cni_config_file,
        r#"{{
            "cniVersion": "1.0.0",
            "name": "test-check-env",
            "type": "ipvlan",
            "master": "eth0"
        }}"#
    )
    .unwrap();

    let config_contents = std::fs::read_to_string(cni_config_file.path()).unwrap();
    let mut cmd = Command::cargo_bin("cni-gesprek").unwrap();
    cmd.env("CNI_COMMAND", "CHECK")
        // Missing CNI_CONTAINERID, CNI_NETNS, CNI_IFNAME
        .env("CNI_PATH", "/usr/lib/cni")
        .write_stdin(config_contents);

    // Should fail when required environment variables are missing
    cmd.assert().failure();
}

#[test]
fn test_del_and_check_integration_scenario() {
    let mut cni_config_file = NamedTempFile::new().unwrap();
    writeln!(
        cni_config_file,
        r#"{{
            "cniVersion": "1.0.0",
            "name": "test-integration",
            "type": "ipvlan",
            "master": "eth0",
            "ipam": {{
                "type": "host-local",
                "subnet": "2001:db8::/64",
                "gateway": "2001:db8::1"
            }}
        }}"#
    )
    .unwrap();

    // First, try CHECK on non-existent setup - should fail
    let config_contents = std::fs::read_to_string(cni_config_file.path()).unwrap();
    let mut check_cmd = Command::cargo_bin("cni-gesprek").unwrap();
    check_cmd
        .env("CNI_COMMAND", "CHECK")
        .env("CNI_CONTAINERID", "integration-test-container")
        .env("CNI_NETNS", "/proc/1234/ns/net")
        .env("CNI_IFNAME", "eth0")
        .env("CNI_PATH", "/usr/lib/cni")
        .write_stdin(config_contents);

    // This should fail since no interface was created yet
    check_cmd.assert().failure();

    // Then run DEL on non-existent setup - should succeed (idempotent)
    let config_contents2 = std::fs::read_to_string(cni_config_file.path()).unwrap();

    let mut del_cmd = Command::cargo_bin("cni-gesprek").unwrap();
    del_cmd
        .env("CNI_COMMAND", "DEL")
        .env("CNI_CONTAINERID", "integration-test-container")
        .env("CNI_NETNS", "/proc/1234/ns/net")
        .env("CNI_IFNAME", "eth0")
        .env("CNI_PATH", "/usr/lib/cni")
        .write_stdin(config_contents2);

    let output = del_cmd.assert().success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert_eq!(stdout.trim(), "");
}

#[test]
fn test_del_command_with_dry_run() {
    let mut cni_config_file = NamedTempFile::new().unwrap();
    writeln!(
        cni_config_file,
        r#"{{
            "cniVersion": "1.0.0",
            "name": "test-del-dry-run",
            "type": "ipvlan",
            "master": "eth0",
            "ipam": {{
                "type": "host-local",
                "subnet": "2001:db8::/64",
                "gateway": "2001:db8::1"
            }}
        }}"#
    )
    .unwrap();

    let config_contents = std::fs::read_to_string(cni_config_file.path()).unwrap();
    let mut cmd = Command::cargo_bin("cni-gesprek").unwrap();
    cmd.env("CNI_COMMAND", "DEL")
        .env("CNI_CONTAINERID", "test-container-del-dry")
        .env("CNI_NETNS", "/proc/1234/ns/net")
        .env("CNI_IFNAME", "eth0")
        .env("CNI_PATH", "/usr/lib/cni")
        .env("DRY_RUN", "true")
        .write_stdin(config_contents);

    let output = cmd.assert().success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);

    // DEL should return empty response for successful cleanup even in dry-run
    assert_eq!(stdout.trim(), "");
}

#[test]
fn test_check_command_with_dry_run() {
    let mut cni_config_file = NamedTempFile::new().unwrap();
    writeln!(
        cni_config_file,
        r#"{{
            "cniVersion": "1.0.0",
            "name": "test-check-dry-run",
            "type": "ipvlan",
            "master": "eth0",
            "ipam": {{
                "type": "host-local",
                "subnet": "2001:db8::/64",
                "gateway": "2001:db8::1"
            }}
        }}"#
    )
    .unwrap();

    let config_contents = std::fs::read_to_string(cni_config_file.path()).unwrap();
    let mut cmd = Command::cargo_bin("cni-gesprek").unwrap();
    cmd.env("CNI_COMMAND", "CHECK")
        .env("CNI_CONTAINERID", "test-container-check-dry")
        .env("CNI_NETNS", "/proc/1234/ns/net")
        .env("CNI_IFNAME", "eth0")
        .env("CNI_PATH", "/usr/lib/cni")
        .env("DRY_RUN", "true")
        .write_stdin(config_contents);

    let output = cmd.assert().success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);

    // CHECK should return empty response on success even in dry-run
    assert_eq!(stdout.trim(), "");
}
