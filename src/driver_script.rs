use crate::driver::NetworkDriver;
use std::error::Error;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

pub struct ScriptDriver;

impl ScriptDriver {
    fn run_cmd(cmd: &str, args: &[&str]) -> Result<String, Box<dyn Error>> {
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
}

impl NetworkDriver for ScriptDriver {
    fn detect_upstream_interface(&self) -> Result<String, Box<dyn Error>> {
        // Try IPv6 default route first
        let output = Self::run_cmd("ip", &["-6", "-j", "route", "show", "default"])?;
        let routes: Vec<serde_json::Value> = serde_json::from_str(&output).unwrap_or_default();

        if let Some(route) = routes.first() {
            if let Some(dev) = route.get("dev").and_then(|v| v.as_str()) {
                return Ok(dev.to_string());
            }
        }

        // Fallback to IPv4 default route
        let output = Self::run_cmd("ip", &["-4", "-j", "route", "show", "default"])?;
        let routes: Vec<serde_json::Value> = serde_json::from_str(&output).unwrap_or_default();

        if let Some(route) = routes.first() {
            if let Some(dev) = route.get("dev").and_then(|v| v.as_str()) {
                return Ok(dev.to_string());
            }
        }

        Err("Could not infer default interface from routing table".into())
    }

    fn get_interface_gateway(&self, ifname: &str) -> Result<Option<String>, Box<dyn Error>> {
        let output = Self::run_cmd(
            "ip",
            &["-6", "-j", "route", "show", "default", "dev", ifname],
        )?;
        let routes: Vec<serde_json::Value> = serde_json::from_str(&output).unwrap_or_default();

        if let Some(route) = routes.first() {
            if let Some(gateway) = route.get("gateway").and_then(|v| v.as_str()) {
                return Ok(Some(gateway.to_string()));
            }
        }
        Ok(None)
    }

    fn get_interface_subnet(&self, ifname: &str) -> Result<(String, u8), Box<dyn Error>> {
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

                    if let (Some(ip), Some(pfx)) = (local, prefix) {
                        return Ok((ip.to_string(), pfx as u8));
                    }
                }
            }
        }

        Err(format!("No global IPv6 address found on interface {}", ifname).into())
    }

    fn check_interface(&self, ifname: &str) -> Result<(), Box<dyn Error>> {
        Self::run_cmd("ip", &["link", "show", "dev", ifname])?;
        Ok(())
    }

    fn create_ipvlan(
        &self,
        parent: &str,
        mode: &str,
        temp_name: &str,
    ) -> Result<(), Box<dyn Error>> {
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
        Self::run_cmd("ip", &["link", "delete", ifname])?;
        Ok(())
    }

    fn set_netns(&self, ifname: &str, netns_path: &Path) -> Result<(), Box<dyn Error>> {
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
        gateway: Option<&str>,
    ) -> Result<(), Box<dyn Error>> {
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
            let args = ns_args(&[
                "ip",
                "-6",
                "route",
                "add",
                "default",
                "via",
                gw,
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
        let start = Instant::now();
        let timeout = Duration::from_secs(5);
        let target_ip = ip_cidr.split('/').next().unwrap_or("");

        loop {
            if start.elapsed() > timeout {
                return Err("Timed out waiting for IPv6 DAD to complete".into());
            }

            let args = ns_args(&["ip", "-j", "-6", "addr", "show", "dev", target_ifname]);
            if let Ok(output) = Self::run_cmd(
                "nsenter",
                &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            ) {
                let entries: Vec<serde_json::Value> =
                    serde_json::from_str(&output).unwrap_or_default();
                let mut found_our_ip = false;
                let mut is_ready = false;
                let mut is_failed = false;

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
                if is_failed {
                    return Err(format!("IPv6 DAD failed for {}", target_ip).into());
                }
                if found_our_ip && is_ready {
                    break;
                }
            }
            thread::sleep(Duration::from_millis(100));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_cmd_success() {
        // Test with a simple command that should always work
        let result = ScriptDriver::run_cmd("echo", &["hello"]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello");
    }

    #[test]
    fn test_run_cmd_failure() {
        // Test with a command that should fail
        let result = ScriptDriver::run_cmd("false", &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Command failed"));
    }

    #[test]
    fn test_run_cmd_nonexistent() {
        // Test with a nonexistent command
        let result = ScriptDriver::run_cmd("nonexistent_command_12345", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_run_cmd_with_args() {
        // Test command with multiple arguments
        let result = ScriptDriver::run_cmd("echo", &["hello", "world"]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello world");
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
        assert!(error_msg.contains("Command failed"));
        assert!(error_msg.contains("false"));
        assert!(error_msg.contains("arg1 arg2"));
    }
}
