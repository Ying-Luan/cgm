PID_FILE := /tmp/cgm/cgm.pid
INSTALL_DIR := /usr/local/bin
INSTALL_PATH := $(INSTALL_DIR)/cgm

.PHONY: list check check-daemon install reinstall uninstall

# List all available commands
list:
	@echo "Available commands:"
	@echo "  list         - List all available commands"
	@echo "  check        - Run formatting, lint, and test checks"
	@echo "  check-daemon - Check if cgm daemon is running"
	@echo "  install      - Build and install cgm to $(INSTALL_PATH)"
	@echo "  reinstall    - Stop daemon, install the new binary, then start daemon"
	@echo "  uninstall    - Remove cgm from $(INSTALL_PATH)"

# Run formatting, lint, and test checks
check:
	cargo fmt -- --check
	cargo clippy
	cargo test

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

# Stop daemon, install the new binary, then start daemon
reinstall:
	-sudo cgm stop
	$(MAKE) install
	sudo cgm start

# Remove cgm from /usr/local/bin
uninstall: check-daemon
	sudo rm -f $(INSTALL_PATH)
	@echo "Uninstalled from $(INSTALL_PATH)"
