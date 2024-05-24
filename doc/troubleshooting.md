
### Errors

1. `Address already in use (os error 98)`: Most likely there already is a socket at port 69 interfering with the TFTP service. Either disable TFTP (see [Configuring](./configuring.md)) and reuse the existing service or stop the process.
    
    On Linux: 
      ```BASH
      sudo ss -lpn 'sport = :69' # finds which process listens on 68
      sudo kill <pid>
      ```
2. `Io(Os { code: 2, kind: NotFound, message: "No such file or directory" })`

    The directory configured for the TFTP service doesn't exist.

### Logging

For a quick start on seeing what is wrong within the logs just re-rerun with the logging set to `info` as seen below.
```BASH
PO_LOG_LEVEL=info PO_TFTP_SERVER_DIR_PATH=/dir/hosting/the/boot/files PO_BOOT_FILE=/path/to/the/bootable/image ./target/release/preboot-oxide
# optionally, change 'info' to 'debug' or 'trace' for more verbose logging.
```
