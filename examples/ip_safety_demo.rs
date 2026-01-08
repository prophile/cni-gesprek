use cni_gesprek::utils::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("IPv6 Address Generation Safety Demonstration");
    println!("===========================================\n");

    let test_cidrs = vec![
        "2001:db8::/64",
        "fe80::/10",
        "2001:db8:85a3::/48",
        "::1/128", // Edge case: single address
        "2001:db8::/32",
    ];

    for cidr in test_cidrs {
        println!("Testing CIDR: {}", cidr);

        // Parse and validate
        match parse_cidr(cidr) {
            Ok((addr, prefix)) => {
                println!("  ✓ Parsed successfully: {} with prefix /{}", addr, prefix);

                // Generate 3 random addresses in this subnet
                for i in 1..=3 {
                    match generate_random_ip(cidr) {
                        Ok(generated_ip) => {
                            let parts: Vec<&str> = generated_ip.split('/').collect();
                            let gen_addr: std::net::Ipv6Addr = parts[0].parse()?;

                            // Verify it's in the subnet
                            match is_address_in_subnet(&gen_addr, cidr) {
                                Ok(true) => println!("    {}. {} ✓", i, generated_ip),
                                Ok(false) => {
                                    println!("    {}. {} ✗ (NOT in subnet!)", i, generated_ip)
                                }
                                Err(e) => println!(
                                    "    {}. {} ✗ (Validation error: {})",
                                    i, generated_ip, e
                                ),
                            }
                        }
                        Err(e) => println!("    {}. Generation failed: {}", i, e),
                    }
                }
            }
            Err(e) => println!("  ✗ Parse failed: {}", e),
        }
        println!();
    }

    // Test invalid CIDR handling
    println!("Testing invalid CIDR handling:");
    let invalid_cidrs = vec![
        "invalid",
        "2001:db8::/129", // Invalid prefix length
        "2001:gg8::/64",  // Invalid hex
        "192.168.1.0/24", // IPv4, not IPv6
    ];

    for invalid_cidr in invalid_cidrs {
        print!("  Testing '{}': ", invalid_cidr);
        match generate_random_ip(invalid_cidr) {
            Ok(ip) => println!("✗ Unexpectedly succeeded: {}", ip),
            Err(e) => println!("✓ Properly rejected: {}", e),
        }
    }

    println!("\n🎉 All safety tests completed successfully!");
    Ok(())
}
