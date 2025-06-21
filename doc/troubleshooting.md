### Logging

Troubleshooting requires inspecting the logs - output is visible (`stdout`) when starting from the command line but when started as a service with systemd, the output can be viewed using `journalctl -uf preboot-oxide`. The detail logged is controlled by the logging level (verbosity). Available levels are:
    
    - error (default and always on)
    - warn
    - info (recommended for user troubleshooting)
    - debug
    - trace

The level can be configured with the `-v` command line argument or with the `PO_LOG_LEVEL` environment variable, depending on how you run the service, see the [Command Line section](./manual.md#command-line) for when you run it from the command line or the [Process environment variables](./manual.md#process-environment-variables) section for when running as a `systemd` service.

### Errors and logs explained

1. `Address already in use (os error 98)`: Most likely there already is a socket at port 69 interfering with the TFTP service. Either disable TFTP (see [Manual](./manual.md)) and reuse the existing service or stop the process.
    
    On Linux: 
      ```BASH
      sudo ss -lpn 'sport = :69' # finds which process listens on 68
      sudo kill <pid>
      ```
2. `Io(Os { code: 2, kind: NotFound, message: "No such file or directory" })`

    The directory configured for the TFTP service doesn't exist.

3. `No TFTP server path or external TFTP server configured.`

    The configuration does not specify a TFTP server directory (`tftp_server_dir`) or an external TFTP server IP (`boot_server_ipv4`). At least one must be set for PXE booting to work.

4. `No boot filename configured.`

    The configuration does not specify a boot file name (`boot_file`), either in the root section of the YAML or in any of the `match` sections. If a YAML file was used, the config ENV variable value is ignored, leading to this error. PXE clients require a boot file executable to be specified.

5. `No server config provided.`

    The DHCP server was started without a configuration. Ensure configuration is loaded from YAML or environment variables. See [Manual](./manual.md)

6. `Cannot determine boot file path for client having MAC address: {client}.`

    The server could not determine the boot file for the client. Check that the configuration provides a `boot_file` for the client or that a default one was defined.

7. `Cannot determine TFTP server IPv4 address for client having MAC address: {client}`

    Check that the configuration provides a `boot_server_ipv4` for the client or in the root section of the config. Usually this message should not come up as the server uses the IP of the interface. Should be an edge case in OS networking try rebooting or restarting the network stack and the service.

8. `No configuration found for client {client_mac_address_str}. Skipping`

    The server could not match the client to any configuration entry. Check your configuration's `match` and root sections.

9. `Initial discovery message for XID {client_xid} not found due to either a bug or incorrect DHCP server behavior. Skipping.`

    The server could not find the initial DHCPDISCOVER message for the transaction. This may indicate a bug or out-of-order DHCP traffic.

10. `No message type found in DHCP message with XID: {client_xid}. Skipping.`

    The received DHCP message did not include a message type. This may indicate malformed DHCP traffic.

11. `Cannot determine client MAC address.`

    The server could not extract the client MAC address from the DHCP message. This may indicate a malformed or unsupported DHCP message.

12. `Client {client_mac_address_str} declined REQUEST.`

    The PXE client sent a DHCPDECLINE, indicating it rejected the offered configuration (possibly due to IP conflict or other issues).

13. `Session for client XID: {client_xid} timed out after {age} seconds. Was expecting IP information from an authoritative DHCP server. Is there a DHCP service running on the network?`

    The server did not receive an Offer from an authoritative DHCP server in time. Ensure a DHCP server is available and reachable.

14. `Session for client XID: {client_xid} timed out after {age} seconds. Was expecting client to follow up with a Request on the offer made.`

    The PXE client did not send a DHCPREQUEST after receiving an Offer. This may indicate network issues, client misconfiguration, or even a bug in the implementatin of the client. Try restarting the client and/or the server. More special clients might require more information in the offer or could choose the select the 1st DHCP offer which came from the DHCP server and might not contain nay boot information. Please open an issue with all the output of the interaction, with a `trace` log level which captures the exact messages.

15. `Not loading YAML configuration: {error}\nFalling back to environment variables.`

    The server failed to load the YAML configuration file and will use environment variables instead. Some common causes can include YAML syntax error, or file permissions. Check the error for details.

16. `TFTP path does not exist or is not directory: {dir}`

    The configured TFTP directory does not exist or is not a directory. Check your configuration and filesystem.

17. `TFTP server not started, no path configured.`

    No TFTP directory was configured, so the TFTP server was not started. This is OK if an external TFTP server IP was configured and the clients will use the external service instead of the one included.

18. `Another instance is already running`

    Only one instance of the server can run at a time. Stop the other instance before starting a new one.

19. `Binding socket to network device: {iface.name}` or `Binding socket to {ip} on network device: {iface.name}`

    Errors here indicate problems listening to the specified network interface or address. Check permissions and interface names.
