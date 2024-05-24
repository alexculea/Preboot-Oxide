# Quick Start (only Debian for now)

1. Ensure your server and booting client are on the same network
2. Install preboot-oxide, get OS, configure & start:
  
    ```BASH
    # donload the latest .deb package
    wget https://github.com/alexcule/preboot-oxide/releases/latest/download/preboot-oxide.deb

    # install it
    sudo dkpg -i preboot-oxide.deb

    # prepare a temporary boot folder (note this gets deleted at reboot, choose another location for permanent storage)
    mkdir -p /tmp/boot
    
    # download the debian installer (or any PXE supported OS installer)
    wget -O /tmp/deb-installer.tar.gz http://deb.debian.org/debian/dists/stable/main/installer-amd64/current/images/netboot/netboot.tar.gz

    # extract the installer archive to the temp directory we made earlier (adjust if changed previously)
    tar -xf /tmp/deb-installer.tar.gz -C /tmp/boot

    # provide config and start (for classic BIOS)
    PO_BOOT_FILE=pxelinux.0 PO_TFTP_SERVER_DIR_PATH=/tmp/boot/debian-installer/amd64 sudo preboot-oxide

    # for EFI
    PO_BOOT_FILE=bootnetx64.efi PO_TFTP_SERVER_DIR_PATH=/tmp/boot/debian-installer/amd64 sudo preboot-oxide

    # Done, booting over the network should now work! ❇️
    ```

    (see [Configuring](./configuring.md) or [Troubleshooting](./troubleshooting.md) otherwise)

