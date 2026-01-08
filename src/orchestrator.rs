use std::error::Error;
use std::net::Ipv6Addr;

use crate::cni::{CniConfig, CniDns};
use crate::utils;

use serde::{Deserialize, Serialize};

/// Core CNI operation results
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkConfiguration {
    pub interface_name: String,
    pub target_ip: String,
    pub gateway: Option<Ipv6Addr>,
    pub master_interface: String,
    pub temporary_name: String,
}

/// CNI operation result structures
#[derive(Serialize, Deserialize, Debug)]
pub struct CniResult {
    #[serde(rename = "cniVersion")]
    pub cni_version: String,
    pub interfaces: Vec<CniInterface>,
    pub ips: Vec<CniIp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns: Option<CniDns>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CniInterface {
    pub name: String,
    pub mac: String,
    pub sandbox: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CniIp {
    pub version: String,
    pub address: String,
    pub interface: usize,
}

/// Business logic orchestrator for CNI operations
///
/// This layer contains pure business logic without system dependencies,
/// making it easily testable and reusable across different drivers.
pub struct CniOrchestrator;

impl CniOrchestrator {
    /// Plan network configuration for ADD operation
    ///
    /// This is pure business logic that determines what network configuration
    /// should be applied without actually executing any system operations.
    pub fn plan_add_operation(
        config: &CniConfig,
        cli_pod_cidr: Option<&str>,
        master_interface: &str,
        interface_name: &str,
        driver_subnet: Option<(String, u8)>,
        gateway: Option<Ipv6Addr>,
    ) -> Result<NetworkConfiguration, Box<dyn Error>> {
        // Determine the CIDR to use
        let cidr_string = if let Some(cli_cidr) = cli_pod_cidr {
            cli_cidr.to_string()
        } else if let Some(conf_cidr) = &config.pod_cidr {
            conf_cidr.clone()
        } else if let Some((ip, pfx)) = driver_subnet {
            format!("{}/{}", ip, pfx)
        } else {
            return Err("No pod CIDR specified in config, CLI args, or master interface".into());
        };

        // Generate target IP within the subnet
        let target_ip = utils::generate_random_ip(&cidr_string)?;

        // Generate a temporary interface name
        let random_suffix: String = (0..8)
            .map(|_| {
                let idx = rand::random::<usize>() % 16;
                format!("{:x}", idx)
            })
            .collect();
        let temporary_name = format!("ipvl{}", random_suffix);

        Ok(NetworkConfiguration {
            interface_name: interface_name.to_string(),
            target_ip,
            gateway,
            master_interface: master_interface.to_string(),
            temporary_name,
        })
    }

    /// Create CNI result structure for ADD operation success
    pub fn create_add_result(
        config: &CniConfig,
        network_config: &NetworkConfiguration,
        netns_path: &str,
    ) -> CniResult {
        CniResult {
            cni_version: config.cni_version.clone(),
            interfaces: vec![CniInterface {
                name: network_config.interface_name.clone(),
                mac: "".to_string(),
                sandbox: netns_path.to_string(),
            }],
            ips: vec![CniIp {
                version: "6".to_string(),
                address: network_config.target_ip.clone(),
                interface: 0,
            }],
            dns: config.dns.clone(),
        }
    }

    /// Plan validation steps for CHECK operation
    ///
    /// Returns what should be validated without performing the actual checks.
    pub fn plan_check_operation(
        config: &CniConfig,
        interface_name: &str,
        master_interface: &str,
    ) -> Result<Vec<ValidationStep>, Box<dyn Error>> {
        // Validate CNI version
        if config.cni_version != "1.0.0" {
            return Err(format!("Unsupported CNI version: {}", config.cni_version).into());
        }

        Ok(vec![
            ValidationStep::NetnsExists,
            ValidationStep::MasterInterface(master_interface.to_string()),
            ValidationStep::TargetInterface(interface_name.to_string()),
        ])
    }

    /// Determine if DEL operation should proceed based on business logic
    ///
    /// DEL should be idempotent - succeed even if resources don't exist.
    pub fn should_proceed_with_del(netns_exists: bool, is_dry_run: bool) -> bool {
        // In dry-run mode, always proceed
        if is_dry_run {
            return true;
        }

        // If netns doesn't exist, there's nothing to clean up - this is success
        // but we don't need to proceed with interface deletion
        netns_exists
    }

    /// Create standard success response for operations without specific output
    pub fn create_success_result() -> CniResult {
        CniResult {
            cni_version: "1.0.0".to_string(),
            interfaces: vec![],
            ips: vec![],
            dns: None,
        }
    }

    /// Create VERSION response
    pub fn create_version_response() -> &'static str {
        r#"{"cniVersion": "1.0.0", "supportedVersions": ["1.0.0"]}"#
    }
}

/// Validation steps that should be performed during CHECK operation
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationStep {
    NetnsExists,
    MasterInterface(String),
    TargetInterface(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cni::CniConfig;

    fn create_test_config() -> CniConfig {
        CniConfig {
            cni_version: "1.0.0".to_string(),
            name: "test-network".to_string(),
            plugin_type: "cni-gesprek".to_string(),
            master: Some("eth0".to_string()),
            pod_cidr: Some("2001:db8::/64".to_string()),
            dns: None,
        }
    }

    #[test]
    fn test_plan_add_operation_with_config_cidr() {
        let config = create_test_config();
        let result = CniOrchestrator::plan_add_operation(
            &config,
            None,
            "eth0",
            "veth0",
            None,
            Some("2001:db8::1".parse().unwrap()),
        )
        .unwrap();

        assert_eq!(result.interface_name, "veth0");
        assert_eq!(result.master_interface, "eth0");
        assert_eq!(result.gateway, Some("2001:db8::1".parse().unwrap()));
        assert!(result.target_ip.starts_with("2001:db8:"));
        assert!(result.target_ip.ends_with("/64"));
        assert!(result.temporary_name.starts_with("ipvl"));
    }

    #[test]
    fn test_plan_add_operation_with_cli_cidr() {
        let config = create_test_config();
        let result = CniOrchestrator::plan_add_operation(
            &config,
            Some("2001:db8:1::/64"),
            "eth0",
            "veth0",
            None,
            None,
        )
        .unwrap();

        assert!(result.target_ip.starts_with("2001:db8:1:"));
    }

    #[test]
    fn test_plan_add_operation_with_driver_subnet() {
        let mut config = create_test_config();
        config.pod_cidr = None; // Remove config CIDR

        let result = CniOrchestrator::plan_add_operation(
            &config,
            None,
            "eth0",
            "veth0",
            Some(("2001:db8:2::".to_string(), 64)),
            None,
        )
        .unwrap();

        assert!(result.target_ip.starts_with("2001:db8:2:"));
    }

    #[test]
    fn test_plan_check_operation() {
        let config = create_test_config();
        let steps = CniOrchestrator::plan_check_operation(&config, "veth0", "eth0").unwrap();

        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0], ValidationStep::NetnsExists);
        assert_eq!(
            steps[1],
            ValidationStep::MasterInterface("eth0".to_string())
        );
        assert_eq!(
            steps[2],
            ValidationStep::TargetInterface("veth0".to_string())
        );
    }

