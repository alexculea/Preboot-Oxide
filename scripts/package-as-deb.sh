#!/bin/bash

if [ "$EUID" -ne 0 ]
  then echo "Please run as root"
  exit
fi

SCRIPTS_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

# Read name and version from Cargo.toml
PACKAGE_NAME=$(grep -m1 "name" Cargo.toml | awk -F '"' '{print $2}')
PACKAGE_VERSION=$(grep -m1 "version" Cargo.toml | awk -F '"' '{print $2}')

# Create a temporary directory for the package contents
PACKAGE_DIR="/tmp/${PACKAGE_NAME}_${PACKAGE_VERSION}"
mkdir -p "$PACKAGE_DIR/bin/"
# Copy files to the package directory
cp $(realpath "$SCRIPTS_DIR/../target/release/preboot-oxide") "$PACKAGE_DIR/bin/preboot-oxide"
cp -R $(realpath "$SCRIPTS_DIR/../assets/deb/*") "$PACKAGE_DIR"

chown -R root:root "$PACKAGE_DIR"
chmod +x "$PACKAGE_DIR/bin/preboot-oxide"

# Build the package using dpkg-deb
dpkg-deb --build "$PACKAGE_DIR"

mv "${PACKAGE_DIR}.deb" ./
rm -rf "$PACKAGE_DIR"