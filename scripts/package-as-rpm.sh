#!/bin/bash

SCRIPTS_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
PROJECT_DIR="$( cd "$SCRIPTS_DIR/.." && pwd )"

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

if ! command -v rpmbuild &> /dev/null; then
    echo "Error: rpmbuild is not installed. Please install rpm-build package."
    exit 1
fi

if [ ! -f "$PROJECT_DIR/assets/shared/preboot-oxide.service" ]; then
    echo "Error: Service file not found in assets/shared/. Please ensure shared assets are properly set up."
    exit 1
fi

if [ ! -f "$PROJECT_DIR/assets/rpm/package.spec" ]; then
    echo "Error: Spec file not found in assets/rpm/. Please ensure RPM assets are properly set up."
    exit 1
fi

# Create rpmbuild directories if they don't exist
RPMBUILD_DIR="/tmp/preboot-oxide/rpmbuild"
mkdir -p "$RPMBUILD_DIR/SOURCES" "$RPMBUILD_DIR/SPECS" "$RPMBUILD_DIR/BUILD" "$RPMBUILD_DIR/BUILDROOT" "$RPMBUILD_DIR/RPMS" "$RPMBUILD_DIR/SRPMS"

# Copy binary and service file to SOURCES
cp "$PROJECT_DIR/target/release/preboot-oxide" "$RPMBUILD_DIR/SOURCES/"
cp "$PROJECT_DIR/assets/shared/preboot-oxide.service" "$RPMBUILD_DIR/SOURCES/"

# Copy spec file and update version
SPEC_FILE="$RPMBUILD_DIR/SPECS/${PACKAGE_NAME}.spec"
cp "$PROJECT_DIR/assets/rpm/package.spec" "$SPEC_FILE"
sed -i "s/^Version:.*/Version: ${PACKAGE_VERSION}/" "$SPEC_FILE"

# Build the RPM
cd "$RPMBUILD_DIR"
rpmbuild --define "_topdir $RPMBUILD_DIR" -ba "SPECS/${PACKAGE_NAME}.spec"

# Move the built RPM to the project directory
RPM_FILE="$RPMBUILD_DIR/RPMS/x86_64/${PACKAGE_NAME}-${PACKAGE_VERSION}-1.x86_64.rpm"
if [ -f "$RPM_FILE" ]; then
    mv "$RPM_FILE" "$PROJECT_DIR/preboot-oxide.x86_64.rpm"
    echo "RPM package built: preboot-oxide.x86_64.rpm"
else
    echo "Error: RPM build failed."
    exit 1
fi
