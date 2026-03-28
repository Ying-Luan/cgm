<div align="center">

# Convenient GPU Manager

A lightweight GPU job scheduler

[![GitHub License](https://img.shields.io/github/license/Ying-Luan/cgm)](LICENSE)
[![Codacy Badge](https://app.codacy.com/project/badge/Grade/8853a13a90ac4417b70c544c297655ac)](https://app.codacy.com/gh/Ying-Luan/cgm/dashboard?utm_source=gh&utm_medium=referral&utm_content=&utm_campaign=Badge_grade)

[English](README.md) | [中文](README_zh.md)

</div>

---

## Features

* Multi-user GPU resource sharing with automatic job queuing and scheduling
* Automatic GPU allocation with `CUDA_VISIBLE_DEVICES` setup
* Environment snapshot at submission time, compatible with conda, venv, etc.
* Detached mode: jobs continue running after terminal closes
* Complete job lifecycle management (submit, cancel, delete, view logs)
* Automatic external GPU usage detection to avoid resource conflicts

## Project Structure

```text
cgm/
├── src/
│   ├── cli.rs               # CLI module entry, defines command structure
│   ├── cli/
│   │   ├── cancel.rs        # cgm cancel
│   │   ├── delete.rs        # cgm delete
│   │   ├── list.rs          # cgm list
│   │   ├── log.rs           # cgm log
│   │   ├── start.rs         # cgm start
│   │   ├── status.rs        # cgm status
│   │   ├── stop.rs          # cgm stop
│   │   └── submit.rs        # cgm submit
│   ├── client.rs            # Socket client for daemon communication
│   ├── constants.rs         # Global constants
│   ├── daemon.rs            # Daemon module entry
│   ├── daemon/
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
│   ├── macros.rs            # macros definitions
│   ├── main.rs              # Program entry point
│   ├── monitor.rs           # Status display (show_status, show_list)
│   ├── os.rs                # User info and permission checks
│   ├── types.rs             # Types module entry
│   └── types/
│       ├── gpu.rs           # GpuInfo, GpuState
│       ├── ipc.rs           # IPC message types
│       ├── job.rs           # Job, JobStatus
│       └── scheduler.rs     # Scheduler type definitions
├── .gitignore
├── Cargo.lock
├── Cargo.toml
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

### Build from Source

Method 1: Use `make`

```bash
git clone https://github.com/Ying-Luan/cgm.git
cd cgm
make install
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
sudo cgm start                           # Start daemon
cgm submit -g 1 -- python main.py        # Submit job
cgm list                                 # List jobs
cgm log 1                                # View log
cgm cancel 1                             # Cancel job
```

### Daemon Logs

```bash
cat /tmp/cgm/cgm.out   # Standard output log
cat /tmp/cgm/cgm.err   # Error log
```
