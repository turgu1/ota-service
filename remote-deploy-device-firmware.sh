#!/bin/bash
# ESPHome Firmware Remote Deployment Script (using SCP)
# This script deploys compiled ESPHome firmware to a remote OTA service firmware directory
#
# Usage: cd to the directory containing your ESPHome YAML file, then run:
#        <path-to-script>/remote-deploy-device-firmware.sh <device.yaml> <remote-host>
#
# Example: 
#   cd /path/to/esphome-configs
#   /path/to/ota-service/remote-deploy-device-firmware.sh esp32-kitchen.yaml user@server.local

set -e  # Exit on error

# Remote OTA Service Configuration File Path
# TODO: Adjust this path to match your remote OTA service configuration file location
REMOTE_OTA_CONFIG_FILE="/etc/ota-service/config.yaml"

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored messages
print_error() {
    echo -e "${RED}ERROR: $1${NC}" >&2
}

print_success() {
    echo -e "${GREEN}SUCCESS: $1${NC}"
}

print_info() {
    echo -e "${YELLOW}INFO: $1${NC}"
}

# Check if required arguments are provided
if [ $# -ne 2 ]; then
    print_error "Invalid number of arguments"
    echo "Usage: cd to the directory with your ESPHome YAML file, then:"
    echo "       $0 <device.yaml> <remote-host>"
    echo ""
    echo "Example:"
    echo "  cd /path/to/esphome-configs"
    echo "  $0 esp32-kitchen.yaml user@server.local"
    echo ""
    echo "Note: SSH key-based authentication is recommended for automation"
    exit 1
fi

ESPHOME_YAML_FILE="$1"
REMOTE_HOST="$2"

# Check if the YAML file exists in current directory
if [ ! -f "$ESPHOME_YAML_FILE" ]; then
    print_error "ESPHome YAML file not found in current directory: $ESPHOME_YAML_FILE"
    echo "Make sure you are in the directory containing the YAML file"
    exit 1
fi

print_info "Current Directory: $(pwd)"
print_info "Remote Host: $REMOTE_HOST"
print_info "Remote OTA Service Config: $REMOTE_OTA_CONFIG_FILE"
print_info "ESPHome Device Config: $ESPHOME_YAML_FILE"
echo ""

# Step 1: Test SSH connection to remote host
print_info "Testing SSH connection to remote host..."
if ! ssh -o BatchMode=yes -o ConnectTimeout=5 "$REMOTE_HOST" "echo 'SSH connection successful'" 2>/dev/null; then
    print_error "Cannot connect to remote host: $REMOTE_HOST"
    print_info "Make sure:"
    print_info "  1. The remote host is reachable"
    print_info "  2. SSH is configured properly"
    print_info "  3. You have SSH key-based authentication set up"
    print_info ""
    print_info "To set up SSH key authentication:"
    print_info "  ssh-copy-id $REMOTE_HOST"
    exit 1
fi
print_success "SSH connection established"

# Step 2: Extract firmware storage path from remote OTA service config
print_info "Extracting firmware storage path from remote OTA service config..."

# Use SSH to read the remote config file
FIRMWARE_FOLDER=$(ssh "$REMOTE_HOST" "grep -A 10 '^firmware:' '$REMOTE_OTA_CONFIG_FILE' | grep 'storage_path:' | awk '{print \$2}' | tr -d '\"' | tr -d \"'\"" 2>/dev/null)

if [ -z "$FIRMWARE_FOLDER" ]; then
    print_error "Could not extract firmware storage_path from remote $REMOTE_OTA_CONFIG_FILE"
    print_info "Make sure the OTA service is installed on the remote host"
    exit 1
fi

print_success "Remote firmware folder: $FIRMWARE_FOLDER"

# Step 3: Verify remote firmware folder exists and is writable
print_info "Verifying remote firmware folder permissions..."

if ! ssh "$REMOTE_HOST" "test -d '$FIRMWARE_FOLDER'" 2>/dev/null; then
    print_error "Remote firmware folder does not exist: $FIRMWARE_FOLDER"
    print_info "The OTA service must be installed first on the remote host."
    exit 1
fi

if ! ssh "$REMOTE_HOST" "test -w '$FIRMWARE_FOLDER'" 2>/dev/null; then
    print_error "No write permission to remote firmware folder: $FIRMWARE_FOLDER"
    print_info "You must be a member of the 'ota-service' group on the remote host."
    print_info "Ask the remote system administrator to run:"
    print_info "  sudo usermod -a -G ota-service <your-remote-username>"
    print_info "Then log out and back in on the remote host for the group change to take effect."
    REMOTE_USER=$(ssh "$REMOTE_HOST" "whoami" 2>/dev/null || echo "unknown")
    REMOTE_GROUPS=$(ssh "$REMOTE_HOST" "groups" 2>/dev/null || echo "unknown")
    print_info "Remote user: $REMOTE_USER"
    print_info "Remote groups: $REMOTE_GROUPS"
    exit 1
fi

print_success "Remote firmware folder is accessible and writable"

# Step 4: Extract device information from ESPHome YAML file
print_info "Extracting device information from ESPHome YAML..."

# Extract substitutions section and get values
# Look for device_id, device_name, and firmware_version in substitutions

DEVICE_ID=$(grep -A 20 "^substitutions:" "$ESPHOME_YAML_FILE" | grep "device_id:" | head -1 | awk '{print $2}' | tr -d '"' | tr -d "'")
DEVICE_NAME=$(grep -A 20 "^substitutions:" "$ESPHOME_YAML_FILE" | grep "device_name:" | head -1 | awk '{print $2}' | tr -d '"' | tr -d "'")
FIRMWARE_VERSION=$(grep -A 20 "^substitutions:" "$ESPHOME_YAML_FILE" | grep "firmware_version:" | head -1 | awk '{print $2}' | tr -d '"' | tr -d "'")

# Validate extracted values
if [ -z "$DEVICE_ID" ]; then
    print_error "Could not extract device_id from $ESPHOME_YAML_FILE"
    print_info "Make sure the YAML file has a 'device_id' in the substitutions section"
    exit 1
fi

if [ -z "$DEVICE_NAME" ]; then
    print_error "Could not extract device_name from $ESPHOME_YAML_FILE"
    print_info "Using device_id as device_name: $DEVICE_ID"
    DEVICE_NAME="$DEVICE_ID"
fi

if [ -z "$FIRMWARE_VERSION" ]; then
    print_error "Could not extract firmware_version from $ESPHOME_YAML_FILE"
    print_info "Make sure the YAML file has a 'firmware_version' in the substitutions section"
    exit 1
fi

print_success "Device ID: $DEVICE_ID"
print_success "Device Name: $DEVICE_NAME"
print_success "Firmware Version: $FIRMWARE_VERSION"
echo ""

# Step 5: Generate proper firmware filename
# Format: <device_id> - <version>.bin
FIRMWARE_FILENAME="${DEVICE_ID} - ${FIRMWARE_VERSION}.bin"
DESTINATION_PATH="${FIRMWARE_FOLDER}/${FIRMWARE_FILENAME}"

print_info "Generated firmware filename: $FIRMWARE_FILENAME"
print_info "Remote destination path: $DESTINATION_PATH"
echo ""

# Step 6: Locate source firmware binary
SOURCE_FIRMWARE=".esphome/build/${DEVICE_NAME}/.pioenvs/${DEVICE_NAME}/firmware.bin"

print_info "Looking for compiled firmware at: $SOURCE_FIRMWARE"

if [ ! -f "$SOURCE_FIRMWARE" ]; then
    print_error "Compiled firmware not found at: $SOURCE_FIRMWARE"
    print_info "Make sure you have compiled the ESPHome firmware first:"
    print_info "  esphome compile $ESPHOME_YAML_FILE"
    exit 1
fi

# Get source file size
SOURCE_SIZE=$(stat -f%z "$SOURCE_FIRMWARE" 2>/dev/null || stat -c%s "$SOURCE_FIRMWARE" 2>/dev/null)
print_success "Found firmware binary ($(numfmt --to=iec-i --suffix=B $SOURCE_SIZE 2>/dev/null || echo "${SOURCE_SIZE} bytes"))"

# Step 7: Copy firmware to remote destination using SCP
print_info "Copying firmware to remote OTA service directory via SCP..."

scp -q "$SOURCE_FIRMWARE" "${REMOTE_HOST}:${DESTINATION_PATH}"

if [ $? -eq 0 ]; then
    print_success "Firmware deployed successfully to remote host!"
    echo ""
    print_info "Summary:"
    print_info "  Source: $SOURCE_FIRMWARE"
    print_info "  Remote Host: $REMOTE_HOST"
    print_info "  Remote Destination: $DESTINATION_PATH"
    print_info "  Device ID: $DEVICE_ID"
    print_info "  Firmware Version: $FIRMWARE_VERSION"
    echo ""
    print_success "The OTA service will detect this firmware on the next check cycle."
else
    print_error "Failed to copy firmware file to remote host"
    exit 1
fi

# Optional: List all firmware files for this device on remote host
echo ""
print_info "All firmware versions for device '$DEVICE_ID' on remote host:"
ssh "$REMOTE_HOST" "ls -lh '$FIRMWARE_FOLDER' | grep '$DEVICE_ID'" || echo "  (none found)"

exit 0
