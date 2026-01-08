mod cni;
mod driver;
mod driver_dryrun;
mod driver_script;
mod environment;
mod orchestrator;
mod output;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

use cni_gesprek::utils;

use clap::Parser;
use cni::{CniConfig, CniContext, CniDns};
use driver::NetworkDriver;
use driver_dryrun::DryRunDriver;
use driver_script::ScriptDriver;
use environment::SystemEnvironmentProvider;
use orchestrator::{CniInterface, CniIp, CniOrchestrator, CniResult, ValidationStep};
use output::{OutputWriter, StandardOutputWriter};
use serde_json;
use std::error::Error;
use std::path::Path;

// --- CLI Arguments ---

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    interface: Option<String>,

    #[arg(long)]
    pod_cidr: Option<String>,

    #[arg(long)]
    dry_run: bool,
}

// --- Command Context ---

/// CommandContext consolidates all common parameters passed to CNI command handlers
/// This eliminates parameter duplication and makes it easier to add new context data
struct CommandContext<'a> {
    args: Args,
    cni: CniContext,
    output: &'a dyn OutputWriter,
}

impl<'a> CommandContext<'a> {
    /// Create a new CommandContext
    fn new(args: Args, cni: CniContext, output: &'a dyn OutputWriter) -> Self {
        Self { args, cni, output }
    }

    /// Get the master interface, preferring CLI args over config
    fn get_master_interface(&self, driver: &dyn NetworkDriver) -> Result<String, Box<dyn Error>> {
        if let Some(cli_iface) = &self.args.interface {
            Ok(cli_iface.clone())
        } else if let Some(conf_master) = self.cni.config.as_ref().and_then(|c| c.master.as_ref()) {
            Ok(conf_master.clone())
        } else {
            driver.detect_upstream_interface()
        }
    }

    /// Ensure configuration is available, returning an error if missing
    fn require_config(&self) -> Result<&CniConfig, Box<dyn Error>> {
        self.cni.require_config()
    }
}

// --- Helpers ---

// --- Main Logic ---

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let env_provider = SystemEnvironmentProvider::default();

    // Load CNI context (environment + configuration parsing)
    let cni_context = CniContext::load(&env_provider, args.dry_run).map_err(|e| {
        let output_writer = StandardOutputWriter;
        let _ = output_writer.write_error(&format!("CNI context loading error: {}", e));
        e
    })?;

    // Determine driver based on dry run status
    let driver: Box<dyn NetworkDriver> = if cni_context.is_dry_run {
        Box::new(DryRunDriver)
    } else {
        Box::new(ScriptDriver)
    };

    let output_writer = StandardOutputWriter;
    let ctx = CommandContext::new(args, cni_context, &output_writer);

    match ctx.cni.environment.command.as_str() {
        "ADD" => cmd_add(&ctx, &*driver),
        "DEL" => cmd_del(&ctx, &*driver),
        "CHECK" => cmd_check(&ctx, &*driver),
        "GC" => cmd_success(&ctx),
        "VERSION" => cmd_version(&ctx),
        "STATUS" => cmd_status(&ctx, &*driver),
        _ => {
            ctx.output.write_error(&format!(
                "Unknown CNI_COMMAND: {}",
                ctx.cni.environment.command
            ))?;
            std::process::exit(1);
        }
    }
}

