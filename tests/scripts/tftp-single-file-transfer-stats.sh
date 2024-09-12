#!/bin/bash

# This script just downloads a file from a TFTP server as configured in the variables below and it measures how long it takes
tftp_server_hostname="deb-dev.local"
tftp_file="debian-installer/amd64/initrd.gz"
working_dir="/tmp" # folder where to download the file

# Check if tftp command exists
if ! command -v tftp >/dev/null 2>&1; then
  echo "Error: tftp command not found. Please install tftp before running this script."
  exit 1
fi

if ! command -v bc >/dev/null 2>&1; then
  echo "Error: bc command not found. Please install bc before running this script."
  exit 1
fi

# Check if $working_dir is empty or /
if [ "$working_dir" = "/" ] || [ -z "$(ls -A $working_dir)" ]; then
  echo "Error: $working_dir is empty or root directory. Please specify a valid working directory."
  exit 1
fi

tftp_filename=$(basename "$tftp_file")
tftp_receive_file_path="/tmp/$tftp_filename"

if [ -e "$tftp_receive_file_path" ]; then
  echo "Error: $tftp_receive_file_path already exists. Please remove the file before running this script."
  exit 1
fi

cd $working_dir
download_start=$(date +%s.%N)
tftp "${tftp_server_hostname}" -c get "${tftp_file}"
download_end=$(date +%s.%N)
download_time_ns=$(bc <<< "$download_end - $download_start")
file_size_bytes=$(stat -c %s "$tftp_receive_file_path")
file_size_mb=$(bc <<< "scale=2; $file_size_bytes / (1024 * 1024)")

echo "File size: $file_size_bytes bytes"
echo "File size: $file_size_mb MB"
echo "Download time: $download_time_ns (seconds)"
echo "Download speed: $(bc <<< "scale=2; $file_size_bytes / $download_time_ns") bytes/second"
echo "Download speed: $(bc <<< "scale=2; $file_size_mb / $download_time_ns") MB/second"

rm "./${tftp_filename}"
cd -
