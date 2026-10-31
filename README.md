<div align="center">

# Convenient GPU Manager

A lightweight GPU job scheduler

[![GitHub License](https://img.shields.io/github/license/Ying-Luan/cgm)](LICENSE)
[![Codacy Badge](https://app.codacy.com/project/badge/Grade/8853a13a90ac4417b70c544c297655ac)](https://app.codacy.com/gh/Ying-Luan/cgm/dashboard?utm_source=gh&utm_medium=referral&utm_content=&utm_campaign=Badge_grade)

[English](README.md) | [中文](README_zh.md)

[Usage Docs](USAGE.md) | [使用文档](USAGE_zh.md)

[GitHub](https://github.com/Ying-Luan/cgm) | [Gitee](https://gitee.com/ying_luan/cgm)

</div>

---

## Features

* Multi-user GPU resource sharing with automatic queuing and scheduling
* Automatic allocation of idle GPUs and `CUDA_VISIBLE_DEVICES` setup
* Environment snapshot at submission time, compatible with conda, venv, etc.
* Detached mode: closing the terminal after submission does not affect execution
* Complete job lifecycle management (submit, cancel, delete, view logs)
* Automatic detection of external GPU usage to avoid resource conflicts

## Project Structure

```text
cgm/
├── .github/
│   └── workflows/
│       └── release.yml
├── src/
│   ├── cli.rs               # CLI module entry, defines command structure
│   ├── cli/
│   │   ├── cancel.rs        # cgm cancel
│   │   ├── config.rs        # cgm config
│   │   ├── delete.rs        # cgm delete
│   │   ├── info.rs          # cgm info
│   │   ├── list.rs          # cgm list
│   │   ├── log.rs           # cgm log
│   │   ├── rerun.rs         # cgm rerun
│   │   ├── start.rs         # cgm start
│   │   ├── status.rs        # cgm status
│   │   ├── stop.rs          # cgm stop
│   │   ├── submit.rs        # cgm submit
│   │   └── utils.rs         # CLI shared helpers
│   ├── client.rs            # Socket client for daemon communication
│   ├── config.rs            # Configuration module entry
│   ├── config/
│   │   ├── load.rs          # Configuration loading and merging
│   │   ├── manage.rs        # Configuration management
│   │   ├── path.rs          # Configuration path resolution
│   │   └── validate.rs      # Configuration validation
│   ├── constants.rs         # Global constants
│   ├── daemon.rs            # Daemon module entry
│   ├── daemon/
│   │   ├── control.rs       # Cancel/stop events and senders
│   │   ├── executor.rs      # Job execution and mpsc lifecycle control
│   │   ├── process.rs       # Daemon start/stop
│   │   ├── scheduler.rs     # Scheduler trait and common functions
│   │   ├── scheduler/
│   │   │   ├── fifo.rs      # FIFO scheduler
│   │   │   └── greedy.rs    # Greedy scheduler
│   │   └── server.rs        # Socket request handling
│   ├── db.rs                # Database module entry
│   ├── db/
│   │   ├── operations.rs    # SQLite CRUD
│   │   └── schema.rs        # Database schema
│   ├── hardware.rs          # Hardware module entry
│   ├── hardware/
│   │   └── gpu.rs           # GPU state management
│   ├── macros.rs            # Macro definitions
│   ├── main.rs              # Program entry point
│   ├── monitor.rs           # Status display (show_status, show_list)
│   ├── os.rs                # User info and permission checks
│   ├── runtime.rs           # Daemon mode, identity, and runtime metadata
│   ├── runtime/
│   │   ├── identity.rs      # Socket peer credentials and authorization
│   │   ├── paths.rs         # State paths and permissions
│   │   └── socket.rs        # Machine-wide abstract socket
│   ├── types.rs             # Types module entry
│   ├── types/
│   │   ├── config.rs        # Configuration types, scope, and source
│   │   ├── daemon.rs        # Daemon mode, state, and metadata
│   │   ├── gpu.rs           # GpuInfo, GpuState
│   │   ├── ipc.rs           # IPC message types
│   │   ├── job.rs           # Job, JobStatus
│   │   └── scheduler.rs     # Scheduler type definitions
│   └── utils.rs             # Shared helper functions
├── .gitignore
├── Cargo.lock
├── Cargo.toml
├── cliff.toml
├── dist-workspace.toml
├── install.sh
├── LICENSE
├── Makefile
├── README_zh.md
├── README.md
├── USAGE_zh.md
└── USAGE.md
```

## Prerequisites

* `Linux` OS
* NVIDIA GPU and driver
* `Cargo` (only for building from source)
* `less` (only for viewing logs)
* `make` (only for using make install)

## Installation

### Install a Prebuilt Release (Recommended)

> Users in China can use the Gitee mirror: `curl --proto '=https' --tlsv1.2 -LsSf https://raw.giteeusercontent.com/ying_luan/cgm/raw/master/install.sh | CGM_MIRROR=gitee bash`

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://raw.githubusercontent.com/Ying-Luan/cgm/master/install.sh | bash
```

The script installs to the system directory `/usr/local/bin/cgm` by default. Set `CGM_INSTALL_MODE=user` before `bash` to install to `~/.local/bin/cgm`.

### Build from Source

> Users in China may use the Gitee mirror: `git clone https://gitee.com/ying_luan/cgm.git`

Method 1: Use `make`

```bash
git clone https://github.com/Ying-Luan/cgm.git
cd cgm
make install MODE=user
# Or: make install MODE=system
```

Method 2: Manual build

```bash
git clone https://github.com/Ying-Luan/cgm.git
cd cgm
cargo build --release
sudo cp target/release/cgm /usr/local/bin/
sudo chmod +x /usr/local/bin/cgm
```

## Usage

Full command documentation see [USAGE.md](USAGE.md)

Quick Reference for Common Commands:

```bash
cgm start --user
cgm config set submit.detach true        # Set a persistent default
cgm submit -g 1 -- python main.py        # Submit job
cgm rerun 1 -c e,d                       # Rerun with current environment and directory
cgm list                                 # List jobs
cgm log 1                                # View log
cgm cancel 1                             # Cancel job
```

### Daemon Logs

```bash
cat ~/.local/state/cgm/cgm.out   # Personal daemon
sudo cat /var/lib/cgm/cgm.err    # System daemon
```