    #[test]
    fn test_should_proceed_with_del() {
        // Dry run should always proceed
        assert!(CniOrchestrator::should_proceed_with_del(false, true));
        assert!(CniOrchestrator::should_proceed_with_del(true, true));

        // Non-dry run should proceed only if netns exists
        assert!(!CniOrchestrator::should_proceed_with_del(false, false));
        assert!(CniOrchestrator::should_proceed_with_del(true, false));
    }

    #[test]
    fn test_create_add_result() {
        let config = create_test_config();
        let network_config = NetworkConfiguration {
            interface_name: "veth0".to_string(),
            target_ip: "2001:db8::42/64".to_string(),
            gateway: Some("2001:db8::1".parse().unwrap()),
            master_interface: "eth0".to_string(),
            temporary_name: "ipvl12345".to_string(),
        };

        let result =
            CniOrchestrator::create_add_result(&config, &network_config, "/proc/123/ns/net");

        assert_eq!(result.cni_version, "1.0.0");
        assert_eq!(result.interfaces.len(), 1);
        assert_eq!(result.interfaces[0].name, "veth0");
        assert_eq!(result.ips.len(), 1);
        assert_eq!(result.ips[0].address, "2001:db8::42/64");
    }

    #[test]
    fn test_version_response() {
        let response = CniOrchestrator::create_version_response();
        assert!(response.contains("1.0.0"));
        assert!(response.contains("supportedVersions"));
    }
}
