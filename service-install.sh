#!/bin/bash

# OTA Service Installation Script
# Automates the installation process described in doc/SERVICE_INSTALL.md
#
# Usage: sudo ./service-install.sh /path/to/ota-service-project
#
# This script must be run as root or with sudo

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored messages
print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_step() {
    echo ""
    echo -e "${GREEN}========================================${NC}"
    echo -e "${GREEN}$1${NC}"
    echo -e "${GREEN}========================================${NC}"
}

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    print_error "This script must be run as root or with sudo"
    echo "Usage: sudo $0 /path/to/ota-service-project"
    exit 1
fi

# Check if project path is provided
if [ -z "$1" ]; then
    print_error "Project path not provided"
    echo "Usage: sudo $0 /path/to/ota-service-project"
    exit 1
fi

PROJECT_PATH="$1"

# Verify project path exists
if [ ! -d "$PROJECT_PATH" ]; then
    print_error "Project path does not exist: $PROJECT_PATH"
    exit 1
fi

# Verify Cargo.toml exists
if [ ! -f "$PROJECT_PATH/Cargo.toml" ]; then
    print_error "Cargo.toml not found in $PROJECT_PATH"
    print_error "Please provide the path to the ota-service project root"
    exit 1
fi

print_info "OTA Service Installation"
print_info "Project path: $PROJECT_PATH"
echo ""

# Step 1: Build the service
print_step "Step 1: Building the service"
cd "$PROJECT_PATH"
print_info "Running: cargo build --release"
cargo build --release

if [ ! -f "$PROJECT_PATH/target/release/ota-service" ]; then
    print_error "Build failed - binary not found at $PROJECT_PATH/target/release/ota-service"
    exit 1
fi
print_info "Build completed successfully"

# Step 2: Create dedicated user
print_step "Step 2: Creating dedicated system user"
if id "ota-service" &>/dev/null; then
    print_warn "User 'ota-service' already exists, skipping creation"
else
    print_info "Creating system user 'ota-service'"
    useradd -r -s /bin/false -d /var/lib/ota-service ota-service
    print_info "User created successfully"
fi

# Step 3: Create directory structure
print_step "Step 3: Creating directory structure"
print_info "Creating directories..."
mkdir -p /etc/ota-service
mkdir -p /var/lib/ota-service/firmware
mkdir -p /var/log/ota-service

print_info "Setting ownership..."
chown -R ota-service:ota-service /var/lib/ota-service
chown -R ota-service:ota-service /var/log/ota-service

print_info "Setting permissions..."
chmod 755 /var/lib/ota-service
chmod 770 /var/lib/ota-service/firmware
chmod 755 /var/log/ota-service
print_info "Directory structure created successfully"
print_warn "Firmware folder has 770 permissions (ota-service group access)"
print_warn "Users deploying firmware must be added to the ota-service group:"
print_warn "  sudo usermod -a -G ota-service <username>"

# Step 4: Install binary
print_step "Step 4: Installing binary"
print_info "Copying binary to /usr/local/bin/ota-service"
cp "$PROJECT_PATH/target/release/ota-service" /usr/local/bin/
chown root:root /usr/local/bin/ota-service
chmod 755 /usr/local/bin/ota-service
print_info "Binary installed successfully"
ls -l /usr/local/bin/ota-service

# Step 5: Install configuration
print_step "Step 5: Installing configuration"
if [ -f "/etc/ota-service/config.yaml" ]; then
    print_warn "Configuration file already exists at /etc/ota-service/config.yaml"
    print_warn "Creating backup at /etc/ota-service/config.yaml.backup"
    cp /etc/ota-service/config.yaml /etc/ota-service/config.yaml.backup
fi

if [ -f "$PROJECT_PATH/config.example.yaml" ]; then
    print_info "Copying config.example.yaml to /etc/ota-service/config.yaml"
    cp "$PROJECT_PATH/config.example.yaml" /etc/ota-service/config.yaml
    chown ota-service:ota-service /etc/ota-service/config.yaml
    chmod 640 /etc/ota-service/config.yaml
    print_info "Configuration installed successfully"
else
    print_error "config.example.yaml not found in $PROJECT_PATH"
    exit 1
fi

# Step 6: Install systemd service
print_step "Step 6: Installing systemd service"
if [ -f "$PROJECT_PATH/ota-service.service" ]; then
    print_info "Copying ota-service.service to /etc/systemd/system/"
    cp "$PROJECT_PATH/ota-service.service" /etc/systemd/system/
    chmod 644 /etc/systemd/system/ota-service.service
    print_info "Reloading systemd configuration"
    systemctl daemon-reload
    print_info "Systemd service installed successfully"
else
    print_error "ota-service.service not found in $PROJECT_PATH"
    exit 1
fi

# Step 7: Enable and start service
print_step "Step 7: Enabling and starting service"
print_info "Enabling service to start at boot"
systemctl enable ota-service

print_info "Starting service"
systemctl start ota-service

# Wait a moment for service to start
sleep 2

# Check service status
print_info "Checking service status..."
if systemctl is-active --quiet ota-service; then
    print_info "Service is running!"
    systemctl status ota-service --no-pager -l
else
    print_error "Service failed to start"
    print_error "Check logs with: sudo journalctl -u ota-service -n 50"
    exit 1
fi

# Final summary
print_step "Installation Complete!"
echo ""
print_info "Installation Summary:"
echo "  • Service user: ota-service"
echo "  • Binary location: /usr/local/bin/ota-service"
echo "  • Configuration: /etc/ota-service/config.yaml"
echo "  • Database: /var/lib/ota-service/devices.db"
echo "  • Firmware storage: /var/lib/ota-service/firmware/ (770 permissions)"
echo "  • Logs: /var/log/ota-service/"
echo "  • Systemd service: /etc/systemd/system/ota-service.service"
echo ""
print_warn "IMPORTANT: Users deploying firmware must be added to ota-service group:"
echo "  sudo usermod -a -G ota-service <username>"
echo "  (User must log out and back in for group change to take effect)"
echo ""
print_warn "IMPORTANT: Edit /etc/ota-service/config.yaml with your settings:"
echo "  • MQTT broker settings (host, username, password)"
echo "  • Web interface password (change from default!)"
echo "  • OTA password (if using authentication)"
echo ""
print_info "After editing config.yaml, restart the service:"
echo "  sudo systemctl restart ota-service"
echo ""
print_info "Useful commands:"
echo "  • View logs: sudo journalctl -u ota-service -f"
echo "  • Check status: sudo systemctl status ota-service"
echo "  • Restart service: sudo systemctl restart ota-service"
echo "  • Stop service: sudo systemctl stop ota-service"
echo ""
if [ -f "/etc/ota-service/config.yaml" ]; then
    WEB_PORT=$(grep -A 10 "^web:" /etc/ota-service/config.yaml | grep "port:" | awk '{print $2}')
    if [ -n "$WEB_PORT" ]; then
        print_info "Access web interface at: http://localhost:$WEB_PORT"
    fi
fi
echo ""
print_info "For detailed documentation, see:"
echo "  • $PROJECT_PATH/doc/SERVICE_INSTALL.md"
echo "  • $PROJECT_PATH/doc/WEB_INTERFACE.md"
echo "  • $PROJECT_PATH/README.md"
