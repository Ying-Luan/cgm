<div align="center">

# CGM Command Reference

[English](USAGE.md) | [中文](USAGE_zh.md)

</div>

---

## Table of Contents

* [Start Daemon](#start-daemon)
* [Stop Daemon](#stop-daemon)
* [Submit Job](#submit-job)
  * [Regarding Environment Variable Expansion](#regarding-environment-variable-expansion)
* [Cancel Job](#cancel-job)
* [Delete Job](#delete-job)
* [View Status](#view-status)
* [View Job List](#view-job-list)
* [View Job Log](#view-job-log)

## Start Daemon

```bash
sudo cgm start [options]
```

| Option | Short | Default | Description |
| ------ | ----- | ------- | ----------- |
| `--force` | `-f` | `false` | Force start. Recreates the database. Used when database file is corrupted |
| `--gpus <GPU_LIST>` | `-g <GPU_LIST>` | `all` | GPUs to manage, comma-separated (e.g., "0,1,2,3") or "all" for all GPUs |
| `--interval <N>` | `-i <N>` | `10` | Scheduling interval in seconds |
| `--scheduler <NAME>` | `-s <NAME>` | `greedy` | Scheduler strategy. Options: greedy, fifo |
| `--threshold <N>` | `-t <N>` | `10` | GPU memory usage threshold (%). GPUs exceeding this are considered externally occupied |

## Stop Daemon

```bash
sudo cgm stop [options]
```

| Option | Short | Default | Description |
| ------ | ----- | ------- | ----------- |
| `--force` | `-f` | `false` | Force stop. Shuts down immediately even if jobs are running |

## Submit Job

```bash
cgm submit [options] -- <command>
```

| Option | Short | Default | Description |
| ------ | ----- | ------- | ----------- |
| `--detach` | `-d` | `false` | Detach mode. Returns immediately without blocking the terminal |
| `--gpus <N>` | `-g <N>` | `1` | Number of GPUs to request for this job |
| `--log <PATH>` | `-l <PATH>` | ` ` | Log file path |

### Examples

```bash
cgm submit -g 1 -- python main.py
```

### Regarding Environment Variable Expansion

To defer environment variable expansion until job execution (e.g., `CUDA_VISIBLE_DEVICES`), wrap in single quotes to prevent the current shell from expanding prematurely.

Wrong (will be expanded by current shell):

```bash
cgm submit -- echo $CUDA_VISIBLE_DEVICES
```

Correct:

```bash
cgm submit -- echo '$CUDA_VISIBLE_DEVICES'
```

Single quotes prevent shell parsing, so `$CUDA_VISIBLE_DEVICES` is passed literally to `cgm` and expanded by the job's shell.

## Cancel Job

```bash
cgm cancel <JOB_ID> [options]
```

| Option | Short | Default | Description |
| ------ | ----- | ------- | ----------- |
| `--force` | `-f` | `false` | Force cancel. Terminates even if job is running |

## Delete Job

```bash
sudo cgm delete [JOB_ID] [options]
```

| Option | Short | Default | Description |
| ------ | ----- | ------- | ----------- |
| `--all` | `-a` | `false` | Delete all terminated jobs (completed / failed / cancelled) |
| `--status <STATUS>` | `-s <STATUS>` | ` ` | Delete by status, comma-separated. Values: `completed`, `failed`, `cancelled` |

`JOB_ID` and `--all` / `--status` are mutually exclusive. Only one can be specified.

## View Status

```bash
cgm status [options]
```

| Option | Short | Default | Description |
| ------ | ----- | ------- | ----------- |

## View Job List

```bash
cgm list [options]
```

| Option | Short | Default | Description |
| ------ | ----- | ------- | ----------- |
| `--all` | `-a` | `false` | Show all jobs without limit |
| `--limit <N>` | `-l <N>` | `20` | Show the latest N jobs |

`--all` and `--limit` are mutually exclusive. Only one can be specified.

## View Job Log

```bash
cgm log <JOB_ID> [options]
```

| Option | Short | Default | Description |
| ------ | ----- | ------- | ----------- |
