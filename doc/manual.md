

<!-- TOC --><a name="configuration"></a>
# Manual


<!-- TOC start (generated with https://github.com/derlin/bitdowntoc) -->

- [Command Line](#command-line)
- [Process environment variables](#process-environment-variables)
   * [Supported ENV variables](#supported-env-variables)
- [YAML configuration file](#yaml-configuration-file)
   * [Simple, same boot for all clients on the network](#simple-same-boot-for-all-clients-on-the-network)
   * [Per-device config from its MAC address](#per-device-config-from-its-mac-address)
   * [A special client or the default for the rest](#a-special-client-or-the-default-for-the-rest)
   * [By CPU architecture](#by-cpu-architecture)
   * [Using an external TFTP server](#using-an-external-tftp-server)
   * [Only use certain networks](#only-use-certain-networks)
- [Reference](#reference)
- [Troubleshooting config issues](#troubleshooting-config-issues)
   * [When running as a service with systemd](#when-running-as-a-service-with-systemd)
   * [When running without systemd](#when-running-without-systemd)
   * [Example trace logs](#example-trace-logs)

<!-- TOC end -->

<!-- TOC --><a name="command-line"></a>
## Command Line

- `-v...`: Helps troubleshoot issues controlling output verbosity. Available levels: warn, info, debug, trace. User troubleshooting level recommended is `info`. Examples:
  - info: `preboot-oxide -vv`
  - debug: `preboot-oxide -vvv`
- `-h`, `--help`: Prints CLI help
- `-V`, `--version`: Prints version

Command line arguments take precedence over environment variables or file configuration.

<!-- TOC --><a name="process-environment-variables"></a>
## Process environment variables
Configuration is supported by either process ENV variables or with YAML config file. For very quick & simple setups ENV vars might be sufficient, YAML otherwise. If a YAML file is found, all process env variables are ignored.

<!-- TOC --><a name="supported-env-variables"></a>
### Supported ENV variables

 
 - `PO_TFTP_SERVER_DIR_PATH`: Path to the local directory to be served by the TFTP service. Optional and if not supplied, it is expected that `PO_TFTP_SERVER_IPV4` will be given instead to use an external TFTP server.

      When this option is configured, it activates the TFTP service at port 69 on the configured `PO_IFACES` and the specified directory in read only mode. No remote changes are allowed but ⚠️ _the directory becomes accessible to any client connecting_ ⚠️. TFTP doesn't support authentication.

 - `PO_TFTP_SERVER_IPV4`: IPv4 of the running Trivial File Transfer Protocol (TFTP) boot server. Only necessary when using a separate TFTP server. If not specified, a TFTP service will be started, indicating its own IP to the booting clients to reach the service. If this has a value, the TFTP service is disabled.
 - `PO_BOOT_FILE`: The UNIX path to the file to be executed at boot time from within the TFTP service. The boot file path is relative to the directory of the TFTP service. If the file is at `/tmp/boot/file.bin` on the local disk and the TFTP service is configured to serve from `/tmp/boot` then the `boot_file` specified should be just `file.bin`. Parameter is required.
 - `PO_LOG_LEVEL`: Filter and verbosity level for output to `stdout`. 

    Syntax: `<component>`=`level`. 
    
    Example: `PO_LOG_LEVEL=preboot_oxide=info`
    
    Allows setting log level to Preboot Oxide and its dependencies. The supported levels are: error, warn, info, debug, trace. 
    
    Example: `PO_LOG_LEVEL=preboot_oxide=info`, for even more verbose output: `PO_LOG_LEVEL=trace`. 
    
    Default: `error`.
 - `PO_IFACES`: Comma separated names of the network interfaces the program should listen on. Example: `PO_IFACES=enp0s3,enp0s8`. Optional, unless specified, it will listen on all network interfaces.
 - `PO_CONF_PATH`: Path for overriding the default YAML configuration file.
 - `PO_MAX_SESSIONS`: Optional number of maximum concurrent sessions to be allowed. Defaults to 500, used to protect against flood filling the system memory.

Specifying ENV variables can be achieved in a number of ways depending on the OS and how the executable is ran. Some examples:

👉 Simple CLI invocation with both arguments and environment variables:

```BASH
PO_TFTP_SERVER_DIR_PATH=/dir/hosting/the/boot/files PO_BOOT_FILE=/path/to/the/bootable/image preboot-oxide -vv
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

👉 OS-level, useful when running as a service, it will depend on the OS used - for some Linux distros it is possible to add the variables to the `/etc/environment` file or editing the systemd service.

<!-- TOC --><a name="yaml-configuration-file"></a>
## YAML configuration file

In short, [YAML](https://yaml.org/) is a static configuration format similar to JSON that uses tabs instead of braces (`{}`). Simple [tutorial](https://www.redhat.com/sysadmin/yaml-beginners).

The YAML content is loaded from `PO_CONF_PATH` env variable or from  `~/.config/preboot-oxide/preboot-oxide.yaml`. There is a sample file at the project root [here](../sample.preboot-oxide.yaml), reading on is recommended for a better understanding.

This .yaml file config will override process ENV variables. When running as service with `systemd`, the location will correspond to the `root` user at `/root/.config/preboot-oxide/preboot-oxide.yaml`. It is possible to override the path using the `PO_CONF_PATH` env variable. [This SO answer](https://serverfault.com/a/413408) describes how to set env variables for systemd services.

Conceptually, all PXE booting devices require only two parameters. The path of the executable file to run at boot time and where to get that file from. The first is a Unix style path, the 2nd is an IPv4 address where the Trivial File Transfer Protocol (TFTP) service is available to serve the file.

The configuration is split between global vs client specific sections. The global section applies to the boot server generally, such as what network cards to use or where are the files for booting. The client sections define the boot file and the TFTP IP. Here are a few examples:

<!-- TOC --><a name="simple-same-boot-for-all-clients-on-the-network"></a>
### Simple, same boot for all clients on the network

```YAML
tftp_server_dir: /where/the/boot/files/are
# if given, the TFTP service will be started and serve this path
# if not given, it is expected that a boot_server_ipv4 is given instead

# this section defines the boot file and server to be used by all clients
default:
  boot_file: /the/boot/file.efi

# that's it, place this in ~/.config/preboot-oxide/preboot-oxide.yaml
```

<!-- TOC --><a name="per-device-config-from-its-mac-address"></a>
### Per-device config from its MAC address

```YAML
tftp_server_dir: /where/the/boot/files/are
match:
  # device #1
  - select:
      ClientMacAddress: 08:00:27:E7:DE:FE # this can also be lowercase
    conf:
      boot_file: /path/to/bootfile.efi

  # device #2
  - select:
      ClientMacAddress: 01:02:03:04:05:06
    conf:
      boot_file: /other/path/to/bootfile.efi

# any other client will be ignored
```

<!-- TOC --><a name="a-special-client-or-the-default-for-the-rest"></a>
### A special client or the default for the rest

```YAML
tftp_server_dir: /where/the/boot/files/are
default:
  # by default we will use this
  boot_file: /path/for/all/clients.bin 
  
match:
  # however for this client only
  - select:
      ClientMacAddress: 08:00:27:E7:DE:FE 

    # is given this
    conf:
      boot_file: /path/to/bootfile.efi 
      boot_server_ipv4: 1.2.3.4
```

When overriding for a specific client, we can specify either `boot_file` or the `boot_server_ipv4` or both. If any is not given, the ones in `default` will be used. Thus in the example above, client `08:00:27:E7:DE:FE` will be told to contact server `1.2.3.4` via TFTP and use `/path/to/bootfile.efi` instead.

<!-- TOC --><a name="by-cpu-architecture"></a>
### By CPU architecture
```YAML
tftp_server_dir: /where/the/boot/files/are

match:
  # UEFI aarch64 (ARM)
  - select:
      ClassIdentifier: Arch:00011
    regex: true # the value above is treated as a regular expression
    conf:
      boot_file: /path/to/arm64/uefi.efi
  # UEFI amd64 or x86 
  - select:
      ClassIdentifier: Arch:0000[6-7]
    regex: true 
    conf:
      boot_file: /path/to/uefi.efi
```

The list of standardized archtectures is [here](https://www.iana.org/assignments/dhcpv6-parameters/dhcpv6-parameters.xhtml#processor-architecture).

<!-- TOC --><a name="using-an-external-tftp-server"></a>
### Using an external TFTP server

```YAML
# tftp_server_dir: /where/the/boot/files/are
# commenting this out will disable TFTP service

default:
  boot_file: /path/to/bootfile.efi
  boot_server_ipv4: 12.34.56.78 # for all clients

match:
  - select:
      ClientMacAddress: 08:00:27:E7:DE:FE 
    conf:
      boot_file: /path/to/bootfile.efi 
      boot_server_ipv4: 123.45.67.89 # different TFTP for this client
```

or just override the boot file but use the same TFTP server

```YAML
default:
  boot_file: /path/to/x86-bios.bin
  boot_server_ipv4: 12.34.56.78

match:
  - select:
      ClientMacAddress: 00:01:02:03:04:05
    conf:
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


<!-- TOC --><a name="reference"></a>
## Reference

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

  When this option is configured, it activates the TFTP service at port 69 on the configured `ifaces` and the specified directory in read only mode. No remote changes are allowed but ⚠️ _the directory becomes accessible to any client connecting_ ⚠️. TFTP doesn't support authentication.

- `boot_file`: The UNIX path to the file to be executed at boot time from within the TFTP service. The boot file path is relative to the directory of the TFTP service. If the file is at `/tmp/boot/file.bin` on the local disk and the TFTP service is configured to serve from `/tmp/boot` then the `boot_file` specified should be just `file.bin`.
- `boot_server_ipv4`: IPv4 address of TFTP service, for when it is desirable to use an external TFTP service. If not specified, a TFTP service will be started, serving files from the specified `tftp_server_dir`.
- `default`: Holds the `boot_file` and, optionally, `boot_server_ipv4` to provide to the booting client devices.
`boot_server_ipv4`.
- `max_sessions`: Optional, defaults to 500. Represents the maximum number of allowed sessions at the same time. A session starts when an OFFER message is seen from DHCP to the booting client and ends when either the client ACKed or refused the request. Sessions older than 3 minutes are automatically removed. This is used to prevent filling the system memory in case of a flood of DHCP messages on the network.
- `match`: List of entries to match, optional. Subfields:

    - `select`: List of fields and values to match. Unless `regex` is `true`, the matching is done by value, case insensitive.
        - Supported fields:

              ClientMacAddress
              ClassIdentifier
              HardwareType
              ClientSystemArchitecture
              RequestedIpAddress
              ServerIdentifier

        - Example:

            ```YAML
            match:
            - select:
                ClassIdentifier: PXEClient:Arch:00007:UNDI:003000
                ClientMacAddress: 08:00:27:be:d8:91
                HardwareType: eth
                ...etc
              conf:
                boot_file: debian-installer/amd64/bootnetx64.efi
            ```

    - `regex`: `true` or `false`. When `true`, the value of the `select` field will be interpreted as a regular expression. The engine used can be tested with https://regex101.com/ (select Rust from the Flavor on the left).
    - `conf`: The resulting config when the client matched the `select`. Subfields:

      - `boot_file`: Same as above. If not specified, the `boot_file` in the `default` section will be used
      - `boot_server_ipv4`: Same as above. If not specified the `boot_server_ipv4` will be used. If `default` doesn't specify a `boot_server_ipv4` either, it is expected to set a path in `tftp_server_dir` and clients will be instructed to use the included TFTP service.

  - `match_type`: `all` or `any`. For `any`, if any of the `select` field-values match, the entry is considered a match. For `all`, all field-values in `select` have to match. In both cases, the first matching entry in the order of definition is used, thus it is best to declare the more specific matches first.

<!-- TOC --><a name="troubleshooting-config-issues"></a>
## Troubleshooting config issues

The best way to troubleshoot configuration issues is to inspect the output of `preboot-oxide`. By default only hard errors are printed however we can activate the `trace` log level to get a view on the internal logic of the booting process. This is possible by starting `preboot-oxide` with the `PO_LOG_LEVEL` environment variable. Depending on how it was installed, here are the two options:

<!-- TOC --><a name="when-running-as-a-service-with-systemd"></a>
### When running as a service with systemd
We can add ENV variables to systemd processes by:
```BASH
sudo systemctl edit preboot-oxide
```
The above should open an editor to the `override.conf` for the service. In the file we add the environment definition:
```
[Service]
Environment="PO_LOG_LEVEL=preboot_oxide::conf=trace"
```


We should end up with something like this:
```
### Editing /etc/systemd/system/preboot-oxide.service.d/override.conf
### Anything between here and the comment below will become the new contents of the file

[Service]
Environment="PO_LOG_LEVEL=preboot_oxide::conf=trace"

### Lines below this comment will be discarded

### /lib/systemd/system/preboot-oxide.service
# [Unit]
# Description=PXE Boot server
# After=network.target
# Wants=
# 
# [Service]
# Restart=always
# Type=simple
# ExecStart=/bin/preboot-oxide
# Environment=
# 
# [Install]
# WantedBy=multi-user.target
```

Save the file, restart the service and open the logs in follow mode:
```BASH
sudo systemctl restart preboot-oxide
sudo journalctl -f -u preboot-oxide # starts the logging in -f follow mode
```

Now that output logs are in `trace` mode and they are followed, the network boot from the client device can be started which should result like those in _Example trace logs_.


It is possible to see what the clients are reporting this way and configure accordingly.

<!-- TOC --><a name="when-running-without-systemd"></a>
### When running without systemd

The preboot-oxide process should be started with `PO_LOG_LEVEL` environment variable having the value ` preboot_oxide::conf=trace`. Check the OS/shell manual on how to specify environment variables to processes.

Once the started, the process will output logs as seen in _Example trace logs_.

<!-- TOC --><a name="example-trace-logs"></a>
### Example trace logs

When a match was found:
```
 2024-06-06T14:56:54.454Z TRACE preboot_oxide::conf > Matching regex field ClassIdentifier="PXEClient:Arch:00007:UNDI:003000" to "Arch:00007", matching = true
 2024-06-06T14:56:54.454Z TRACE preboot_oxide::conf > Found matching entry from 'match' rule.
ConfEntry {
    boot_file: Some(
        "/this/specific/client",
    ),
    boot_server_ipv4: None,
}
 2024-06-06T14:56:54.454Z TRACE preboot_oxide::conf > Final result combined with default:
ConfEntry {
    boot_file: Some(
        "/this/specific/client",
    ),
    boot_server_ipv4: None,
}
 2024-06-06T14:56:58.071Z TRACE preboot_oxide::conf > Matching regex field ClassIdentifier="PXEClient:Arch:00007:UNDI:003000" to "Arch:00007", matching = true
 2024-06-06T14:56:58.071Z TRACE preboot_oxide::conf > Found matching entry from 'match' rule.
 ```

When a match was not found

```
 2024-06-06T15:36:21.363Z TRACE preboot_oxide::conf > Matching regex field ClassIdentifier="PXEClient:Arch:00007:UNDI:003000" to "Arch:00009", matching = false
 2024-06-06T15:36:21.363Z TRACE preboot_oxide::conf > No matching entry found from 'match' rule.
```