fn cmd_add(ctx: &CommandContext, driver: &dyn NetworkDriver) -> Result<(), Box<dyn Error>> {
    let config = ctx.require_config()?;
    let netns_str = ctx.cni.environment.get_netns()?;
    let netns_path = Path::new(netns_str);
    let ifname = ctx.cni.environment.get_ifname()?;

    // 1. Determine Host Interface
    let master_interface = ctx.get_master_interface(driver)?;
    driver.check_interface(&master_interface)?;

    // 2. Get driver subnet info if needed
    let driver_subnet = if ctx.args.pod_cidr.is_none() && config.pod_cidr.is_none() {
        let (ip, pfx) = driver.get_interface_subnet(&master_interface)?;
        Some((ip, pfx))
    } else {
        None
    };

    // 3. Detect Default Gateway
    let gateway = driver
        .get_interface_gateway(&master_interface)
        .unwrap_or(None);

    // 4. Plan network configuration (business logic)
    let network_config = CniOrchestrator::plan_add_operation(
        config,
        ctx.args.pod_cidr.as_deref(),
        &master_interface,
        ifname,
        driver_subnet,
        gateway,
    )?;

    // 5. Execute system operations
    driver.create_ipvlan(&master_interface, "l2", &network_config.temporary_name)?;

    // 6. Move to Netns
    if let Err(e) = driver.set_netns(&network_config.temporary_name, netns_path) {
        let _ = driver.delete_interface(&network_config.temporary_name);
        return Err(e);
    }

    // 7. Configure Inside Netns
    if let Err(e) = driver.configure_in_netns(
        netns_path,
        &network_config.temporary_name,
        &ifname,
        &network_config.target_ip,
        network_config.gateway.as_deref(),
    ) {
        return Err(e);
    }

    // 8. Output Result (business logic creates the structure)
    let result = CniOrchestrator::create_add_result(config, &network_config, netns_str);
    ctx.output.write_output(&serde_json::to_string(&result)?)?;

    Ok(())
}

fn cmd_status(ctx: &CommandContext, driver: &dyn NetworkDriver) -> Result<(), Box<dyn Error>> {
    let master_interface = match ctx.get_master_interface(driver) {
        Ok(i) => i,
        Err(_) => std::process::exit(1),
    };

    driver.check_interface(&master_interface)?;
    cmd_success(ctx)
}

fn cmd_version(ctx: &CommandContext) -> Result<(), Box<dyn Error>> {
    let response = CniOrchestrator::create_version_response();
    ctx.output.write_output(response)?;
    Ok(())
}

fn cmd_del(ctx: &CommandContext, driver: &dyn NetworkDriver) -> Result<(), Box<dyn Error>> {
    // Get required environment variables from CniEnvironment
    let netns_str = ctx.cni.environment.get_netns()?;
    let _container_id = ctx.cni.environment.get_container_id()?;
    let ifname = ctx.cni.environment.get_ifname()?;

    let netns_path = Path::new(netns_str);
    let netns_exists = ctx.cni.is_dry_run || netns_path.exists();

    // Business logic: determine if we should proceed
    if !CniOrchestrator::should_proceed_with_del(netns_exists, ctx.cni.is_dry_run) {
        // Netns doesn't exist, nothing to clean up - this is success
        return Ok(());
    }

    // Execute system operation: delete interface
    match driver.delete_interface_in_netns(netns_path, ifname) {
        Ok(()) => Ok(()),
        Err(e) => {
            // Log the error but don't fail - DEL should be idempotent
            ctx.output
                .write_error(&format!("Warning during DEL: {}", e))?;
            Ok(())
        }
    }
}

fn cmd_check(ctx: &CommandContext, driver: &dyn NetworkDriver) -> Result<(), Box<dyn Error>> {
    let config = ctx.require_config()?;
    let netns_str = ctx.cni.environment.get_netns()?;
    let _container_id = ctx.cni.environment.get_container_id()?;
    let ifname = ctx.cni.environment.get_ifname()?;
    let netns_path = Path::new(netns_str);

    // Determine master interface
    let master_interface = ctx.get_master_interface(driver)?;

    // Plan validation steps (business logic)
    let validation_steps =
        CniOrchestrator::plan_check_operation(config, ifname, &master_interface)?;

    // Execute validation steps (system operations)
    for step in validation_steps {
        match step {
            ValidationStep::CheckNetnsExists => {
                if !ctx.cni.is_dry_run && !netns_path.exists() {
                    return Err(format!("Network namespace does not exist: {}", netns_str).into());
                }
            }
            ValidationStep::CheckMasterInterface(ref interface) => {
                driver.check_interface(interface)?;
            }
            ValidationStep::CheckTargetInterface(ref interface) => {
                if !driver.interface_exists_in_netns(netns_path, interface)? {
                    return Err(format!(
                        "Interface '{}' not found in network namespace '{}'",
                        interface, netns_str
                    )
                    .into());
                }
            }
        }
    }

    // All checks passed - return success (empty response)
    Ok(())
}

