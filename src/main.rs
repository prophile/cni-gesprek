mod driver;
mod driver_dryrun;
mod driver_script;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

use clap::Parser;
use driver::NetworkDriver;
use driver_dryrun::DryRunDriver;
use driver_script::ScriptDriver;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::env;
use std::error::Error;
use std::io::{self, Read};
use std::net::Ipv6Addr;
use std::path::Path;
use std::str::FromStr;

// --- Configuration Structures (CNI Spec) ---

#[derive(Serialize, Deserialize, Debug)]
struct CniConfig {
    cniVersion: String,
    name: String,
    #[serde(rename = "type")]
    plugin_type: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    master: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    podCIDR: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    dns: Option<CniDns>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct CniDns {
    #[serde(skip_serializing_if = "Option::is_none")]
    nameservers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    search: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug)]
struct CniResult {
    cniVersion: String,
    interfaces: Vec<CniInterface>,
    ips: Vec<CniIp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dns: Option<CniDns>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct CniInterface {
    name: String,
    mac: String,
    sandbox: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct CniIp {
    version: String,
    address: String,
    interface: usize,
}

// --- CLI Arguments ---

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    interface: Option<String>,

    #[arg(long)]
    pod_cidr: Option<String>,

    #[arg(long)]
    dry_run: bool,
}

// --- Helpers ---

fn generate_random_ip(cidr: &str) -> Result<String, Box<dyn Error>> {
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
        let shift = 128 - prefix_len;
        let set_bit = 1u128.checked_shl(shift as u32).unwrap_or(0);
        if shift == 128 {
            0
        } else {
            !(set_bit - 1)
        }
    };

    let mut rng = rand::thread_rng();
    let random_part: u128 = rng.gen();
    let final_u128 = (base_u128 & mask) | (random_part & !mask);

    let final_ip = Ipv6Addr::from(final_u128);
    Ok(format!("{}/{}", final_ip, prefix_len))
}

// --- Main Logic ---

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let driver: Box<dyn NetworkDriver> = if args.dry_run {
        Box::new(DryRunDriver)
    } else {
        Box::new(ScriptDriver)
    };

    let cni_command = env::var("CNI_COMMAND").unwrap_or_else(|_| "VERSION".to_string());

    let cni_config: Option<CniConfig> =
        if ["ADD", "DEL", "CHECK", "STATUS"].contains(&cni_command.as_str()) {
            let mut buffer = String::new();
            let _ = io::stdin().read_to_string(&mut buffer);
            if !buffer.is_empty() {
                serde_json::from_str(&buffer).ok()
            } else {
                None
            }
        } else {
            None
        };

    match cni_command.as_str() {
        "ADD" => cmd_add(&args, cni_config, &*driver),
        "DEL" => cmd_success(),
        "CHECK" => cmd_success(),
        "GC" => cmd_success(),
        "VERSION" => cmd_version(),
        "STATUS" => cmd_status(&args, cni_config, &*driver),
        _ => {
            eprintln!("Unknown CNI_COMMAND: {}", cni_command);
            std::process::exit(1);
        }
    }
}

fn cmd_add(
    args: &Args,
    config: Option<CniConfig>,
    driver: &dyn NetworkDriver,
) -> Result<(), Box<dyn Error>> {
    let config = config.ok_or("Missing CNI configuration on stdin")?;
    let netns_str = env::var("CNI_NETNS").map_err(|_| "CNI_NETNS not set")?;
    let netns_path = Path::new(&netns_str);
    let ifname = env::var("CNI_IFNAME").map_err(|_| "CNI_IFNAME not set")?;

    // 1. Determine Host Interface
    let master_interface = if let Some(cli_iface) = &args.interface {
        cli_iface.clone()
    } else if let Some(conf_master) = &config.master {
        conf_master.clone()
    } else {
        driver.detect_upstream_interface()?
    };

    driver.check_interface(&master_interface)?;

    // 2. Determine Subnet (CIDR)
    let cidr_string = if let Some(cli_cidr) = &args.pod_cidr {
        cli_cidr.clone()
    } else if let Some(conf_cidr) = &config.podCIDR {
        conf_cidr.clone()
    } else {
        let (ip, pfx) = driver.get_interface_subnet(&master_interface)?;
        format!("{}/{}", ip, pfx)
    };

    // 3. Detect Default Gateway
    let gateway = driver
        .get_interface_gateway(&master_interface)
        .unwrap_or(None);

    // 4. Generate Random IP
    let target_ip_cidr = generate_random_ip(&cidr_string)?;

    // 5. Create IPvlan (Host Side)
    let random_suffix: String = (0..8)
        .map(|_| {
            let idx = rand::random::<usize>() % 16;
            format!("{:x}", idx)
        })
        .collect();
    let temp_name = format!("ipvl{}", random_suffix);

    driver.create_ipvlan(&master_interface, "l2", &temp_name)?;

    // 6. Move to Netns
    if let Err(e) = driver.set_netns(&temp_name, netns_path) {
        let _ = driver.delete_interface(&temp_name);
        return Err(e);
    }

    // 7. Configure Inside Netns
    if let Err(e) = driver.configure_in_netns(
        netns_path,
        &temp_name,
        &ifname,
        &target_ip_cidr,
        gateway.as_deref(),
    ) {
        return Err(e);
    }

    // 8. Output Result
    let result = CniResult {
        cniVersion: config.cniVersion,
        interfaces: vec![CniInterface {
            name: ifname,
            mac: "".to_string(),
            sandbox: netns_str,
        }],
        ips: vec![CniIp {
            version: "6".to_string(),
            address: target_ip_cidr,
            interface: 0,
        }],
        // UPDATED: Pass through the DNS config provided in the JSON input
        dns: config.dns,
    };

    println!("{}", serde_json::to_string(&result)?);

    Ok(())
}

fn cmd_status(
    args: &Args,
    config: Option<CniConfig>,
    driver: &dyn NetworkDriver,
) -> Result<(), Box<dyn Error>> {
    let master_interface = if let Some(cli_iface) = &args.interface {
        cli_iface.clone()
    } else if let Some(c) = config.as_ref().and_then(|c| c.master.as_ref()) {
        c.clone()
    } else {
        match driver.detect_upstream_interface() {
            Ok(i) => i,
            Err(_) => std::process::exit(1),
        }
    };

    driver.check_interface(&master_interface)?;
    cmd_success()
}

fn cmd_version() -> Result<(), Box<dyn Error>> {
    println!(r#"{{"cniVersion": "1.0.0", "supportedVersions": ["1.0.0"]}}"#);
    Ok(())
}

fn cmd_success() -> Result<(), Box<dyn Error>> {
    println!(r#"{{"cniVersion": "1.0.0", "interfaces": [], "ips": [], "dns": {{}}}}"#);
    Ok(())
}
