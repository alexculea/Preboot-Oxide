## Installing from the .deb package

### Download & install
```BASH
    # donload the latest .deb package
    wget https://github.com/alexculea/preboot-oxide/releases/latest/download/preboot-oxide-amd64.deb

    # install
    sudo dkpg -i preboot-oxide-amd64.deb
    rm preboot-oxide-amd64.deb # clean up temp file
```

### Configure

Configuration is supported both with YAML and (more limited) with process ENV variables. The YAML is accessed from `$HOME/.config/preboot-oxide/preboot-oxide.yaml`. If running as a service with systemd, by default, the user is set to `root` thus the config path is `/root/.config/preboot-oxide/preboot-oxide.yaml`.

Example:
```BASH
content="
  tftp_server_dir: /where/the/boot/files/are
  default:
    boot_file: /the/boot/file.efi
"
sudo echo "$content" > /root/.config/preboot-oxide/preboot-oxide.yaml

# see sample.preboot-oxide.yaml in project root for more info on config
```

### Enable Service

```BASH
sudo systemd enable preboot-oxide

# starts
sudo systemd start preboot-oxide

# to check status use
sudo systemd status preboot-oxide
```

(see [Configuring](./configuring.md) or [Troubleshooting](./troubleshooting.md) in case of failure)


### Uninstalling

```BASH
sudo apt remove preboot-oxide
```