fn cmd_success(ctx: &CommandContext) -> Result<(), Box<dyn Error>> {
    let result = CniOrchestrator::create_success_result();
    ctx.output.write_output(&serde_json::to_string(&result)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::net::Ipv6Addr;
    use std::str::FromStr;

    #[test]
    fn test_generate_random_ip_basic() {
        let cidr = "2001:db8::/64";
        let result = utils::generate_random_ip(cidr).unwrap();

        // Should be in format IP/64
        assert!(result.contains('/'));
        let parts: Vec<&str> = result.split('/').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[1], "64");

        // IP should be parseable
        let ip = Ipv6Addr::from_str(parts[0]).unwrap();

        // Should start with the correct prefix (2001:db8)
        let ip_bytes = ip.octets();
        assert_eq!(ip_bytes[0], 0x20);
        assert_eq!(ip_bytes[1], 0x01);
        assert_eq!(ip_bytes[2], 0x0d);
        assert_eq!(ip_bytes[3], 0xb8);
    }

    #[test]
    fn test_generate_random_ip_edge_cases() {
        // Test /0 prefix (whole IPv6 space)
        let result = utils::generate_random_ip("2001:db8::1/0").unwrap();
        assert!(result.contains("/0"));

        // Test /128 prefix (single IP)
        let result = utils::generate_random_ip("2001:db8::1/128").unwrap();
        assert_eq!(result, "2001:db8::1/128");

        // Test /64 prefix (standard)
        let result = utils::generate_random_ip("2001:db8:1:2::/64").unwrap();
        assert!(result.contains("/64"));
        let parts: Vec<&str> = result.split('/').collect();
        let ip = Ipv6Addr::from_str(parts[0]).unwrap();
        let ip_bytes = ip.octets();

        // First 8 bytes should match prefix
        assert_eq!(ip_bytes[0], 0x20);
        assert_eq!(ip_bytes[1], 0x01);
        assert_eq!(ip_bytes[2], 0x0d);
        assert_eq!(ip_bytes[3], 0xb8);
        assert_eq!(ip_bytes[4], 0x00);
        assert_eq!(ip_bytes[5], 0x01);
        assert_eq!(ip_bytes[6], 0x00);
        assert_eq!(ip_bytes[7], 0x02);
    }

    #[test]
    fn test_generate_random_ip_randomness() {
        let cidr = "2001:db8::/64";
        let mut generated_ips = HashSet::new();

        // Generate multiple IPs and ensure they're different
        for _ in 0..10 {
            let ip = utils::generate_random_ip(cidr).unwrap();
            generated_ips.insert(ip);
        }

        // Should have generated different IPs (very high probability)
        assert!(generated_ips.len() > 1);
    }

    #[test]
    fn test_generate_random_ip_invalid_cidr() {
        // Invalid format
        assert!(utils::generate_random_ip("not-a-cidr").is_err());

        // Invalid IP
        assert!(utils::generate_random_ip("invalid-ip/64").is_err());

        // Invalid prefix length
        assert!(utils::generate_random_ip("2001:db8::/129").is_err());

        // No prefix length
        assert!(utils::generate_random_ip("2001:db8::").is_err());
    }

    #[test]
    fn test_generate_random_ip_prefix_preservation() {
        // Test various prefix lengths
        let test_cases = vec![
            ("2001:db8::/32", 32),
            ("2001:db8:1::/48", 48),
            ("2001:db8:1:2::/64", 64),
            ("2001:db8:1:2:3::/80", 80),
            ("2001:db8:1:2:3:4::/96", 96),
        ];

        for (cidr, prefix_len) in test_cases {
            let result = utils::generate_random_ip(cidr).unwrap();
            let result_parts: Vec<&str> = result.split('/').collect();
            let generated_ip = Ipv6Addr::from_str(result_parts[0]).unwrap();

            let original_parts: Vec<&str> = cidr.split('/').collect();
            let base_ip = Ipv6Addr::from_str(original_parts[0]).unwrap();

            // Check that the prefix bits are preserved
            let base_bytes = base_ip.octets();
            let gen_bytes = generated_ip.octets();

            let prefix_bytes = prefix_len / 8;
            let prefix_bits = prefix_len % 8;

            // Check full bytes
            for i in 0..prefix_bytes as usize {
                assert_eq!(
                    base_bytes[i], gen_bytes[i],
                    "Prefix mismatch at byte {} for {}",
                    i, cidr
                );
            }

            // Check partial byte if any
            if prefix_bits > 0 {
                let byte_idx = prefix_bytes as usize;
                let mask = 0xFF << (8 - prefix_bits);
                assert_eq!(
                    base_bytes[byte_idx] & mask,
                    gen_bytes[byte_idx] & mask,
                    "Prefix mismatch in partial byte for {}",
                    cidr
                );
            }
        }
    }

    #[test]
    fn test_cni_config_deserialization() {
        let json = r#"{
            "cniVersion": "1.0.0",
            "name": "test-network",
            "type": "cni-gesprek",
            "master": "eth0",
            "podCIDR": "2001:db8::/64",
            "dns": {
                "nameservers": ["2001:db8::53"],
                "domain": "example.com"
            }
        }"#;

        let config: CniConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.cni_version, "1.0.0");
        assert_eq!(config.name, "test-network");
        assert_eq!(config.plugin_type, "cni-gesprek");
        assert_eq!(config.master, Some("eth0".to_string()));
        assert_eq!(config.pod_cidr, Some("2001:db8::/64".to_string()));

        let dns = config.dns.unwrap();
        assert_eq!(dns.nameservers.unwrap(), vec!["2001:db8::53"]);
        assert_eq!(dns.domain.unwrap(), "example.com");
    }

    #[test]
    fn test_cni_result_serialization() {
        let result = CniResult {
            cni_version: "1.0.0".to_string(),
            interfaces: vec![CniInterface {
                name: "eth0".to_string(),
                mac: "aa:bb:cc:dd:ee:ff".to_string(),
                sandbox: "/proc/123/ns/net".to_string(),
            }],
            ips: vec![CniIp {
                version: "6".to_string(),
                address: "2001:db8::1/64".to_string(),
                interface: 0,
            }],
            dns: Some(CniDns {
                nameservers: Some(vec!["2001:db8::53".to_string()]),
                domain: Some("example.com".to_string()),
                search: None,
                options: None,
            }),
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["cniVersion"], "1.0.0");
        assert_eq!(parsed["interfaces"][0]["name"], "eth0");
        assert_eq!(parsed["ips"][0]["address"], "2001:db8::1/64");
        assert_eq!(parsed["dns"]["nameservers"][0], "2001:db8::53");
    }

    #[test]
    fn test_args_parsing() {
        use clap::Parser;

        // Test with all arguments
        let args = Args::try_parse_from(&[
            "cni-gesprek",
            "--interface",
            "eth1",
            "--pod-cidr",
            "2001:db8:1::/64",
            "--dry-run",
        ])
        .unwrap();

        assert_eq!(args.interface, Some("eth1".to_string()));
        assert_eq!(args.pod_cidr, Some("2001:db8:1::/64".to_string()));
        assert!(args.dry_run);

        // Test with minimal arguments
        let args = Args::try_parse_from(&["cni-gesprek"]).unwrap();
        assert_eq!(args.interface, None);
        assert_eq!(args.pod_cidr, None);
        assert!(!args.dry_run);
    }

    #[test]
    fn test_ip_generation_within_subnet() {
        let cidr = "2001:db8:1:2::/64";

        for _ in 0..20 {
            let generated = utils::generate_random_ip(cidr).unwrap();
            let ip_part = generated.split('/').next().unwrap();
            let ip = Ipv6Addr::from_str(ip_part).unwrap();

            // Convert to u128 for easier subnet checking
            let ip_u128 = u128::from(ip);
            let base_u128 = u128::from(Ipv6Addr::from_str("2001:db8:1:2::").unwrap());

            // Check that it's in the correct /64 subnet
            // First 64 bits should be the same
            let subnet_mask = 0xFFFFFFFFFFFFFFFF_u128 << 64;
            assert_eq!(
                ip_u128 & subnet_mask,
                base_u128 & subnet_mask,
                "Generated IP {} not in subnet {}",
                ip,
                cidr
            );
        }
    }

    #[test]
    fn test_generate_random_ip_stress() {
        let cidr = "2001:db8::/96"; // Small host space for collision testing
        let mut ips = HashSet::new();

        // Generate many IPs and check for reasonable distribution
        for _ in 0..1000 {
            let ip = utils::generate_random_ip(cidr).unwrap();
            ips.insert(ip);
        }

        // With a /96 prefix (32 bits of host space), we should see good distribution
        // Even with 1000 attempts, we should see mostly unique IPs
        assert!(
            ips.len() > 900,
            "Poor randomness: only {} unique IPs out of 1000",
            ips.len()
        );
    }

    mod input_validation_tests {
        use super::*;

        #[test]
        fn test_interface_name_patterns() {
            // Test that we would accept valid interface names
            let valid_names = vec!["eth0", "ens3", "wlan0", "br0", "docker0", "veth1234"];

            for name in valid_names {
                // This is a placeholder - we'd test actual validation when implemented
                assert!(name.len() < 16); // Linux IFNAMSIZ is 16
                assert!(!name.is_empty());
                assert!(!name.contains('/'));
                assert!(!name.contains(' '));
            }
        }

        #[test]
        fn test_cidr_validation_patterns() {
            let test_cases = vec![
                ("2001:db8::/64", true),
                ("2001:db8:1:2:3:4:5:6/128", true),
                ("::/0", true),
                ("invalid", false),
                ("2001:db8::/129", false),
                ("2001:db8::/-1", false),
                ("2001:db8::", false),
            ];

            for (cidr, should_be_valid) in test_cases {
                let result = utils::generate_random_ip(cidr);
                if should_be_valid {
                    assert!(result.is_ok(), "Expected {} to be valid", cidr);
                } else {
                    assert!(result.is_err(), "Expected {} to be invalid", cidr);
                }
            }
        }
    }

    mod error_handling_tests {
        use super::*;

        #[test]
        fn test_error_propagation() {
            // Test that errors are properly propagated through the system
            let invalid_cidr = "not-a-cidr";
            let error = utils::generate_random_ip(invalid_cidr).unwrap_err();
            assert!(error.to_string().contains("Invalid CIDR format"));
        }

        #[test]
        fn test_box_dyn_error_compatibility() {
            // Ensure our errors work with Box<dyn Error>
            let result: Result<String, Box<dyn std::error::Error>> = Err("test error".into());
            assert!(result.is_err());
        }
    }

    #[cfg(feature = "testing")]
    mod integration_helpers {
        use super::*;
        use crate::testing::*;

        #[test]
        fn test_command_capture_integration() {
            let driver = CommandCaptureDriver::new();

            // Test that captured commands can be analyzed
            // This would test the integration between main logic and testing framework
            let captured = driver.get_captured_commands();
            assert!(captured.is_empty()); // Should start empty
        }

        #[test]
        fn test_test_scenario_building() {
            let (config, env) = TestScenarios::basic_add();
            let config_json = config.build_json_string();
            let env_vars = env.build();

            assert!(config_json.contains("cniVersion"));
            assert!(env_vars.contains_key("CNI_COMMAND"));
            assert_eq!(env_vars.get("CNI_COMMAND"), Some(&"ADD".to_string()));
        }
    }
}
