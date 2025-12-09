#!/bin/bash
# ESPHome Firmware Deployment Script
# This script deploys compiled ESPHome firmware to the OTA service firmware directory
#
# Usage: cd to the directory containing your ESPHome YAML file, then run:
#        <path-to-script>/deploy-firmware.sh <device.yaml>
#
# Example: 
#   cd /path/to/esphome-configs
#   /path/to/ota-service/deploy-firmware.sh esp32-kitchen.yaml

set -e  # Exit on error

# OTA Service Configuration File
# TODO: Adjust this path to match your OTA service configuration file location
OTA_CONFIG_FILE="$HOME/Dev/ota-service/test-data/config.yaml"

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
if [ $# -ne 1 ]; then
    print_error "Invalid number of arguments"
    echo "Usage: cd to the directory with your ESPHome YAML file, then:"
    echo "       $0 <device.yaml>"
    echo ""
    echo "Example:"
    echo "  cd /path/to/esphome-configs"
    echo "  $0 esp32-kitchen.yaml"
    exit 1
fi

ESPHOME_YAML_FILE="$1"

# Check if the YAML file exists in current directory
if [ ! -f "$ESPHOME_YAML_FILE" ]; then
    print_error "ESPHome YAML file not found in current directory: $ESPHOME_YAML_FILE"
    echo "Make sure you are in the directory containing the YAML file"
    exit 1
fi

# Validate OTA service config file exists
if [ ! -f "$OTA_CONFIG_FILE" ]; then
    print_error "OTA service config file not found: $OTA_CONFIG_FILE"
    exit 1
fi

print_info "Current Directory: $(pwd)"
print_info "OTA Service Config: $OTA_CONFIG_FILE"
print_info "ESPHome Device Config: $ESPHOME_YAML_FILE"
echo ""

# Step 1: Extract firmware storage path from OTA service config
print_info "Extracting firmware storage path from OTA service config..."

# Use grep and awk to extract the storage_path value
FIRMWARE_FOLDER=$(grep -A 10 "^firmware:" "$OTA_CONFIG_FILE" | grep "storage_path:" | awk '{print $2}' | tr -d '"' | tr -d "'")

if [ -z "$FIRMWARE_FOLDER" ]; then
    print_error "Could not extract firmware storage_path from $OTA_CONFIG_FILE"
    exit 1
fi

print_success "Firmware folder: $FIRMWARE_FOLDER"

# Create firmware folder if it doesn't exist
if [ ! -d "$FIRMWARE_FOLDER" ]; then
    print_info "Creating firmware folder: $FIRMWARE_FOLDER"
    mkdir -p "$FIRMWARE_FOLDER"
fi

# Step 2: Extract device information from ESPHome YAML file
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

# Step 3: Generate proper firmware filename
# Format: <device_id> - <version>.bin
FIRMWARE_FILENAME="${DEVICE_ID} - ${FIRMWARE_VERSION}.bin"
DESTINATION_PATH="${FIRMWARE_FOLDER}/${FIRMWARE_FILENAME}"

print_info "Generated firmware filename: $FIRMWARE_FILENAME"
print_info "Destination path: $DESTINATION_PATH"
echo ""

# Step 4: Locate source firmware binary
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

# Step 5: Copy firmware to destination
print_info "Copying firmware to OTA service directory..."

cp "$SOURCE_FIRMWARE" "$DESTINATION_PATH"

if [ $? -eq 0 ]; then
    print_success "Firmware deployed successfully!"
    echo ""
    print_info "Summary:"
    print_info "  Source: $SOURCE_FIRMWARE"
    print_info "  Destination: $DESTINATION_PATH"
    print_info "  Device ID: $DEVICE_ID"
    print_info "  Firmware Version: $FIRMWARE_VERSION"
    echo ""
    print_success "The OTA service will detect this firmware on the next check cycle."
else
    print_error "Failed to copy firmware file"
    exit 1
fi

# Optional: List all firmware files for this device
echo ""
print_info "All firmware versions for device '$DEVICE_ID':"
ls -lh "$FIRMWARE_FOLDER" | grep "$DEVICE_ID" || echo "  (none found)"

exit 0
