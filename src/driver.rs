use std::error::Error;
use std::path::Path;

pub trait NetworkDriver {
    /// Detects the default upstream interface (usually via default route).
    fn detect_upstream_interface(&self) -> Result<String, Box<dyn Error>>;

    /// Finds the gateway address associated with the given interface's default route.
    fn get_interface_gateway(&self, ifname: &str) -> Result<Option<String>, Box<dyn Error>>;

    /// Finds the first global scope IPv6 subnet on the given interface.
    /// Returns a tuple (IP, PrefixLen), e.g., ("2001:db8::1", 64).
    fn get_interface_subnet(&self, ifname: &str) -> Result<(String, u8), Box<dyn Error>>;

    /// Checks if a specific interface exists and is UP.
    fn check_interface(&self, ifname: &str) -> Result<(), Box<dyn Error>>;

    /// Creates an ipvlan L2 bridge interface on the host.
    fn create_ipvlan(
        &self,
        parent: &str,
        mode: &str,
        temp_name: &str,
    ) -> Result<(), Box<dyn Error>>;

    /// Deletes a network interface by name.
    fn delete_interface(&self, ifname: &str) -> Result<(), Box<dyn Error>>;

    /// Moves a network interface into a specific network namespace.
    fn set_netns(&self, ifname: &str, netns_path: &Path) -> Result<(), Box<dyn Error>>;

    /// Configures the interface inside the namespace:
    /// 1. Rename to target name.
    /// 2. Add the specific IPv6 Address.
    /// 3. Set UP.
    /// 4. Enable accept_ra.
    /// 5. Add default gateway (if provided).
    /// 6. Strict DAD check (fails on collision).
    fn configure_in_netns(
        &self,
        netns_path: &Path,
        temp_ifname: &str,
        target_ifname: &str,
        ip_cidr: &str,
        gateway: Option<&str>,
    ) -> Result<(), Box<dyn Error>>;
}
