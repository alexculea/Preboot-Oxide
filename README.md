# Simple DHCP Server for PXE booting

This tool takes care of the DHCP part of the PXE network booting process. It does not handle the TFPT service, a separate tool is required for TFTP.

## Use cases
  - Quickly boot OSes from the network without having to format USB drives
  - Automating OS installs for a large number of devices
  - Provision specific hardware by maping net MACs addresses to boot images

The tool exists specifically to:
  - facilitate easy/simple/quick setup to get running
  - to be fully compatible with **an existing authoritative DHCP server** on the same network
  - is designed specifically for PXE boots and not for *usual* DHCP operation

## Building from source

- Install Rust compiler [from the install page](https://www.rust-lang.org/tools/install).

```BASH
cargo build --release
```

- The configuration is done via process environment variables or via saving them to `.env` in the same folder as the executable (target/release folder within the project root after compilation).

### Supported ENV variables

 - `PXE_DHCP_BOOT_SERVER_IPV4`: IPv4 of the TFTP boot server.
 - `PXE_DHCP_BOOT_SERVER_IPV6`: IPv6 of the TFTP boot server.
 - `PXE_DHCP_BOOT_FILE`: Path to the boot image on the TFTP server
 - `PXE_DHCP_MAC_BOOT_FILES`: semicolon separated values of MAC address to boot image path. Ex: `AA:BB:CC:DD:FF;/path/for/thisdevice;11:22:33:44:55;/path/for/this/other/device`
 - `PXE_DHCP_LOG_LEVEL`: log level, with error, warn, info, debug, trace. Example config: `RUST_LOG=trace` will be the most verbose


### Debugging as root
To debug the process as root, remote LLDB debug server can be used with the existin VS Code launch configuration in `./vscode/launch.json` > `Remote LLDB as root`. 

```BASH
sudo su
lldb-server platform --server --listen 127.0.0.1:12345
```

### Errors

1. `Address already in use (os error 98)`: pxe-dhcp-rs requires both ports 67 and 68 to be free which are usually not. On most Linux systems dhclient will occupy port 68 and if a DHCP server is installed that will occupy 67.
    
    On Linux: 
      ```BASH
      sudo ss -lpn 'sport = :68' # finds which process listens on 68
      sudo ss -lpn 'sport = :67' # finds which process listens on 68
      
      sudo pkill -f dhclient && sudo pkill -f dhcpd # the usual suspects
      ```
