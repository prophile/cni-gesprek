use serde_json::{json, Value};
use std::collections::HashMap;

/// Builder for creating CNI configuration for tests
#[derive(Debug, Clone)]
pub struct CniConfigBuilder {
    config: Value,
}

impl CniConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: json!({
                "cniVersion": "1.0.0",
                "name": "test-network",
                "type": "cni-gesprek"
            }),
        }
    }

    /// Create a builder from existing JSON string
    pub fn from_json_string(json_str: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let parsed: Value = serde_json::from_str(json_str)?;
        Ok(Self { config: parsed })
    }

    pub fn with_name(mut self, name: &str) -> Self {
        self.config["name"] = json!(name);
        self
    }

    pub fn with_cni_version(mut self, version: &str) -> Self {
        self.config["cniVersion"] = json!(version);
        self
    }

    pub fn with_master(mut self, master: &str) -> Self {
        self.config["master"] = json!(master);
        self
    }

    pub fn with_pod_cidr(mut self, cidr: &str) -> Self {
        self.config["podCIDR"] = json!(cidr);
        self
    }

    pub fn with_dns(
        mut self,
        nameservers: Vec<&str>,
        domain: Option<&str>,
        search: Option<Vec<&str>>,
    ) -> Self {
        let mut dns = json!({
            "nameservers": nameservers
        });

        if let Some(domain) = domain {
            dns["domain"] = json!(domain);
        }

        if let Some(search) = search {
            dns["search"] = json!(search);
        }

        self.config["dns"] = dns;
        self
    }

    pub fn build(self) -> Value {
        self.config
    }

    pub fn build_json_string(self) -> String {
        serde_json::to_string(&self.config).unwrap()
    }
}

impl Default for CniConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for CNI environment variables
#[derive(Debug, Clone)]
pub struct CniEnvBuilder {
    env_vars: HashMap<String, String>,
}

impl CniEnvBuilder {
    pub fn new() -> Self {
        Self {
            env_vars: HashMap::new(),
        }
    }

    pub fn with_command(mut self, command: &str) -> Self {
        self.env_vars
            .insert("CNI_COMMAND".to_string(), command.to_string());
        self
    }

    pub fn with_container_id(mut self, id: &str) -> Self {
        self.env_vars
            .insert("CNI_CONTAINERID".to_string(), id.to_string());
        self
    }

    pub fn with_netns(mut self, netns: &str) -> Self {
        self.env_vars
            .insert("CNI_NETNS".to_string(), netns.to_string());
        self
    }

    pub fn with_ifname(mut self, ifname: &str) -> Self {
        self.env_vars
            .insert("CNI_IFNAME".to_string(), ifname.to_string());
        self
    }

    pub fn with_args(mut self, args: &str) -> Self {
        self.env_vars
            .insert("CNI_ARGS".to_string(), args.to_string());
        self
    }

    pub fn with_path(mut self, path: &str) -> Self {
        self.env_vars
            .insert("CNI_PATH".to_string(), path.to_string());
        self
    }

    pub fn with_custom(mut self, key: &str, value: &str) -> Self {
        self.env_vars.insert(key.to_string(), value.to_string());
        self
    }

    pub fn build(self) -> HashMap<String, String> {
        self.env_vars
    }

