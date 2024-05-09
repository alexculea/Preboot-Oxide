# Preboot-Oxide: simple network boot server (PXE)

<img src="assets/logo.webp" height="96" style="border-radius: 96px" />

PXE boot server for networks where a DHCP server already exists and should be kept separate.
This tool takes care of both the DHCP and TFTP of the PXE network [booting process](https://en.wikipedia.org/wiki/Preboot_Execution_Environment]).

## Use cases
  - Quickly boot OSes from the network without having to use USB drives
  - Automate OS installs for a large number of devices
  - Integrate OS installs into Infrastructure as Code paradigms*

Highlights include:
  - Facilitates easy & simple setup to get network booting working
  - Compatible with **an existing authoritative DHCP server** on the same network
  - Excellent maintenability and memory safety due to being written in Rust

## Supported platforms
Should compile & work on all Unix derivatives - only tested on Debian Linux.

## Project Status
Early stage - not recommended for critical or large scale use. Was was tested to work with limited device/network setup permutations.

### Known issues or limitations
- No IPv6 support
- Requires elevated access to listen on privileged ports 67, 68 and 69
- Requires a separate DHCP server as it is designed specifically for PXE boots and not for *usual* DHCP operation
- It will conflict with DHCP servers and clients on the same machine**
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
PO_TFTP_SERVER_DIR_PATH=/dir/hosting/the/boot/files PO_BOOT_FILE=/path/to/the/bootable/image ./target/release/Preboot-Oxide
```

Using .env file:
```BASH
# configure the .env file first
content="
  PO_TFTP_SERVER_DIR_PATH=/dir/hosting/the/boot/files
  PO_BOOT_FILE=/the/bootable/image
"
echo "$content" > ./target/release/.env

# and run
./target/release/Preboot-Oxide
```

PXE network booting should now work. ❇️ 

(see Requirements & Errors otherwise)

### Requirements
- The booting device should support the (usually very common) PXE boot procedure.
- Both the server and the booting device need to be on the same LAN, or within a network where UDP broadcast is supported (this generally doesn't work outside local networks).

### Errors

1. `Address already in use (os error 98)`: Preboot-Oxide requires both ports 67 and 68 to be free which are usually not. On most Linux systems `dhclient` will occupy port 68 and if a DHCP server is installed that will occupy 67.
    
    On Linux: 
      ```BASH
      sudo ss -lpn 'sport = :68' # finds which process listens on 68
      sudo ss -lpn 'sport = :67' # finds which process listens on 68
      
      sudo pkill -f dhclient && sudo pkill -f dhcpd # the usual suspects
      
      # temporary operation with these processes stopped should be fine for a while but will break the OS DHCP operation of configuring its network interfaces
      
      # to get around this permanently, static network configuration can be used to not need the use of `dhclient`.
      ```

### Logging

For a quick start on seeing what is wrong within the logs just re-rerun with the logging set to `info` as seen below.
```BASH
PO_LOG_LEVEL=info PO_TFTP_SERVER_DIR_PATH=/dir/hosting/the/boot/files PO_BOOT_FILE=/path/to/the/bootable/image ./target/release/Preboot-Oxide
# optionally, change 'info' to 'debug' or 'trace' for more verbose logging.
```

### Supported ENV variables

 - `PO_TFTP_SERVER_IPV4`: IPv4 of the running & TFTP boot server. Optional, if not specified, the server will start a TFTP service and will indicate its own IP to the booting clients to reach the service. If this has a value, the TFTP service is disabled.
 - `PO_BOOT_FILE`: Path to the boot image on the TFTP server. Relative to your TFTP server, often seen as `pxelinux.0` or `folder/path/to-it.efi`. Parameter is required.
 - `PO_LOG_LEVEL`
    
    Allows setting log level to Preboot Oxide and its dependencies. The supported levels are: error, warn, info, debug, trace. Example: `PO_LOG_LEVEL=Preboot_Oxide=trace`, for even more verbose output: `PO_LOG_LEVEL=trace`. Default: `error`.
 - `PO_IFACES`: Comma separated names of the network interfaces the program should listen on. Example: `PO_IFACES=enp0s3,enp0s8`. Optional, unless specified, it will listen on all network interfaces.


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

## Footnotes
*: Using unattended install setups it is possible to customize any aspect of the OS install such that all installs are reproducible.

**: As both the OS DHCP client and Preboot-Oxide need port 68, the client operation has to be interrupted to start the boot server. This should be fine in most scenarios if the network was already configured and the DHCP client can be stopped safely - however long term effects could include network disconnection if the network configuration is changed while the DHCP client is off or if the boot server was configured as a service, it could interfere in raising up the networks due to an erroring client. One possible workaround is to ensure that the network is up first, then stop the client and then start the server. This should allow both functions normally for the duration of the IP lease. A fully compatible solution is still being researched for Debian, to submit a proposal please don't hesitate to open an issue.