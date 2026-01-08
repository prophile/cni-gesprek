pub mod cni;
pub mod driver;
pub mod driver_dryrun;
pub mod driver_script;
pub mod environment;
pub mod output;

pub mod testing;

// Utility functions
pub mod utils {
    use rand::Rng;
    use std::error::Error;
    use std::net::Ipv6Addr;
    use std::str::FromStr;

    pub fn generate_random_ip(cidr: &str) -> Result<String, Box<dyn Error>> {
        let parts: Vec<&str> = cidr.split('/').collect();
        if parts.len() != 2 {
            return Err("Invalid CIDR format".into());
        }

        let base_ip = Ipv6Addr::from_str(parts[0])?;
        let prefix_len: u8 = parts[1].parse()?;

        if prefix_len > 128 {
            return Err("Invalid prefix length".into());
        }

        let base_u128 = u128::from(base_ip);

        let mask = if prefix_len == 0 {
            0
        } else {
            (0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFu128).wrapping_shl(128 - prefix_len as u32)
        };

        let mut rng = rand::thread_rng();
        let random_bits = rng.gen::<u128>() & !mask;
        let final_ip_u128 = (base_u128 & mask) | random_bits;

        let final_ip = Ipv6Addr::from(final_ip_u128);
        Ok(format!("{}/{}", final_ip, prefix_len))
    }
}
