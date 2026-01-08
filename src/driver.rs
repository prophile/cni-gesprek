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
}

/// High-level network discovery and introspection operations.
/// These operations query existing network state without modifying it.
pub trait NetworkDiscovery {
    /// Detects the default upstream interface (usually via default route).
    /// Returns the interface name that has the default IPv6 route.
    fn detect_upstream_interface(&self) -> Result<String, Box<dyn Error>>;

    /// Finds the gateway address associated with the given interface's default route.
    /// Returns None if no gateway is configured for this interface.
    fn get_interface_gateway(&self, ifname: &str) -> Result<Option<Ipv6Addr>, Box<dyn Error>>;

    /// Finds the first global scope IPv6 subnet on the given interface.
    /// Returns proper IPv6 subnet information with validation.
    fn get_interface_subnet(&self, ifname: &str) -> Result<Ipv6Subnet, Box<dyn Error>>;

    /// Checks if a specific interface exists and is UP.
    /// Returns Ok(()) if interface exists and is operational, Err otherwise.
    fn check_interface(&self, ifname: &str) -> Result<(), Box<dyn Error>>;
}

/// Low-level interface lifecycle management operations.
/// These operations create, modify, or destroy network interfaces.
pub trait InterfaceLifecycle {
    /// Creates an ipvlan L2 bridge interface on the host.
    /// The interface will be created as a child of the parent interface.
    fn create_ipvlan(
        &self,
        parent: &str,
        mode: &str,
        temp_name: &str,
    ) -> Result<(), Box<dyn Error>>;

    /// Deletes a network interface by name.
    /// Should be idempotent - succeeds even if interface doesn't exist.
    fn delete_interface(&self, ifname: &str) -> Result<(), Box<dyn Error>>;
}

/// Network namespace operations for container isolation.
/// These operations move interfaces between namespaces and configure them.
pub trait NetworkNamespaceOps {
    /// Moves a network interface into a specific network namespace.
    /// The interface will no longer be visible in the current namespace.
    fn set_netns(&self, ifname: &str, netns_path: &Path) -> Result<(), Box<dyn Error>>;

    /// Configures the interface inside the namespace with proper IPv6 types.
    /// Performs the complete setup sequence:
    /// 1. Rename to target name
    /// 2. Add the specific IPv6 Address
    /// 3. Set UP
    /// 4. Enable accept_ra
    /// 5. Add default gateway (if provided)
    /// 6. Strict DAD check (fails on collision)
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
    /// Returns true if the interface exists, false otherwise.
    fn interface_exists_in_netns(
        &self,
        netns_path: &Path,
        ifname: &str,
    ) -> Result<bool, Box<dyn Error>>;
}

/// Comprehensive network driver that combines all network operations.
/// This trait provides a unified interface for high-level CNI operations
/// by composing the focused traits above.
pub trait NetworkDriver: NetworkDiscovery + InterfaceLifecycle + NetworkNamespaceOps {
    // This trait intentionally left empty as it combines the other traits.
    // All functionality is provided through the composed traits:
    // - NetworkDiscovery for querying network state
    // - InterfaceLifecycle for managing interface creation/deletion
    // - NetworkNamespaceOps for namespace-aware operations
}

/// Convenience implementations for any type that implements all component traits
impl<T> NetworkDriver for T where T: NetworkDiscovery + InterfaceLifecycle + NetworkNamespaceOps {}

#[cfg(test)]
mod trait_abstraction_tests {
    use super::*;
    use std::error::Error;
    use std::net::Ipv6Addr;
    use std::path::Path;

    /// Mock implementation that only provides network discovery
    struct DiscoveryOnlyDriver;

    impl NetworkDiscovery for DiscoveryOnlyDriver {
        fn detect_upstream_interface(&self) -> Result<String, Box<dyn Error>> {
            Ok("mock_eth0".to_string())
        }

        fn get_interface_gateway(&self, _ifname: &str) -> Result<Option<Ipv6Addr>, Box<dyn Error>> {
            Ok(Some("fe80::1".parse().unwrap()))
        }

        fn get_interface_subnet(&self, _ifname: &str) -> Result<Ipv6Subnet, Box<dyn Error>> {
            let addr = "2001:db8::1".parse().unwrap();
            Ipv6Subnet::new(addr, 64)
        }

        fn check_interface(&self, _ifname: &str) -> Result<(), Box<dyn Error>> {
            Ok(())
        }
    }

    /// Mock implementation that only provides interface lifecycle
    struct LifecycleOnlyDriver;

    impl InterfaceLifecycle for LifecycleOnlyDriver {
        fn create_ipvlan(
            &self,
            _parent: &str,
            _mode: &str,
            _temp_name: &str,
        ) -> Result<(), Box<dyn Error>> {
            Ok(())
        }

        fn delete_interface(&self, _ifname: &str) -> Result<(), Box<dyn Error>> {
            Ok(())
        }
    }

    /// Mock implementation that only provides namespace operations
    struct NamespaceOnlyDriver;

    impl NetworkNamespaceOps for NamespaceOnlyDriver {
        fn set_netns(&self, _ifname: &str, _netns_path: &Path) -> Result<(), Box<dyn Error>> {
            Ok(())
        }

        fn configure_in_netns(
            &self,
            _netns_path: &Path,
            _temp_ifname: &str,
            _target_ifname: &str,
            _ip_cidr: &str,
            _gateway: Option<&Ipv6Addr>,
        ) -> Result<(), Box<dyn Error>> {
            Ok(())
        }

        fn delete_interface_in_netns(
            &self,
            _netns_path: &Path,
            _ifname: &str,
        ) -> Result<(), Box<dyn Error>> {
            Ok(())
        }

