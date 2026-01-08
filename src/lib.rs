pub mod cni;
pub mod command_dispatcher;
pub mod driver;
pub mod driver_dryrun;
pub mod driver_factory;
pub mod driver_script;
pub mod environment;
pub mod error;
pub mod orchestrator;
pub mod output;

pub mod testing;

// Utility functions
pub mod utils {
    use ipnet::Ipv6Net;
    use rand::Rng;
    use std::error::Error;
    use std::net::Ipv6Addr;
    use std::str::FromStr;

    /// Generate a random IPv6 address within the given CIDR subnet
    ///
    /// This function uses proper IPv6 network types to ensure generated addresses
    /// are always valid and within the specified subnet.
    pub fn generate_random_ip(cidr: &str) -> Result<String, Box<dyn Error>> {
        // Parse the CIDR using ipnet for proper validation
        let network = Ipv6Net::from_str(cidr)
            .map_err(|e| format!("Invalid CIDR format '{}': {}", cidr, e))?;

        // Get the network address and prefix length
        let network_addr = network.network();
        let prefix_len = network.prefix_len();

        // Handle special cases
        match prefix_len {
            128 => {
                // Single host, return the exact address
                return Ok(format!("{}/{}", network_addr, prefix_len));
            }
            0 => {
                // Entire IPv6 space, generate completely random address
                let mut rng = rand::thread_rng();
                let random_addr = Ipv6Addr::from(rng.gen::<u128>());
                return Ok(format!("{}/0", random_addr));
            }
            _ => {
                // Normal case: generate random address in subnet
            }
        }

        // Convert network address to u128 for bit manipulation
        let network_u128 = u128::from(network_addr);

        // Create mask for the network portion
        let host_bits = 128 - prefix_len as u32;
        let host_mask = if host_bits >= 128 {
            u128::MAX
        } else {
            (1u128 << host_bits) - 1
        };

        // Generate random bits for the host portion
        let mut rng = rand::thread_rng();
        let random_host_bits = rng.gen::<u128>() & host_mask;

        // Combine network portion with random host bits
        let final_ip_u128 = network_u128 | random_host_bits;
        let final_ip = Ipv6Addr::from(final_ip_u128);

        // Validate the generated address is within the network
        if !network.contains(&final_ip) {
            return Err(format!(
                "Generated address {} is not within network {}",
                final_ip, network
            )
            .into());
        }

        Ok(format!("{}/{}", final_ip, prefix_len))
    }

    /// Parse and validate a CIDR string, returning the network information
    pub fn parse_cidr(cidr: &str) -> Result<(Ipv6Addr, u8), Box<dyn Error>> {
        let network = Ipv6Net::from_str(cidr)
            .map_err(|e| format!("Invalid CIDR format '{}': {}", cidr, e))?;
        Ok((network.network(), network.prefix_len()))
    }

    /// Check if an IPv6 address is within a given CIDR subnet
    pub fn is_address_in_subnet(addr: &Ipv6Addr, cidr: &str) -> Result<bool, Box<dyn Error>> {
        let network = Ipv6Net::from_str(cidr)
            .map_err(|e| format!("Invalid CIDR format '{}': {}", cidr, e))?;
        Ok(network.contains(addr))
    }

    /// Extract network and host portions from an IPv6 address given a prefix length
    pub fn split_ipv6_address(
        addr: &Ipv6Addr,
        prefix_len: u8,
    ) -> Result<(Ipv6Addr, u128), Box<dyn Error>> {
        if prefix_len > 128 {
            return Err("Prefix length cannot be greater than 128".into());
        }

        let addr_u128 = u128::from(*addr);
        let host_bits = 128 - prefix_len as u32;

        // Create network mask
        let network_mask = if host_bits >= 128 {
            0
        } else {
            u128::MAX << host_bits
        };

        // Extract network and host portions
        let network_u128 = addr_u128 & network_mask;
        let host_u128 = addr_u128 & !network_mask;

        Ok((Ipv6Addr::from(network_u128), host_u128))
    }
}
