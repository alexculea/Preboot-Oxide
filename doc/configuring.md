

<!-- TOC --><a name="configuration"></a>
# Configuration

The are 2 supported modes for configuration

  1. Process environment variables
  3. YAML configuration file

<!-- TOC start (generated with https://github.com/derlin/bitdowntoc) -->
#### Table of contents
- [Configuration](#configuration)
   * [1. Process environment variables](#1-process-environment-variables)
        - [Supported ENV variables](#supported-env-variables)
   * [3. YAML configuration file](#3-yaml-configuration-file)
      + [Simplest example](#simplest-example)
      + [Per-device config from its MAC address](#per-device-config-from-its-mac-address)
      + [Using an external TFTP server](#using-an-external-tftp-server)
      + [Only use certain networks](#only-use-certain-networks)
   * [Supported fields](#supported-fields)

<!-- TOC end -->


<!-- TOC --><a name="1-process-environment-variables"></a>
## 1. Process environment variables
<!-- TOC --><a name="supported-env-variables"></a>
#### Supported ENV variables

 - `PO_TFTP_SERVER_IPV4`: IPv4 of the running Trivial File Transfer Protocol (TFTP) boot server. Only necessary when using a separate TFTP server. If not specified, a TFTP service will be started, indicating its own IP to the booting clients to reach the service. If this has a value, the TFTP service is disabled.
 - `PO_BOOT_FILE`: Path to the boot image within the TFTP server - relative to the TFTP directory served, commonly configured as `pxelinux.0` or `folder/path/to-it.efi`. Parameter is required.
 - `PO_LOG_LEVEL`: Filter and verbosity level for output to `stdout`. 

    Syntax: `<component>`=`level`. 
    
    Example: `PO_LOG_LEVEL=preboot_-_oxide=info`
    
    Allows setting log level to Preboot Oxide and its dependencies. The supported levels are: error, warn, info, debug, trace. 
    
    Example: `PO_LOG_LEVEL=Preboot_Oxide=trace`, for even more verbose output: `PO_LOG_LEVEL=trace`. 
    
    Default: `error`.
 - `PO_IFACES`: Comma separated names of the network interfaces the program should listen on. Example: `PO_IFACES=enp0s3,enp0s8`. Optional, unless specified, it will listen on all network interfaces.
 - `PO_CONF_PATH`: Path for overriding the default YAML configuration file.
 - `PO_MAX_SESSIONS`: Optional number of maximum concurrent sessions to be allowed. Defaults to 500, used to protect against flood filling the system memory.

Specifying ENV variables can be achieved in a number of ways depending on the OS and how the executable is ran. Some examples:

👉 Simple CLI invocation:

```BASH
PO_TFTP_SERVER_DIR_PATH=/dir/hosting/the/boot/files PO_BOOT_FILE=/path/to/the/bootable/image preboot-oxide
```

👉 `.env` file in the same directory as the executable
```BASH
content="
  PO_TFTP_SERVER_DIR_PATH=/dir/hosting/the/boot/files
  PO_BOOT_FILE=/the/bootable/image
"
echo "$content" > ./target/release/.env

# and run
./target/release/preboot-oxide
```

👉 OS-level, useful when running as a service, it will depend on the OS used - for most Linux distros it is possible by adding the variables to the `/etc/environment` file

<!-- TOC --><a name="3-yaml-configuration-file"></a>
## 3. YAML configuration file

In short, [YAML](https://yaml.org/) is a static configuration format similar to JSON that uses tabs instead of braces (`{}`). Simple [tutorial](https://www.redhat.com/sysadmin/yaml-beginners).

The YAML content is loaded from `PO_CONF_PATH` env variable or from  `~/.config/preboot-oxide/preboot-oxide.yaml`. There is a sample file at the project root [here](../sample.preboot-oxide.yaml), reading on is recommended for a better understanding.

This will override process ENV variables (temporary, it is planned to reverse the priority with the variables taking precedence). When running as service with `systemd`, the location will correspond to the `root` user at `/root/.config/preboot-oxide/preboot-oxide.yaml`.

Conceptually, all PXE booting devices require only two parameters. The string path of the executable file to run at boot time and where to get that file from. The first is a Unix style path, the 2nd is an IPv4 address where the Trivial File Transfer Protocol (TFTP) service is available to serve the file.

The configuration is split between global vs client specific sections. The global section applies to the boot server generally, such as what network cards to use or where are the files for booting. The client sections define the boot file and the TFTP IP. 

<!-- TOC --><a name="simplest-example"></a>
### Simplest example

```YAML
tftp_server_dir: /where/the/boot/files/are
# if given, the TFTP service will be started and serve this path
# if not given, it is expected that a boot_server_ipv4 is given instead

# this section defines the boot file and server to be used by all clients
default:
  boot_file: /the/boot/file.efi
```

<!-- TOC --><a name="per-device-config-from-its-mac-address"></a>
### Per-device config from its MAC address

```YAML
tftp_server_dir: /where/the/boot/files/are
by_mac_address:
  00:01:02:03:04:05:
    boot_file: /path/to/bootfile.efi
```

Using a default is still possible:

```YAML
tftp_server_dir: /where/the/boot/files/are
default:
  boot_file: /path/for/all/clients.bin

by_mac_address:
  00:01:02:03:04:05:
    boot_file: /path/to/bootfile.efi # different file for this one
```

When overriding for a specific client, we can specify either `boot_file` or the `boot_server_ipv4` or both. If any is not given, the ones in `default` will be used.

<!-- TOC --><a name="using-an-external-tftp-server"></a>
### Using an external TFTP server

```YAML
# tftp_server_dir: /where/the/boot/files/are
# commenting this out will disable TFTP service

default:
  boot_file: /path/to/bootfile.efi
  boot_server_ipv4: 12.34.56.78 # for all clients

by_mac_address:
  00:01:02:03:04:05:
    boot_file: /path/to/bootfile.efi
    boot_server_ipv4: 123.45.67.89 # different TFTP for this client
```

or just override the boot file but use the same TFTP server

```YAML
default:
  boot_file: /path/to/x86-bios.bin
  boot_server_ipv4: 12.34.56.78

by_mac_address:
  00:01:02:03:04:05:
    boot_file: /path/to/x86-uefi.efi # this client boots UEFI
```

<!-- TOC --><a name="only-use-certain-networks"></a>
### Only use certain networks
```YAML
ifaces:
  - enp0s3
  - enp0s8

default:
  boot_file: /path/for/all/clients.bin
  boot_server_ipv4: 12.34.56.78
```


<!-- TOC --><a name="supported-fields"></a>
## Supported fields

- `ifaces`: List of network interfaces for listening. Ex:

  ```YAML
  ifaces:
    - enp0s3
    - enp0s8
  ```

- `tftp_server_dir`: Path to the local directory to be served by the TFTP service.

  ```YAML
  tftp_server_dir: /where/the/boot/files/are
  ```

  Optional, if not given, it is expected that `boot_server_ipv4` will be given instead to use an external TFTP server.

- `boot_file`: The UNIX path to the file to be executed at boot time from within the TFTP service.
- `boot_server_ipv4`: IPv4 address of TFTP service.
- `default`: Holds the `boot_file` and, optionally, `boot_server_ipv4` to provide to the booting client devices.
- `by_mac_addres`: Holds the list of mac addresses that override either `boot_file` or `boot_server_ipv4`.
- `max_sessions`: Optional, defaults to 500. Represents the maximum number of allowed sessions at the same time. A session starts when an OFFER message is seen from DHCP to the booting client and ends when either the client ACKed or refused the request. Sessions older than 3 minutes are automatically removed. This is used to prevent filling the system memory in case of a flood of DHCP messages on the network.