    /// Convert to a vector of tuples for use with process builders
    pub fn as_env_vec(&self) -> Vec<(String, String)> {
        self.env_vars
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

impl Default for CniEnvBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Standard test scenarios for CNI configuration
pub struct TestScenarios;

impl TestScenarios {
    /// Basic ADD command scenario
    pub fn basic_add() -> (CniConfigBuilder, CniEnvBuilder) {
        let config = CniConfigBuilder::new()
            .with_name("test-basic")
            .with_master("eth0")
            .with_pod_cidr("2001:db8::/64");

        let env = CniEnvBuilder::new()
            .with_command("ADD")
            .with_container_id("test-container-123")
            .with_netns("/proc/12345/ns/net")
            .with_ifname("eth0");

        (config, env)
    }

    /// ADD command with DNS configuration
    pub fn add_with_dns() -> (CniConfigBuilder, CniEnvBuilder) {
        let config = CniConfigBuilder::new()
            .with_name("test-dns")
            .with_master("eth1")
            .with_pod_cidr("2001:db8:1::/64")
            .with_dns(
                vec!["2001:db8::53", "2001:db8::54"],
                Some("example.com"),
                Some(vec!["example.com", "internal.example.com"]),
            );

        let env = CniEnvBuilder::new()
            .with_command("ADD")
            .with_container_id("test-container-dns")
            .with_netns("/proc/54321/ns/net")
            .with_ifname("net1");

        (config, env)
    }

    /// ADD command without explicit interface (auto-detection)
    pub fn add_auto_detect() -> (CniConfigBuilder, CniEnvBuilder) {
        let config = CniConfigBuilder::new()
            .with_name("test-auto")
            .with_pod_cidr("2001:db8:2::/64");

        let env = CniEnvBuilder::new()
            .with_command("ADD")
            .with_container_id("test-container-auto")
            .with_netns("/proc/99999/ns/net")
            .with_ifname("eth0");

        (config, env)
    }

    /// ADD command without explicit CIDR (subnet detection)
    pub fn add_detect_subnet() -> (CniConfigBuilder, CniEnvBuilder) {
        let config = CniConfigBuilder::new()
            .with_name("test-subnet-detect")
            .with_master("eth2");

        let env = CniEnvBuilder::new()
            .with_command("ADD")
            .with_container_id("test-container-subnet")
            .with_netns("/proc/88888/ns/net")
            .with_ifname("net0");

        (config, env)
    }

    /// DEL command scenario
    pub fn delete() -> (CniConfigBuilder, CniEnvBuilder) {
        let config = CniConfigBuilder::new().with_name("test-delete");

        let env = CniEnvBuilder::new()
            .with_command("DEL")
            .with_container_id("test-container-del")
            .with_netns("/proc/77777/ns/net")
            .with_ifname("eth0");

        (config, env)
    }

    /// CHECK command scenario
    pub fn check() -> (CniConfigBuilder, CniEnvBuilder) {
        let config = CniConfigBuilder::new().with_name("test-check");

        let env = CniEnvBuilder::new()
            .with_command("CHECK")
            .with_container_id("test-container-check")
            .with_netns("/proc/66666/ns/net")
            .with_ifname("eth0");

        (config, env)
    }

    /// VERSION command scenario
    pub fn version() -> (CniConfigBuilder, CniEnvBuilder) {
        let config = CniConfigBuilder::new();

        let env = CniEnvBuilder::new().with_command("VERSION");

        (config, env)
    }

    /// STATUS command scenario
    pub fn status() -> (CniConfigBuilder, CniEnvBuilder) {
        let config = CniConfigBuilder::new()
            .with_name("test-status")
            .with_master("eth0");

        let env = CniEnvBuilder::new()
            .with_command("STATUS")
            .with_container_id("test-container-status")
            .with_netns("/proc/55555/ns/net")
            .with_ifname("eth0");

        (config, env)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cni_config_builder() {
        let config = CniConfigBuilder::new()
            .with_name("test-net")
            .with_master("eth0")
            .with_pod_cidr("2001:db8::/64")
            .build();

        assert_eq!(config["name"], "test-net");
        assert_eq!(config["master"], "eth0");
        assert_eq!(config["podCIDR"], "2001:db8::/64");
    }

    #[test]
    fn test_cni_env_builder() {
        let env = CniEnvBuilder::new()
            .with_command("ADD")
            .with_container_id("test123")
            .with_netns("/proc/123/ns/net")
            .with_ifname("eth0")
            .build();

        assert_eq!(env.get("CNI_COMMAND"), Some(&"ADD".to_string()));
        assert_eq!(env.get("CNI_CONTAINERID"), Some(&"test123".to_string()));
        assert_eq!(env.get("CNI_NETNS"), Some(&"/proc/123/ns/net".to_string()));
        assert_eq!(env.get("CNI_IFNAME"), Some(&"eth0".to_string()));
    }

    #[test]
    fn test_basic_add_scenario() {
        let (config, env) = TestScenarios::basic_add();
        let config_json = config.build();
        let env_vars = env.build();

        assert_eq!(config_json["name"], "test-basic");
        assert_eq!(env_vars.get("CNI_COMMAND"), Some(&"ADD".to_string()));
    }
}