        fn interface_exists_in_netns(
            &self,
            _netns_path: &Path,
            _ifname: &str,
        ) -> Result<bool, Box<dyn Error>> {
            Ok(false)
        }
    }

    /// Complete mock driver that implements all traits
    struct CompleteDriver;

    impl NetworkDiscovery for CompleteDriver {
        fn detect_upstream_interface(&self) -> Result<String, Box<dyn Error>> {
            Ok("complete_eth0".to_string())
        }

        fn get_interface_gateway(&self, _ifname: &str) -> Result<Option<Ipv6Addr>, Box<dyn Error>> {
            Ok(Some("fe80::2".parse().unwrap()))
        }

        fn get_interface_subnet(&self, _ifname: &str) -> Result<Ipv6Subnet, Box<dyn Error>> {
            let addr = "2001:db8:1::1".parse().unwrap();
            Ipv6Subnet::new(addr, 64)
        }

        fn check_interface(&self, _ifname: &str) -> Result<(), Box<dyn Error>> {
            Ok(())
        }
    }

    impl InterfaceLifecycle for CompleteDriver {
        fn create_ipvlan(
            &self,
            _parent: &str,
            _mode: &str,
            _temp_name: &str,
        ) -> Result<(), Box<dyn Error>> {
            Ok(())
        }

        fn delete_interface(&self, _ifname: &str) -> Result<(), Box<dyn Error>> {
            Ok(())
        }
    }

    impl NetworkNamespaceOps for CompleteDriver {
        fn set_netns(&self, _ifname: &str, _netns_path: &Path) -> Result<(), Box<dyn Error>> {
            Ok(())
        }

        fn configure_in_netns(
            &self,
            _netns_path: &Path,
            _temp_ifname: &str,
            _target_ifname: &str,
            _ip_cidr: &str,
            _gateway: Option<&Ipv6Addr>,
        ) -> Result<(), Box<dyn Error>> {
            Ok(())
        }

        fn delete_interface_in_netns(
            &self,
            _netns_path: &Path,
            _ifname: &str,
        ) -> Result<(), Box<dyn Error>> {
            Ok(())
        }

        fn interface_exists_in_netns(
            &self,
            _netns_path: &Path,
            _ifname: &str,
        ) -> Result<bool, Box<dyn Error>> {
            Ok(true)
        }
    }

    #[test]
    fn test_focused_trait_separation() {
        // Test that traits can be implemented independently
        let discovery = DiscoveryOnlyDriver;
        assert_eq!(discovery.detect_upstream_interface().unwrap(), "mock_eth0");

        let lifecycle = LifecycleOnlyDriver;
        lifecycle.create_ipvlan("eth0", "l2", "temp_vlan").unwrap();

        let namespace = NamespaceOnlyDriver;
        assert_eq!(
            namespace
                .interface_exists_in_netns(Path::new("/proc/1/ns/net"), "test")
                .unwrap(),
            false
        );
    }

    #[test]
    fn test_complete_driver_composition() {
        // Test that complete driver implements NetworkDriver automatically
        let driver = CompleteDriver;

        // Test through NetworkDriver trait bound
        fn use_network_driver<T: NetworkDriver>(driver: &T) -> Result<String, Box<dyn Error>> {
            driver.detect_upstream_interface()
        }

        let result = use_network_driver(&driver).unwrap();
        assert_eq!(result, "complete_eth0");
    }

    #[test]
    fn test_trait_composition_functionality() {
        let driver = CompleteDriver;

        // Test NetworkDiscovery methods
        assert_eq!(driver.detect_upstream_interface().unwrap(), "complete_eth0");
        assert_eq!(
            driver
                .get_interface_gateway("eth0")
                .unwrap()
                .unwrap()
                .to_string(),
            "fe80::2"
        );

        // Test InterfaceLifecycle methods
        driver.create_ipvlan("eth0", "l2", "test").unwrap();
        driver.delete_interface("test").unwrap();

        // Test NetworkNamespaceOps methods
        driver
            .set_netns("test", Path::new("/proc/1/ns/net"))
            .unwrap();
        assert_eq!(
            driver
                .interface_exists_in_netns(Path::new("/proc/1/ns/net"), "test")
                .unwrap(),
            true
        );
    }

    #[test]
    fn test_trait_object_flexibility() {
        let discovery: Box<dyn NetworkDiscovery> = Box::new(DiscoveryOnlyDriver);
        let lifecycle: Box<dyn InterfaceLifecycle> = Box::new(LifecycleOnlyDriver);
        let namespace: Box<dyn NetworkNamespaceOps> = Box::new(NamespaceOnlyDriver);

        // Test that trait objects work correctly
        assert!(discovery.detect_upstream_interface().is_ok());
        assert!(lifecycle.create_ipvlan("eth0", "l2", "test").is_ok());
        assert!(namespace
            .set_netns("test", Path::new("/proc/1/ns/net"))
            .is_ok());
    }

    #[test]
    fn test_ipv6_subnet_validation() {
        let addr = "2001:db8::1".parse().unwrap();

        // Valid subnet
        let subnet = Ipv6Subnet::new(addr, 64).unwrap();
        assert_eq!(subnet.address, addr);
        assert_eq!(subnet.prefix_length, 64);

        // Invalid subnet (prefix too large)
        let invalid_subnet = Ipv6Subnet::new(addr, 129);
        assert!(invalid_subnet.is_err());
        assert!(invalid_subnet
            .unwrap_err()
            .to_string()
            .contains("Invalid prefix length"));
    }
}
