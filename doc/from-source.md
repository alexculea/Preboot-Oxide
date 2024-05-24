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
PO_TFTP_SERVER_DIR_PATH=/dir/hosting/the/boot/files PO_BOOT_FILE=/path/to/the/bootable/image sudo ./target/release/preboot-oxide
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
sudo ./target/release/preboot-oxide
```

PXE network booting should now work. ❇️ 

(see [Configuring](./configuring.md) or [Troubleshooting](./troubleshooting.md) otherwise)

### Requirements
- The booting device should support the (usually very common) PXE boot procedure.
- Both the server and the booting device need to be on the same LAN, or within a network where UDP broadcast is supported (this generally doesn't work outside local networks).
