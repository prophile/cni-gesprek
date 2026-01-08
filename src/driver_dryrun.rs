use crate::driver::{InterfaceLifecycle, Ipv6Subnet, NetworkDiscovery, NetworkNamespaceOps};
use std::error::Error;
use std::net::Ipv6Addr;
use std::path::Path;

pub struct DryRunDriver;

impl DryRunDriver {
    fn log(&self, msg: &str) {
        eprintln!("[DRY-RUN] {}", msg);
    }
}

impl NetworkDiscovery for DryRunDriver {
    fn detect_upstream_interface(&self) -> Result<String, Box<dyn Error>> {
        self.log("Inferring default interface... (Simulated: eth0)");
        Ok("eth0".to_string())
    }

    fn get_interface_gateway(&self, ifname: &str) -> Result<Option<Ipv6Addr>, Box<dyn Error>> {
        self.log(&format!(
            "Checking gateway on {}... (Simulated: fe80::1)",
            ifname
        ));
        Ok(Some("fe80::1".parse().unwrap()))
    }

    fn get_interface_subnet(&self, ifname: &str) -> Result<Ipv6Subnet, Box<dyn Error>> {
        self.log(&format!(
            "Checking subnet on {}... (Simulated: 2001:db8::1/64)",
            ifname
        ));
        let addr = "2001:db8::1".parse().unwrap();
        Ipv6Subnet::new(addr, 64)
    }

    fn check_interface(&self, ifname: &str) -> Result<(), Box<dyn Error>> {
        self.log(&format!("Checking if interface {} exists...", ifname));
        Ok(())
    }
}

impl InterfaceLifecycle for DryRunDriver {
    fn create_ipvlan(
        &self,
        parent: &str,
        mode: &str,
        temp_name: &str,
    ) -> Result<(), Box<dyn Error>> {
        self.log(&format!(
            "ip link add link {} name {} type ipvlan mode {} bridge",
            parent, temp_name, mode
        ));
        Ok(())
    }

    fn delete_interface(&self, ifname: &str) -> Result<(), Box<dyn Error>> {
        self.log(&format!("ip link delete {}", ifname));
        Ok(())
    }
}

impl NetworkNamespaceOps for DryRunDriver {
    fn set_netns(&self, ifname: &str, netns_path: &Path) -> Result<(), Box<dyn Error>> {
        self.log(&format!(
            "ip link set dev {} netns {:?}",
            ifname, netns_path
        ));
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
        let ns = netns_path.display();
        self.log(&format!(
            "nsenter --net={} -F -- ip link set dev {} name {}",
            ns, temp_ifname, target_ifname
        ));
        self.log(&format!(
            "nsenter --net={} -F -- ip addr add {} dev {}",
            ns, ip_cidr, target_ifname
        ));
        self.log(&format!(
            "nsenter --net={} -F -- sysctl -w net.ipv6.conf.{}.accept_ra=2",
            ns, target_ifname
        ));
        self.log(&format!(
            "nsenter --net={} -F -- ip link set up dev {}",
            ns, target_ifname
        ));
        if let Some(gw) = gateway {
            self.log(&format!(
                "nsenter --net={} -F -- ip -6 route add default via {} dev {}",
                ns, gw, target_ifname
            ));
        }
        self.log(&format!(
            "nsenter --net={} -F -- ip -j -6 addr show (Wait for DAD)",
            ns
        ));
        Ok(())
    }

    fn delete_interface_in_netns(
        &self,
        netns_path: &Path,
        ifname: &str,
    ) -> Result<(), Box<dyn Error>> {
        let ns = netns_path.display();
        self.log(&format!(
            "nsenter --net={} -F -- ip link delete dev {}",
            ns, ifname
        ));
        Ok(())
    }

    fn interface_exists_in_netns(
        &self,
        netns_path: &Path,
        ifname: &str,
    ) -> Result<bool, Box<dyn Error>> {
        let ns = netns_path.display();
        self.log(&format!(
            "nsenter --net={} -F -- ip link show dev {} (Simulated: exists)",
            ns, ifname
        ));
        Ok(true) // In dry-run mode, assume interface exists for testing
    }
}
