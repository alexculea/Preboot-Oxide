### Logging

All logging is done via standard output. The output is directly visible when manually starting from the command line. When started as a service with systemd, the output is logged and can be viewed using `journalctl -uf preboot-oxide`. The detail logged is controlled by the logging level (verbosity). Available levels are:
    
    - error (default and always on)
    - warn
    - info (recommended for user troubleshooting)
    - debug
    - trace

The level can be configured with the `-v` command line argument or with the `PO_LOG_LEVEL` environment variable, see the [CLI section](./manual.md#command-line) or the [Process environment variables]((./manual.md#process-environment-variables)) section.

### Errors

1. `Address already in use (os error 98)`: Most likely there already is a socket at port 69 interfering with the TFTP service. Either disable TFTP (see [Manual](./manual.md)) and reuse the existing service or stop the process.
    
    On Linux: 
      ```BASH
      sudo ss -lpn 'sport = :69' # finds which process listens on 68
      sudo kill <pid>
      ```
2. `Io(Os { code: 2, kind: NotFound, message: "No such file or directory" })`

    The directory configured for the TFTP service doesn't exist.

