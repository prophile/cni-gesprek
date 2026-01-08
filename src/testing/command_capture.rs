use crate::driver::NetworkDriver;
use std::cell::RefCell;
use std::error::Error;
use std::path::Path;
use std::rc::Rc;

/// A command that would be executed by the network driver
#[derive(Debug, Clone, PartialEq)]
pub struct CapturedCommand {
    pub program: String,
    pub args: Vec<String>,
    pub working_dir: Option<String>,
    pub description: String,
}

/// A driver that captures all commands instead of executing them
pub struct CommandCaptureDriver {
    commands: Rc<RefCell<Vec<CapturedCommand>>>,
}

impl CommandCaptureDriver {
    pub fn new() -> Self {
        Self {
            commands: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Get all captured commands
    pub fn captured_commands(&self) -> Vec<CapturedCommand> {
        self.commands.borrow().clone()
    }

    /// Clear captured commands
    pub fn clear_commands(&self) {
        self.commands.borrow_mut().clear();
    }

    /// Get a clone of the commands reference for sharing with test contexts
    pub fn commands_ref(&self) -> Rc<RefCell<Vec<CapturedCommand>>> {
        self.commands.clone()
    }

    fn capture_command(&self, program: &str, args: &[&str], description: &str) {
        let cmd = CapturedCommand {
            program: program.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            working_dir: None,
            description: description.to_string(),
        };
        self.commands.borrow_mut().push(cmd);
    }

    fn capture_nsenter_command(&self, netns_path: &Path, command: &[&str], description: &str) {
        let mut args = vec![
            format!("--net={}", netns_path.display()),
            "-F".to_string(),
            "--".to_string(),
        ];
        args.extend(command.iter().map(|s| s.to_string()));

        let cmd = CapturedCommand {
            program: "nsenter".to_string(),
            args,
            working_dir: None,
            description: description.to_string(),
        };
        self.commands.borrow_mut().push(cmd);
    }
}

impl Default for CommandCaptureDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkDriver for CommandCaptureDriver {
    fn detect_upstream_interface(&self) -> Result<String, Box<dyn Error>> {
        self.capture_command(
            "ip",
            &["-6", "-j", "route", "show", "default"],
            "Detect upstream interface via IPv6 default route",
        );
        // Return a predictable test interface
        Ok("test_eth0".to_string())
    }

    fn get_interface_gateway(&self, ifname: &str) -> Result<Option<String>, Box<dyn Error>> {
        self.capture_command(
            "ip",
            &["-6", "-j", "route", "show", "default", "dev", ifname],
            &format!("Get gateway for interface {}", ifname),
        );
        // Return a predictable test gateway
        Ok(Some("fe80::1".to_string()))
    }

    fn get_interface_subnet(&self, ifname: &str) -> Result<(String, u8), Box<dyn Error>> {
        self.capture_command(
            "ip",
            &["-j", "-6", "addr", "show", "dev", ifname, "scope", "global"],
            &format!("Get subnet for interface {}", ifname),
        );
        // Return a predictable test subnet
        Ok(("2001:db8::1".to_string(), 64))
    }

    fn check_interface(&self, ifname: &str) -> Result<(), Box<dyn Error>> {
        self.capture_command(
            "ip",
            &["link", "show", "dev", ifname],
            &format!("Check if interface {} exists", ifname),
        );
        Ok(())
    }

    fn create_ipvlan(
        &self,
        parent: &str,
        mode: &str,
        temp_name: &str,
    ) -> Result<(), Box<dyn Error>> {
        self.capture_command(
            "ip",
            &[
                "link", "add", "link", parent, "name", temp_name, "type", "ipvlan", "mode", mode,
                "bridge",
            ],
            &format!(
                "Create ipvlan {} on parent {} with mode {}",
                temp_name, parent, mode
            ),
        );
        Ok(())
    }

    fn delete_interface(&self, ifname: &str) -> Result<(), Box<dyn Error>> {
        self.capture_command(
            "ip",
            &["link", "delete", ifname],
            &format!("Delete interface {}", ifname),
        );
        Ok(())
    }

    fn set_netns(&self, ifname: &str, netns_path: &Path) -> Result<(), Box<dyn Error>> {
        self.capture_command(
            "ip",
            &[
                "link",
                "set",
                "dev",
                ifname,
                "netns",
                &netns_path.display().to_string(),
            ],
            &format!("Move interface {} to netns {:?}", ifname, netns_path),
        );
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
        // 1. Rename interface
        self.capture_nsenter_command(
            netns_path,
            &[
                "ip",
                "link",
                "set",
                "dev",
                temp_ifname,
                "name",
                target_ifname,
            ],
            &format!("Rename interface {} to {}", temp_ifname, target_ifname),
        );

        // 2. Add IP address
        self.capture_nsenter_command(
            netns_path,
            &["ip", "addr", "add", ip_cidr, "dev", target_ifname],
            &format!("Add IP {} to interface {}", ip_cidr, target_ifname),
        );

        // 3. Configure accept_ra
        let sysctl_key = format!("net.ipv6.conf.{}.accept_ra=2", target_ifname);
        self.capture_nsenter_command(
            netns_path,
            &["sysctl", "-w", &sysctl_key],
            &format!("Enable accept_ra on interface {}", target_ifname),
        );

        // 4. Set interface up
        self.capture_nsenter_command(
            netns_path,
            &["ip", "link", "set", "up", "dev", target_ifname],
            &format!("Bring up interface {}", target_ifname),
        );

        // 5. Add default gateway if provided
        if let Some(gw) = gateway {
            self.capture_nsenter_command(
                netns_path,
                &[
                    "ip",
                    "-6",
                    "route",
                    "add",
                    "default",
                    "via",
                    gw,
                    "dev",
                    target_ifname,
                ],
                &format!("Add default route via {} on {}", gw, target_ifname),
            );
        }

        // 6. DAD check
        self.capture_nsenter_command(
            netns_path,
            &["ip", "-j", "-6", "addr", "show", "dev", target_ifname],
            &format!("Check DAD status on interface {}", target_ifname),
        );

        Ok(())
    }

    fn delete_interface_in_netns(
        &self,
        netns_path: &Path,
        ifname: &str,
    ) -> Result<(), Box<dyn Error>> {
        self.capture_nsenter_command(
            netns_path,
            &["ip", "link", "delete", "dev", ifname],
            &format!("Delete interface {} in netns", ifname),
        );
        Ok(())
    }

    fn interface_exists_in_netns(
        &self,
        netns_path: &Path,
        ifname: &str,
    ) -> Result<bool, Box<dyn Error>> {
        self.capture_nsenter_command(
            netns_path,
            &["ip", "link", "show", "dev", ifname],
            &format!("Check if interface {} exists in netns", ifname),
        );
        // For testing purposes, always return true
        Ok(true)
    }
}

/// Helper functions for test assertions
impl CommandCaptureDriver {
    /// Check if any command matches the given program and args
    pub fn has_command(&self, program: &str, args: &[&str]) -> bool {
        self.commands.borrow().iter().any(|cmd| {
            cmd.program == program
                && cmd.args == args.iter().map(|s| s.to_string()).collect::<Vec<_>>()
        })
    }

    /// Check if any command contains the given arguments (partial match)
    pub fn has_command_containing(&self, program: &str, partial_args: &[&str]) -> bool {
        self.commands.borrow().iter().any(|cmd| {
            cmd.program == program
                && partial_args
                    .iter()
                    .all(|arg| cmd.args.contains(&arg.to_string()))
        })
    }

    /// Get all commands for a specific program
    pub fn commands_for_program(&self, program: &str) -> Vec<CapturedCommand> {
        self.commands
            .borrow()
            .iter()
            .filter(|cmd| cmd.program == program)
            .cloned()
            .collect()
    }

    /// Get the number of captured commands
    pub fn command_count(&self) -> usize {
        self.commands.borrow().len()
    }

    /// Check if a specific nsenter command was executed
    pub fn has_nsenter_command(&self, netns: &str, inner_command: &[&str]) -> bool {
        let expected_args: Vec<String> = std::iter::once(format!("--net={}", netns))
            .chain(std::iter::once("-F".to_string()))
            .chain(std::iter::once("--".to_string()))
            .chain(inner_command.iter().map(|s| s.to_string()))
            .collect();

        self.commands
            .borrow()
            .iter()
            .any(|cmd| cmd.program == "nsenter" && cmd.args == expected_args)
    }
}
