.PHONY: list check check-mode build check-daemon install reinstall uninstall

# Resolve install paths and privileges for the selected mode.
ifeq ($(MODE),user)
BIN = $(HOME)/.local/bin/cgm
SUDO =
else
BIN = /usr/local/bin/cgm
SUDO = $(shell if [ "$$(id -u)" -ne 0 ]; then echo sudo; fi)
endif

# List all available commands
list:
	@echo "Available commands:"
	@echo "  list         - List all available commands"
	@echo "  check        - Run formatting, lint, and test checks"
	@echo "  check-mode   - Check if MODE is set to user or system"
	@echo "  build        - Build the release binary"
	@echo "  check-daemon - Check if cgm daemon is running"
	@echo "  install      - Build and install cgm (MODE=user or MODE=system)"
	@echo "  reinstall    - Stop daemon, install the new binary, then start daemon"
	@echo "  uninstall    - Remove cgm"

# Run formatting, lint, and test checks
check:
	cargo fmt -- --check
	cargo clippy
	cargo test

# Check if MODE is set to user or system
check-mode:
	@case "$(MODE)" in \
		user|system) \
			;; \
		*) \
			echo "Error: specify MODE=user or MODE=system" >&2; \
			exit 1 \
			;; \
	esac

# Build the release binary
build:
	cargo build --release

# Check if cgm daemon is running
check-daemon: build
	@./target/release/cgm status --quiet; status=$$?; \
	case $$status in \
	  0) echo "Error: daemon socket occupied" >&2; exit 1;; \
	  1) ;; \
	  *) echo "Error: cannot verify daemon status (exit $$status)" >&2; exit 1;; \
	esac

# Build and install cgm (MODE=user or MODE=system)
install: check-mode check-daemon build
	$(SUDO) install -D -m 0755 ./target/release/cgm "$(BIN)"
	@echo "Installed to $(BIN)"

# Stop daemon, install the new binary, then start daemon
reinstall: check-mode
	-$(SUDO) cgm stop
	$(MAKE) install MODE="$(MODE)"
	$(SUDO) cgm start --$(MODE)

# Remove cgm
uninstall: check-mode check-daemon
	$(SUDO) rm -f "$(BIN)"
	@echo "Uninstalled from $(BIN)"
