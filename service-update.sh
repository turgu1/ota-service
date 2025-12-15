#!/bin/bash

# OTA Service Update Script
# Automates updating the OTA service on a Linux server
#
# Usage: sudo ./service-update.sh /path/to/ota-service-project
#
# This script must be run as root or with sudo

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;36m'
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

print_substep() {
    echo -e "${BLUE}>>> $1${NC}"
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

# Verify service is installed
if [ ! -f "/usr/local/bin/ota-service" ]; then
    print_error "OTA service binary not found at /usr/local/bin/ota-service"
    print_error "Service does not appear to be installed. Use install.sh first."
    exit 1
fi

# Verify systemd service exists
if [ ! -f "/etc/systemd/system/ota-service.service" ]; then
    print_error "Systemd service file not found at /etc/systemd/system/ota-service.service"
    print_error "Service does not appear to be installed. Use install.sh first."
    exit 1
fi

print_info "OTA Service Update"
print_info "Project path: $PROJECT_PATH"
echo ""

# Optional: Update source code
read -p "Do you want to pull latest changes from git? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    print_step "Pulling latest changes from git"
    cd "$PROJECT_PATH"
    if [ -d ".git" ]; then
        print_info "Running: git pull"
        # Run git as original user to avoid permission issues
        if [ -n "$SUDO_USER" ]; then
            sudo -u "$SUDO_USER" -H bash -c "cd '$PROJECT_PATH' && git pull"
        else
            git pull
        fi
    else
        print_warn "Not a git repository, skipping git pull"
    fi
fi

# Step 1: Build new version
print_step "Step 1: Building new version"
cd "$PROJECT_PATH"

# Detect the original user (before sudo)
if [ -n "$SUDO_USER" ]; then
    ORIGINAL_USER="$SUDO_USER"
    ORIGINAL_HOME=$(eval echo ~$SUDO_USER)
    print_info "Building as user: $ORIGINAL_USER (cargo requires user environment)"
    print_info "Running: cargo build --release"
    # Run cargo as the original user with their full environment
    sudo -u "$ORIGINAL_USER" -H bash -c "source $ORIGINAL_HOME/.cargo/env 2>/dev/null || true; cd '$PROJECT_PATH' && cargo build --release"
else
    print_info "Running: cargo build --release"
    cargo build --release
fi

if [ ! -f "$PROJECT_PATH/target/release/ota-service" ]; then
    print_error "Build failed - binary not found at $PROJECT_PATH/target/release/ota-service"
    exit 1
fi
print_info "Build completed successfully"

# Step 2: Check current service status
print_step "Step 2: Checking service status"
if systemctl is-active --quiet ota-service; then
    print_info "Service is currently running"
    SERVICE_WAS_RUNNING=true
else
    print_warn "Service is not currently running"
    SERVICE_WAS_RUNNING=false
fi

# Step 3: Stop the service
print_step "Step 3: Stopping service"
if systemctl is-active --quiet ota-service; then
    print_info "Stopping ota-service..."
    systemctl stop ota-service
    
    # Wait for service to fully stop
    sleep 2
    
    if systemctl is-active --quiet ota-service; then
        print_error "Failed to stop service"
        exit 1
    fi
    print_info "Service stopped successfully"
else
    print_info "Service already stopped"
fi

# Step 4: Backup current binary
print_step "Step 4: Backing up current binary"
BACKUP_PATH="/usr/local/bin/ota-service.backup"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_PATH_TIMESTAMPED="/usr/local/bin/ota-service.backup.$TIMESTAMP"

print_info "Creating backup at $BACKUP_PATH_TIMESTAMPED"
cp /usr/local/bin/ota-service "$BACKUP_PATH_TIMESTAMPED"
# Also create/update the generic backup
cp /usr/local/bin/ota-service "$BACKUP_PATH"
print_info "Backup created successfully"
print_info "Old binary saved as: $BACKUP_PATH_TIMESTAMPED"

# Step 5: Install new binary
print_step "Step 5: Installing new binary"
print_info "Copying new binary to /usr/local/bin/ota-service"
cp "$PROJECT_PATH/target/release/ota-service" /usr/local/bin/
chown root:root /usr/local/bin/ota-service
chmod 755 /usr/local/bin/ota-service
print_info "New binary installed successfully"
ls -l /usr/local/bin/ota-service

# Step 6: Check if service file needs updating
print_step "Step 6: Checking service file"
if [ -f "$PROJECT_PATH/ota-service.service" ]; then
    if ! cmp -s "$PROJECT_PATH/ota-service.service" "/etc/systemd/system/ota-service.service"; then
        print_warn "Service file has changed"
        read -p "Update systemd service file? (y/N): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            print_info "Updating service file"
            cp "$PROJECT_PATH/ota-service.service" /etc/systemd/system/
            chmod 644 /etc/systemd/system/ota-service.service
            print_info "Reloading systemd daemon"
            systemctl daemon-reload
            print_info "Service file updated"
        fi
    else
        print_info "Service file unchanged, no update needed"
    fi
fi

# Step 7: Start the service
print_step "Step 7: Starting service"
print_info "Starting ota-service..."
systemctl start ota-service

# Wait for service to start
sleep 2

# Step 8: Verify service is running
print_step "Step 8: Verifying service status"
if systemctl is-active --quiet ota-service; then
    print_info "Service started successfully!"
    systemctl status ota-service --no-pager -l
else
    print_error "Service failed to start"
    print_error "Check logs with: sudo journalctl -u ota-service -n 50"
    print_error ""
    print_error "To rollback to previous version:"
    print_error "  sudo systemctl stop ota-service"
    print_error "  sudo cp $BACKUP_PATH /usr/local/bin/ota-service"
    print_error "  sudo systemctl start ota-service"
    exit 1
fi

# Final summary
print_step "Update Complete!"
echo ""
print_info "Update Summary:"
echo "  • New binary installed: /usr/local/bin/ota-service"
echo "  • Backup saved: $BACKUP_PATH_TIMESTAMPED"
echo "  • Service status: Running"
echo ""
print_info "Useful commands:"
echo "  • View logs: sudo journalctl -u ota-service -f"
echo "  • Check status: sudo systemctl status ota-service"
echo "  • Restart service: sudo systemctl restart ota-service"
echo ""
if [ -f "$BACKUP_PATH_TIMESTAMPED" ]; then
    print_info "To rollback to previous version:"
    echo "  sudo systemctl stop ota-service"
    echo "  sudo cp $BACKUP_PATH_TIMESTAMPED /usr/local/bin/ota-service"
    echo "  sudo systemctl start ota-service"
fi
echo ""
print_info "Update completed successfully!"
