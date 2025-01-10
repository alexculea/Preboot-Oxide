# Preboot-Oxide: simple network boot server (PXE)

<img src="assets/logo.webp" height="96" style="border-radius: 96px" />

PXE boot server for networks with an existing DHCP server, targets _very fast and easy_ config & setup and takes care of both the DHCP and TFTP of the PXE network [booting process](https://en.wikipedia.org/wiki/Preboot_Execution_Environment]). See [Quick Start](./doc/quick-start.md) if in a hurry.

## Use cases
  - Quickly boot OSes from the network without having to use USB drives
  - Automate OS installs for a large number of devices
  - Integrate OS installs into Infrastructure as Code paradigms*

Highlights include:
  - Facilitates easy & simple setup to get network booting working
  - Compatible with **an existing authoritative DHCP server** on the same network
  - Excellent maintainability and memory safety due to being written in Rust
    - No Rust `unsafe` code in any of the network input processing logic.

## Supported platforms
Should compile & work on all Unix derivatives - only tested on Debian Linux.

## Project Status
Early stage - not recommended for critical or large scale use. Was was tested to work with limited device/network setup permutations.

For future plans, see [Roadmap](./ROADMAP.md).

### Known issues or limitations
- No IPv6 support
- Requires elevated access to listen on privileged ports 67, 68 and 69
- Requires a separate DHCP server as it is designed specifically for PXE boots and not for *usual* DHCP operation

## Installation

- [From source](./doc/from-source.md)
- [From .deb package (for Debian & Ubuntu)](./doc/from-deb.md)

## Configuration
- [User Manual](./doc/manual.md)

### Development notes

### Debugging
VS Code is the recommended IDE.
Debugging is best done when the program runs as privileged and this can be achieved using the `lldb-server` (which needs to be installed separately). Once present,
`./vscode/launch.json` has `Remote LLDB as root` to match the launch configuration.

```BASH
# run the server before launching the debugger from VS Code
sudo su
lldb-server platform --server --listen 127.0.0.1:12345
```

## Footnotes
*: Using unattended install setups it is possible to customize any aspect of the OS install such that all installs are reproducible.
