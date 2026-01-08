use std::error::Error;
use std::net::Ipv6Addr;
use std::path::Path;

/// Represents IPv6 subnet information with proper types
#[derive(Debug, Clone, PartialEq)]
pub struct Ipv6Subnet {
    pub address: Ipv6Addr,
    pub prefix_length: u8,
}

impl Ipv6Subnet {
    pub fn new(address: Ipv6Addr, prefix_length: u8) -> Result<Self, Box<dyn Error>> {
        if prefix_length > 128 {
            return Err(format!("Invalid prefix length: {}", prefix_length).into());
        }
        Ok(Self {
            address,
            prefix_length,
        })
    }

    pub fn to_cidr_string(&self) -> String {
        format!("{}/{}", self.address, self.prefix_length)
    }
}

pub trait NetworkDriver {
    /// Detects the default upstream interface (usually via default route).
    fn detect_upstream_interface(&self) -> Result<String, Box<dyn Error>>;

    /// Finds the gateway address associated with the given interface's default route.
    fn get_interface_gateway(&self, ifname: &str) -> Result<Option<Ipv6Addr>, Box<dyn Error>>;

    /// Finds the first global scope IPv6 subnet on the given interface.
    /// Returns proper IPv6 subnet information with validation.
    fn get_interface_subnet(&self, ifname: &str) -> Result<Ipv6Subnet, Box<dyn Error>>;

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

    /// Configures the interface inside the namespace with proper IPv6 types:
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
        gateway: Option<&Ipv6Addr>,
    ) -> Result<(), Box<dyn Error>>;

    /// Deletes a network interface inside a specific network namespace.
    /// Should be idempotent - succeed even if interface doesn't exist.
    fn delete_interface_in_netns(
        &self,
        netns_path: &Path,
        ifname: &str,
    ) -> Result<(), Box<dyn Error>>;

    /// Checks if a network interface exists inside a specific network namespace.
    fn interface_exists_in_netns(
        &self,
        netns_path: &Path,
        ifname: &str,
    ) -> Result<bool, Box<dyn Error>>;
}
