## Installing from source

### Depdencies

- Install Rust compiler [from the install page](https://www.rust-lang.org/tools/install).


### Clone & build

```BASH
git clone git@github.com:alexculea/Preboot-Oxide.git && cd ./Preboot-Oxide
cargo build --release
```

### Build packages
For .deb
```BASH
sudo ./scripts/package-as-deb.sh
```
or for CentOS/Fedora
```BASH
./scripts/package-as-rpm.sh
```

### Install the packages
.deb
```BASH
sudo dpkg -i ./preboot-oxide-amd64.deb
```
.rpm (on Fedora)
```BASH
sudo dnf install ./preboot-oxide.x86_64.rpm
```

Check installation
```
preboot-oxide -V
```

### Configure & start
Configuration supports either process environment variables or using a YAML file. See the [Manual](./manual.md) for how to configure.

```BASH
# via the CLI with ENV variables
PO_TFTP_SERVER_DIR_PATH=/dir/hosting/the/boot/files PO_BOOT_FILE=/path/to/the/bootable/image sudo preboot-oxide
```


```BASH
# Using YAML:
sudo mkdir -R /root/.config/preboot-oxide
sudo cp ./sample.preboot-oxide.yaml /root/.config/preboot-oxide/preboot-oxide.yaml
```

Start & check status
```BASH
sudo systemctl enable preboot-oxide
sudo systemctl status preboot-oxide
```

(see [Manual](./manual.md) or [Troubleshooting](./troubleshooting.md) otherwise)

### Requirements
- The booting device should support the (usually very common) PXE boot procedure.
- Both the server and the booting device need to be on the same LAN, or within a network where UDP broadcast is supported (this generally doesn't work outside local networks).
