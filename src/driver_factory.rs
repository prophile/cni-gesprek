use crate::driver::NetworkDriver;
use crate::driver_dryrun::DryRunDriver;
use crate::driver_script::ScriptDriver;

/// Driver factory responsible for creating appropriate network drivers
/// based on configuration and runtime parameters
pub struct DriverFactory;

impl DriverFactory {
    /// Create a network driver based on the dry-run flag and optional driver override
    ///
    /// This separates driver selection logic from the main application flow,
    /// making it easier to test and configure different driver behaviors.
    pub fn create_driver(
        is_dry_run: bool,
        _override_driver: Option<&str>,
    ) -> Box<dyn NetworkDriver> {
        if is_dry_run {
            Box::new(DryRunDriver)
        } else {
            // In the future, override_driver could be used to select different
            // driver implementations (e.g., "script", "netlink", "mock")
            Box::new(ScriptDriver)
        }
    }

    /// Create a driver for testing purposes with explicit type selection
    pub fn create_test_driver(driver_type: DriverType) -> Box<dyn NetworkDriver> {
        match driver_type {
            DriverType::DryRun => Box::new(DryRunDriver),
            DriverType::Script => Box::new(ScriptDriver),
        }
    }
}

/// Available driver types for explicit driver selection
#[derive(Debug, Clone, PartialEq)]
pub enum DriverType {
    DryRun,
    Script,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_driver_dry_run() {
        let driver = DriverFactory::create_driver(true, None);
        // Verify driver was created by checking it's not None (using format to consume it)
        let _ = format!("{:p}", driver.as_ref());
    }

    #[test]
    fn test_create_driver_normal() {
        let driver = DriverFactory::create_driver(false, None);
        // Verify driver was created by checking it's not None (using format to consume it)
        let _ = format!("{:p}", driver.as_ref());
    }

    #[test]
    fn test_create_test_driver_variants() {
        let dry_run_driver = DriverFactory::create_test_driver(DriverType::DryRun);
        let script_driver = DriverFactory::create_test_driver(DriverType::Script);

        // Verify both drivers were created successfully by using them in format
        let _ = format!("{:p}", dry_run_driver.as_ref());
        let _ = format!("{:p}", script_driver.as_ref());
    }
}
