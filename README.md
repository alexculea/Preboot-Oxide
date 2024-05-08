# Simple DHCP Server for Preboot Execution Environment (PXE) booting

<img src="assets/logo.webp" height="96" style="border-radius: 96px" />

PXE boot server for networks where a DHCP server already exists and should be kept separate.
This tool currently takes care of the DHCP part of the PXE network [booting process](https://en.wikipedia.org/wiki/Preboot_Execution_Environment]). It does not handle the TFTP service, a separate tool is required for TFTP.

## Use cases
  - Quickly boot OSes from the network without having to use USB drives
  - Automate OS installs for a large number of devices

Highlights include:
  - facilitates easy & simple setup to get network booting working
  - compatible with **an existing authoritative DHCP server** on the same network
  - excellent maintenability and memory safety due to being written in Rust

## Supported platforms
Should compile & work on all Unix derivatives - only tested on Debian Linux.

## Project Status
Early stage - not recommended for critical or large scale use. The tool was was tested to work with limited device/network setup permutations.

### Known issues or limitations
- No IPv6 support
- Needs to run at the same IP as the TFTP server.
- Requires elevated access to listen on privileged ports 67 & 68
- Requires a separate DHCP server as it is designed specifically for PXE boots and not for *usual* DHCP operation
- Lacks OS service configuration
- There's a DOS vulnerability where a flood of DHCP `OFFER` without follow up `ACK` will cause the process to fill the system memory.

Some of the issues have planned fixes, see Roadmap.

## Quick start

### Clone & build

- Install Rust compiler [from the install page](https://www.rust-lang.org/tools/install).

```BASH
cargo build --release
```

### Configure & start
Configuration supports either process environment variables or using a dotenv (.env) file in the same folder as the executable.

```BASH
# using variables
PO_SERVER_IPV4=<your-ip> PO_BOOT_FILE=/path/to/the/bootable/image ./target/release/Preboot-Oxide
```

Using .env file:
```BASH
# configure the .env file first
content="
  PO_SERVER_IPV4=<your-ip>
  PO_BOOT_FILE=/path/to/the/bootable/image
"
echo "$content" > ./target/release/.env

# and run
./target/release/Preboot-Oxide
```

### Errors

1. `Address already in use (os error 98)`: Preboot-Oxide requires both ports 67 and 68 to be free which are usually not. On most Linux systems `dhclient` will occupy port 68 and if a DHCP server is installed that will occupy 67.
    
    On Linux: 
      ```BASH
      sudo ss -lpn 'sport = :68' # finds which process listens on 68
      sudo ss -lpn 'sport = :67' # finds which process listens on 68
      
      sudo pkill -f dhclient && sudo pkill -f dhcpd # the usual suspects
      
      # temporary operation with these processes stopped should be fine for a while but will break the OS DHCP operation of configuring its network interfaces
      
      # to get around this, it is possible to setup an UDP duplicate proxy using iptables and ensure the DHCP packets get delivered to both dhclient/dhcpd and Preboot-Oxide by setting up a virtual network interface for each and routing the packets accordingly. support for this type of proxying is in on the roadmap

      # additionally, static network configuration can be used to not need the use of `dhclient`.
      ```

### Supported ENV variables

 - `PO_SERVER_IPV4`: IPv4 of the running & TFTP boot server.
 - `PO_BOOT_FILE`: Path to the boot image on the TFTP server. Relative to your TFTP server, often seen as `pxelinux.0` or `folder/path/to-it.efi`
 - `PO_LOG_LEVEL`
    
    Allows setting log level to Preboot Oxide and its dependencies. The supported levels are: error, warn, info, debug, trace. Example: `PO_LOG_LEVEL=Preboot_Oxide=trace`, for even more verbose output: `PO_LOG_LEVEL=trace`. Default: `error`.
 - `PO_IFACES`: Comma separated names of the network interfaces the program should listen on. Example: `PO_IFACES=enp0s3,enp0s8`. Unless specified, it will listen on all network interfaces.


### Development notes

### Debugging
VS Code is the recommende IDE.
Debugging is best done when the program runs as priviliged and this can be achieved using the `lldb-server` (which needs to be installed separately). Once present,
`./vscode/launch.json` has `Remote LLDB as root` to match the launch configuration.

```BASH
# run the server before launching the debugger from VS Code
sudo su
lldb-server platform --server --listen 127.0.0.1:12345
```


