# cni-gesprek

`cni-gesprek` is a lightweight, IPv6-only Container Network Interface (CNI) plugin written in Rust.

It provides container connectivity by creating an `ipvlan` interface (L2 mode, bridge). Instead of relying on DHCP or kernel SLAAC, it instantaneously assigns a **random, cryptographically generated IPv6 address** within the host's subnet. It also explicitly copies the default gateway from the host to the container, ensuring immediate connectivity without the latency of waiting for Router Advertisements.

## Features

* **IPv6 Native:** Built strictly for single-stack IPv6 environments.
* **Pseudo-Stateless:** mimics "stateless" auto-configuration by generating random stable addresses, but applied statically for speed.
* **Zero-Config:** Automatically detects the upstream interface, subnet, and gateway from the host's routing table.
* **Collision Safe:** Performs strict checks on Duplicate Address Detection (DAD) status, failing fast if a collision occurs (allowing the orchestrator to retry).
* **No Async Runtime:** Uses synchronous I/O for small binary size and simple execution flow.

## Installation

```bash
cargo build --release
sudo cp target/release/cni-gesprek /opt/cni/bin/
```

## Configuration Example

`/etc/cni/net.d/10-gesprek.conf`:

```json
{
  "cniVersion": "1.0.0",
  "name": "mynet",
  "type": "gesprek"
}
```

## Manual Override

```json
{
  "cniVersion": "1.0.0",
  "name": "mynet",
  "type": "gesprek",
  "master": "eth1",
  "podCIDR": "2001:db8:cafe::/64"
}
```

## License

This project is licensed under the *CC0 1.0 Universal (CC0-1.0) Public Domain Dedication*.
