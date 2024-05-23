## Installing from source

### Depdencies

- Install Rust compiler [from the install page](https://www.rust-lang.org/tools/install).


### Clone & build

```BASH
git clone git@github.com:alexculea/Preboot-Oxide.git && cd ./Preboot-Oxide
cargo build --release
```

### Configure & start
Configuration supports either process environment variables or using a dotenv (.env) file in the same folder as the executable.

```BASH
# using variables
PO_TFTP_SERVER_DIR_PATH=/dir/hosting/the/boot/files PO_BOOT_FILE=/path/to/the/bootable/image ./target/release/preboot-oxide
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
./target/release/preboot-oxide
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
PO_LOG_LEVEL=info PO_TFTP_SERVER_DIR_PATH=/dir/hosting/the/boot/files PO_BOOT_FILE=/path/to/the/bootable/image ./target/release/preboot-oxide
# optionally, change 'info' to 'debug' or 'trace' for more verbose logging.
```

### Supported ENV variables

 - `PO_TFTP_SERVER_IPV4`: IPv4 of the running & TFTP boot server. Optional, if not specified, the server will start a TFTP service and will indicate its own IP to the booting clients to reach the service. If this has a value, the TFTP service is disabled.
 - `PO_BOOT_FILE`: Path to the boot image on the TFTP server. Relative to your TFTP server, often seen as `pxelinux.0` or `folder/path/to-it.efi`. Parameter is required.
 - `PO_LOG_LEVEL`
    
    Allows setting log level to Preboot Oxide and its dependencies. The supported levels are: error, warn, info, debug, trace. Example: `PO_LOG_LEVEL=Preboot_Oxide=trace`, for even more verbose output: `PO_LOG_LEVEL=trace`. Default: `error`.
 - `PO_IFACES`: Comma separated names of the network interfaces the program should listen on. Example: `PO_IFACES=enp0s3,enp0s8`. Optional, unless specified, it will listen on all network interfaces.

