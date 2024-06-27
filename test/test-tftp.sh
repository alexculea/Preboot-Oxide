#!/bin/bash

## Looks at YAML configuration for where the TFTP hosted directory is
## Lists all files in the directory and downloads them over TFTP to a temporary folder
## Caputes memory used before the test and after the test

## Config
process_name=preboot-oxide
yaml_conf_path=~/.config/preboot-oxide/preboot-oxide.yaml

# Find process with name preboot-oxide
preboot_oxide_pid=$(pgrep "$process_name")
start_resident_memory=0

# Check if the process is running
if [ -n "$preboot_oxide_pid" ]; then
  echo "$process_name process found with PID: $preboot_oxide_pid"
  # Get the resident memory of the process
  start_resident_memory=$(grep -s "VmRSS" /proc/"$preboot_oxide_pid"/status | awk '{print $2}')
  start_resident_memory=$(("$start_resident_memory" / 1024))
  echo "Resident memory of preboot-oxide process: ${start_resident_memory}MB"
else
  echo "preboot-oxide process not found"
  exit 1
fi

# Read tftp_server_dir from YAML file
tftp_dir=$(grep -Po 'tftp_server_dir:\s*\K.*' "$yaml_conf_path")

# Use the tftp_dir variable in your script
tftp_dir_files=$(find "$tftp_dir" -type f)

# Generate random tmp folder
tmp_folder=$(cat /dev/urandom | tr -dc 'a-zA-Z0-9' | fold -w 10 | head -n 1)
tmp_folder="/tmp/po-test-$tmp_folder"
mkdir -p "$tmp_folder" && cd "$tmp_folder"

while IFS= read -r file; do 
  # Subtract tftp_dir path from file
  relative_path=$(echo "$file" | sed "s|$tftp_dir||")

  tftp deb-dev.local -c get "${relative_path}"
  if [ $? -eq 0 ]; then
    echo "While processing $file"
  fi
done <<< "$tftp_dir_files"
cd -

end_resident_memory=$(grep -s "VmRSS" /proc/"$preboot_oxide_pid"/status | awk '{print $2}')
end_resident_memory=$(("$end_resident_memory" / 1024))
echo "Resident memory of preboot-oxide process after TFTP test: ${end_resident_memory}MB"

rm -rf "/tmp/$tmp_folder"


