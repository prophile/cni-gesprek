use std::path::Path;

use crate::cni::{CniConfig, CniContext};
use crate::driver::NetworkDriver;
use crate::error::{CniResult, CommandError};
use crate::orchestrator::{CniOrchestrator, ValidationStep};
use crate::output::OutputWriter;

/// CLI Arguments structure (replicated to avoid circular dependencies)
#[derive(Clone)]
pub struct Args {
    pub interface: Option<String>,
    pub pod_cidr: Option<String>,
    pub dry_run: bool,
}

/// CommandContext consolidates all common parameters passed to CNI command handlers
/// This eliminates parameter duplication and makes it easier to add new context data
#[derive(Clone)]
pub struct CommandContext<'a> {
    pub args: Args,
    pub cni: CniContext,
    pub output: &'a dyn OutputWriter,
}

impl<'a> CommandContext<'a> {
    pub fn new(args: Args, cni: CniContext, output: &'a dyn OutputWriter) -> Self {
        Self { args, cni, output }
    }

    /// Helper to get config and handle errors consistently
    pub fn require_config(&self) -> CniResult<&CniConfig> {
        self.cni.require_config()
    }

    /// Helper to determine master interface with fallback logic
    pub fn get_master_interface(&self, driver: &dyn NetworkDriver) -> CniResult<String> {
        if let Some(interface) = &self.args.interface {
            return Ok(interface.clone());
        }

        if let Some(config) = &self.cni.config {
            if let Some(master) = &config.master {
                return Ok(master.clone());
            }
        }

        // Auto-detect first available interface
        driver.detect_upstream_interface().map_err(|e| e.into())
    }
}

/// Command dispatcher routes CNI commands to appropriate handlers
///
/// This separates command routing logic from the main application flow,
/// making it easier to test individual commands and add new command types.
pub struct CommandDispatcher<'a> {
    output: &'a dyn OutputWriter,
}

impl<'a> CommandDispatcher<'a> {
    pub fn new(output: &'a dyn OutputWriter) -> Self {
        Self { output }
    }

    /// Dispatch a CNI command to its appropriate handler
    ///
    /// Returns an error if the command is unknown or if the handler fails.
    pub fn dispatch(
        &self,
        command: &str,
        args: &Args,
        cni_context: &CniContext,
        driver: &dyn NetworkDriver,
    ) -> CniResult<()> {
        let ctx = CommandContext::new(args.clone(), cni_context.clone(), self.output);

        match command {
            "ADD" => self.cmd_add(&ctx, driver),
            "DEL" => self.cmd_del(&ctx, driver),
            "CHECK" => self.cmd_check(&ctx, driver),
            "GC" => self.cmd_success(&ctx),
            "VERSION" => self.cmd_version(&ctx),
            "STATUS" => self.cmd_status(&ctx, driver),
            _ => {
                self.output
                    .write_error(&format!("Unknown CNI_COMMAND: {}", command))?;
                Err(CommandError::UnknownCommand {
                    command: command.to_string(),
                }
                .into())
            }
        }
    }

    /// Get list of supported CNI commands
    pub fn supported_commands() -> &'static [&'static str] {
        &["ADD", "DEL", "CHECK", "GC", "VERSION", "STATUS"]
    }

    /// Check if a command is supported
    pub fn is_supported_command(command: &str) -> bool {
        Self::supported_commands().contains(&command)
    }
}

/// Command routing configuration for testability
#[derive(Debug, Clone)]
pub struct CommandConfig {
    pub allow_unknown_commands: bool,
    pub exit_on_error: bool,
}

impl Default for CommandConfig {
    fn default() -> Self {
        Self {
            allow_unknown_commands: false,
            exit_on_error: true,
        }
    }
}

impl<'a> CommandDispatcher<'a> {
    /// Dispatch command with configuration options for testing
    pub fn dispatch_with_config(
        &self,
        command: &str,
        args: &Args,
        cni_context: &CniContext,
        driver: &dyn NetworkDriver,
        config: &CommandConfig,
    ) -> CniResult<()> {
        if !Self::is_supported_command(command) && !config.allow_unknown_commands {
            let error_msg = format!("Unknown CNI_COMMAND: {}", command);
            self.output.write_error(&error_msg)?;

            if config.exit_on_error {
                std::process::exit(1);
            } else {
                return Err(CommandError::UnknownCommand {
                    command: command.to_string(),
                }
                .into());
            }
        }

        self.dispatch(command, args, cni_context, driver)
    }

    // Command handler implementations

    fn cmd_add(&self, ctx: &CommandContext, driver: &dyn NetworkDriver) -> CniResult<()> {
        let config = ctx.require_config()?;
        let netns_str = ctx.cni.environment.get_netns()?;
        let netns_path = Path::new(netns_str);
        let ifname = ctx.cni.environment.get_ifname()?;

        // 1. Determine Host Interface
        let master_interface = ctx.get_master_interface(driver)?;
        driver.check_interface(&master_interface)?;

        // 2. Get driver subnet info if needed
        let driver_subnet = if ctx.args.pod_cidr.is_none() && config.pod_cidr.is_none() {
            let subnet = driver.get_interface_subnet(&master_interface)?;
            Some((subnet.address.to_string(), subnet.prefix_length))
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
            return Err(e.into());
        }

        // 7. Configure Inside Netns
        if let Err(e) = driver.configure_in_netns(
            netns_path,
            &network_config.temporary_name,
            ifname,
            &network_config.target_ip,
            network_config.gateway.as_ref(),
        ) {
            return Err(e.into());
        }

        // 8. Output Result (business logic creates the structure)
        let result = CniOrchestrator::create_add_result(config, &network_config, netns_str);
        ctx.output.write_output(&serde_json::to_string(&result)?)?;

        Ok(())
    }

