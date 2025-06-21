# Quick Start via .deb

1. Ensure your server and booting client are on the same network
2. Install preboot-oxide, get OS, configure & start:
  
    ```BASH
    # download and install
    wget https://github.com/alexculea/preboot-oxide/releases/latest/download/preboot-oxide-amd64.deb
    sudo dpkg -i preboot-oxide-amd64.deb

    # prepare a temporary boot folder
    mkdir -p /tmp/boot
    
    # download the debian PXE installer (or any PXE supported OS installer)
    wget -O /tmp/deb-installer.tar.gz http://deb.debian.org/debian/dists/stable/main/installer-amd64/current/images/netboot/netboot.tar.gz

    # extract the installer archive
    tar -xf /tmp/deb-installer.tar.gz -C /tmp/boot

    # provide config and start (for EFI)
    PO_BOOT_FILE=bootnetx64.efi PO_TFTP_SERVER_DIR_PATH=/tmp/boot/debian-installer/amd64 sudo preboot-oxide

    # provide config and start (for classic BIOS)
    PO_BOOT_FILE=pxelinux.0 PO_TFTP_SERVER_DIR_PATH=/tmp/boot/debian-installer/amd64 sudo preboot-oxide

    # Done, booting over the network should now work! ❇️
    # If all good, stop the program (Ctrl+C) and enable the service to be always on
    sudo systemctl enable preboot-oxide 
    ```
3. **Optional**, consider setting a higher verbosity level to oversee the booting process or troubleshoot issues

    ```BASH
    # for EFI
    PO_BOOT_FILE=bootnetx64.efi PO_TFTP_SERVER_DIR_PATH=/tmp/boot/debian-installer/amd64 sudo preboot-oxide -vv
    ```

    See [Manual](./manual.md) or [Troubleshooting](./troubleshooting.md).

