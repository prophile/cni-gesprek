use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::net::Ipv6Addr;
use std::str::FromStr;

// Since generate_random_ip is in main.rs, we'll recreate it for benchmarking
fn generate_random_ip_bench(cidr: &str) -> Result<String, Box<dyn std::error::Error>> {
    use rand::Rng;

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

fn bench_ip_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("ip_generation");

    let test_cidrs = vec![
        ("small_prefix", "2001:db8::/32"),
        ("standard_prefix", "2001:db8::/64"),
        ("large_prefix", "2001:db8::/96"),
        ("single_ip", "2001:db8::1/128"),
    ];

    for (name, cidr) in test_cidrs {
        group.bench_with_input(
            BenchmarkId::new("generate_random_ip", name),
            cidr,
            |b, cidr| b.iter(|| generate_random_ip_bench(black_box(cidr))),
        );
    }

    group.finish();
}

fn bench_ip_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("ip_parsing");

    let test_ips = vec![
        "2001:db8::1",
        "2001:db8:1:2:3:4:5:6",
        "::",
        "::1",
        "fe80::1",
    ];

    for ip_str in test_ips {
        group.bench_with_input(
            BenchmarkId::new("parse_ipv6", ip_str),
            ip_str,
            |b, ip_str| b.iter(|| Ipv6Addr::from_str(black_box(ip_str))),
        );
    }

    group.finish();
}

fn bench_json_operations(c: &mut Criterion) {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    struct CniConfig {
        #[serde(rename = "cniVersion")]
        cni_version: String,
        name: String,
        #[serde(rename = "type")]
        plugin_type: String,
        master: Option<String>,
        #[serde(rename = "podCIDR")]
        pod_cidr: Option<String>,
        dns: Option<serde_json::Value>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct CniResult {
        #[serde(rename = "cniVersion")]
        cni_version: String,
        interfaces: Vec<serde_json::Value>,
        ips: Vec<serde_json::Value>,
        dns: Option<serde_json::Value>,
    }

    let config_json = r#"{
        "cniVersion": "1.0.0",
        "name": "test-network",
        "type": "cni-gesprek",
        "master": "eth0",
        "podCIDR": "2001:db8::/64",
        "dns": {
            "nameservers": ["2001:db8::53"],
            "domain": "example.com"
        }
    }"#;

    c.bench_function("json_parse_config", |b| {
        b.iter(|| serde_json::from_str::<CniConfig>(black_box(config_json)))
    });

    let result = CniResult {
        cni_version: "1.0.0".to_string(),
        interfaces: vec![],
        ips: vec![],
        dns: None,
    };

    c.bench_function("json_serialize_result", |b| {
        b.iter(|| serde_json::to_string(black_box(&result)))
    });
}

fn bench_bit_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("bit_operations");

    let test_cases = vec![
        ("mask_32", 32u8),
        ("mask_64", 64u8),
        ("mask_96", 96u8),
        ("mask_128", 128u8),
    ];

    for (name, prefix_len) in test_cases {
        group.bench_with_input(
            BenchmarkId::new("create_mask", name),
            &prefix_len,
            |b, &prefix_len| {
                b.iter(|| {
                    let shift = 128 - prefix_len;
                    let set_bit = 1u128.checked_shl(shift as u32).unwrap_or(0);
                    if shift == 128 {
                        black_box(0)
                    } else {
                        black_box(!(set_bit - 1))
                    }
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_ip_generation,
    bench_ip_parsing,
    bench_json_operations,
    bench_bit_operations
);
criterion_main!(benches);