    fn cmd_del(&self, ctx: &CommandContext, driver: &dyn NetworkDriver) -> CniResult<()> {
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

    fn cmd_check(&self, ctx: &CommandContext, driver: &dyn NetworkDriver) -> CniResult<()> {
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
                        return Err(CommandError::NetnsNotFound {
                            netns: netns_str.to_string(),
                        }
                        .into());
                    }
                }
                ValidationStep::CheckMasterInterface(ref interface) => {
                    driver.check_interface(interface)?;
                }
                ValidationStep::CheckTargetInterface(ref interface) => {
                    if !driver.interface_exists_in_netns(netns_path, interface)? {
                        return Err(CommandError::InterfaceNotFound {
                            interface: interface.clone(),
                            netns: netns_str.to_string(),
                        }
                        .into());
                    }
                }
            }
        }

        // All checks passed - return success (empty response)
        Ok(())
    }

    fn cmd_version(&self, ctx: &CommandContext) -> CniResult<()> {
        let response = CniOrchestrator::create_version_response();
        ctx.output.write_output(response)?;
        Ok(())
    }

    fn cmd_status(&self, ctx: &CommandContext, driver: &dyn NetworkDriver) -> CniResult<()> {
        let master_interface = match ctx.get_master_interface(driver) {
            Ok(i) => i,
            Err(_) => std::process::exit(1),
        };

        driver.check_interface(&master_interface)?;
        self.cmd_success(ctx)
    }

    fn cmd_success(&self, ctx: &CommandContext) -> CniResult<()> {
        let result = CniOrchestrator::create_success_result();
        ctx.output.write_output(&serde_json::to_string(&result)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cni::CniContext;
    use crate::driver_dryrun::DryRunDriver;
    use crate::environment::MockEnvironmentProvider;
    use crate::output::MockOutputWriter;

    fn create_test_context() -> (MockOutputWriter, CniContext, DryRunDriver) {
        let mut env_provider = MockEnvironmentProvider::new();
        env_provider
            .set("CNI_COMMAND", "VERSION")
            .set("CNI_CONTAINERID", "test-container")
            .set("CNI_NETNS", "/var/run/netns/test")
            .set("CNI_IFNAME", "eth0");

        let cni_context = CniContext::load(&env_provider, false).unwrap();
        let output = MockOutputWriter::new();
        let driver = DryRunDriver;

        (output, cni_context, driver)
    }

    #[test]
    fn test_supported_commands() {
        let commands = CommandDispatcher::supported_commands();
        assert!(commands.contains(&"ADD"));
        assert!(commands.contains(&"DEL"));
        assert!(commands.contains(&"CHECK"));
        assert!(commands.contains(&"GC"));
        assert!(commands.contains(&"VERSION"));
        assert!(commands.contains(&"STATUS"));
    }

    #[test]
    fn test_is_supported_command() {
        assert!(CommandDispatcher::is_supported_command("ADD"));
        assert!(CommandDispatcher::is_supported_command("VERSION"));
        assert!(!CommandDispatcher::is_supported_command("UNKNOWN"));
        assert!(!CommandDispatcher::is_supported_command(""));
    }

    #[test]
    fn test_dispatch_version_command() {
        let (output, cni_context, driver) = create_test_context();
        let dispatcher = CommandDispatcher::new(&output);
        let args = Args {
            interface: None,
            pod_cidr: None,
            dry_run: false,
        };

        let result = dispatcher.dispatch("VERSION", &args, &cni_context, &driver);
        assert!(result.is_ok());

        let output_lines = output.get_output();
        assert!(!output_lines.is_empty());
        assert!(output_lines[0].contains("cniVersion"));
    }

    #[test]
    fn test_dispatch_unknown_command() {
        let (output, cni_context, driver) = create_test_context();
        let dispatcher = CommandDispatcher::new(&output);
        let args = Args {
            interface: None,
            pod_cidr: None,
            dry_run: false,
        };

        let config = CommandConfig {
            allow_unknown_commands: false,
            exit_on_error: false,
        };

        let result =
            dispatcher.dispatch_with_config("UNKNOWN", &args, &cni_context, &driver, &config);
        assert!(result.is_err());

        let error_lines = output.get_errors();
        assert!(!error_lines.is_empty());
        assert!(error_lines[0].contains("Unknown CNI_COMMAND"));
    }

    #[test]
    fn test_command_config_default() {
        let config = CommandConfig::default();
        assert!(!config.allow_unknown_commands);
        assert!(config.exit_on_error);
    }
}
