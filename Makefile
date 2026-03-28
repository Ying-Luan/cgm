PID_FILE := /tmp/cgm/cgm.pid
INSTALL_DIR := /usr/local/bin
INSTALL_PATH := $(INSTALL_DIR)/cgm

.PHONY: list check-daemon install uninstall

# List all available commands
list:
	@echo "Available commands:"
	@echo "  list         - List all available commands"
	@echo "  check-daemon - Check if cgm daemon is running"
	@echo "  install      - Build and install cgm to $(INSTALL_PATH)"
	@echo "  uninstall    - Remove cgm from $(INSTALL_PATH)"

# Check if cgm daemon is running
check-daemon:
	@if cgm status 2>/dev/null | sed 's/\x1b\[[0-9;]*m//g' | grep -q "Daemon: ACTIVE"; then \
		echo "Error: cgm daemon is running. Please run 'sudo cgm stop' first."; \
		exit 1; \
	fi

# Build release binary and install to /usr/local/bin
install: check-daemon
	cargo build --release
	sudo cp ./target/release/cgm $(INSTALL_PATH)
	sudo chmod +x $(INSTALL_PATH)
	@echo "Installed to $(INSTALL_PATH)"

# Remove cgm from /usr/local/bin
uninstall: check-daemon
	sudo rm -f $(INSTALL_PATH)
	@echo "Uninstalled from $(INSTALL_PATH)"
