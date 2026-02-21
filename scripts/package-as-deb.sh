#!/bin/bash

SCRIPTS_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
PROJECT_DIR=$(realpath "$SCRIPTS_DIR/..")

# Read name and version from Cargo.toml
PACKAGE_NAME=$(grep -m1 "name" Cargo.toml | awk -F '"' '{print $2}')
PACKAGE_VERSION=$(grep -m1 "version" Cargo.toml | awk -F '"' '{print $2}')

# Check if binary exists
if [ ! -f "$PROJECT_DIR/target/release/preboot-oxide" ]; then
    echo "Error: Binary not found. Please run 'cargo build --release' first."
    exit 1
fi

# Check binary version
ACTUAL_VERSION=$("$PROJECT_DIR/target/release/preboot-oxide" -V | awk '{print $2}')
if [ "$ACTUAL_VERSION" != "$PACKAGE_VERSION" ]; then
    echo "Error: Version mismatch between Cargo.toml and the built binary at /target/release/preboot-oxide. Expected $PACKAGE_VERSION, found $ACTUAL_VERSION."
    echo "Did you forget to run 'cargo build --release'?"
    exit 1
fi

if [ "$EUID" != 0 ]; then
  sudo "$0" "$@"
  exit $?
fi
PACKAGE_NAME=$(grep -m1 "name" Cargo.toml | awk -F '"' '{print $2}')
PACKAGE_VERSION=$(grep -m1 "version" Cargo.toml | awk -F '"' '{print $2}')

# Create a temporary directory for the package contents
PACKAGE_DIR="/tmp/${PACKAGE_NAME}_${PACKAGE_VERSION}"
mkdir -p "$PACKAGE_DIR/bin/"
# Copy files to the package directory
cp $(realpath "$SCRIPTS_DIR/../target/release/preboot-oxide") "$PACKAGE_DIR/bin/preboot-oxide"
cp -R $(realpath "$SCRIPTS_DIR/../assets/deb/*") "$PACKAGE_DIR"
mkdir -p "$PACKAGE_DIR/usr/lib/systemd/system"
cp $(realpath "$SCRIPTS_DIR/../assets/shared/preboot-oxide.service") "$PACKAGE_DIR/usr/lib/systemd/system/"

chown -R root:root "$PACKAGE_DIR"
chmod +x "$PACKAGE_DIR/bin/preboot-oxide"

# Update the package version
CONTROL_FILE="$PACKAGE_DIR/DEBIAN/control"
sed -E -i "s/^Version: [0-9]+\.[0-9]+\.[0-9]+/Version: ${PACKAGE_VERSION}/" "$CONTROL_FILE"

# Build the package using dpkg-deb
dpkg-deb --build "$PACKAGE_DIR"

mv "${PACKAGE_DIR}.deb" ./
rm -rf "$PACKAGE_DIR"

# rename package to include architecture & keep the latest link working in the docs
mv "${PACKAGE_NAME}_${PACKAGE_VERSION}.deb" "${PACKAGE_NAME}-amd64.deb"