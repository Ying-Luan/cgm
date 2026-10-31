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
* [Rerun Job](#rerun-job)
* [Cancel Job](#cancel-job)
* [Delete Job](#delete-job)
* [View Status](#view-status)
* [View Job List](#view-job-list)
* [View Job Info](#view-job-info)
* [View Job Log](#view-job-log)
* [Configuration](#configuration)
  * [Initialize Configuration](#initialize-configuration)
  * [Set Configuration Value](#set-configuration-value)
  * [Remove Configuration Value](#remove-configuration-value)
  * [Get Configuration Value](#get-configuration-value)
  * [Show Configuration](#show-configuration)
  * [Validate Configuration](#validate-configuration)
  * [Show Configuration Path](#show-configuration-path)
  * [Precedence](#precedence)

## Start Daemon

```bash
cgm start [OPTIONS]
```

| Option | Short | Default | Description | Config Key |
| ------ | ----- | ------- | ----------- | --- |
| `--system` | | `false` | Start the system daemon as root | |
| `--user` | | `false` | Start a personal daemon as a non-root user | |
| `--force` | `-f` | `false` | Force start. Recreates the database. Used when database file is corrupted | ` ` |
| `--gpus <GPU_LIST>` | `-g <GPU_LIST>` | `all` | GPUs to manage, comma-separated (e.g., "0,1,2,3") or "all" for all GPUs | `start.gpus` |
| `--interval <N>` | `-i <N>` | `10` | Scheduling interval in seconds | `start.interval` |
| `--scheduler <NAME>` | `-s <NAME>` | `greedy` | Scheduler strategy. Options: greedy, fifo | `start.scheduler` |
| `--threshold <N>` | `-t <N>` | `10` | GPU memory usage threshold (%). GPUs exceeding this are considered externally occupied | `start.threshold` |

## Stop Daemon

```bash
cgm stop [OPTIONS]
```

| Option | Short | Default | Description | Config Key |
| ------ | ----- | ------- | ----------- | --- |
| `--force` | `-f` | `false` | Terminate registered active jobs and wait for finalization | ` ` |

Only the daemon owner or root may stop the daemon.

## Submit Job

```bash
cgm submit [OPTIONS] -- <COMMAND>
```

| Option | Short | Default | Description | Config Key |
| ------ | ----- | ------- | ----------- | --- |
| `--allow-gpus <ID,ID,...>` | `-G <ID,ID,...>` | ` ` | Restrict GPU IDs this job may use, comma-separated | `submit.allow_gpus` |
| `--depends-on <ID,ID,...>` | `-D` | ` ` | Comma-separated prerequisite jobs that must complete successfully | ` ` |
| `--detach` | `-d` | `false` | Detach mode. Returns immediately without blocking the terminal | `submit.detach` |
| `--follow` | ` ` | `true` | Open `less` after submission and follow the log, blocks the terminal | `submit.detach` |
| `--log <PATH>` | `-l <PATH>` | ` ` | Log file path | ` ` |
| `--require-gpus <N>` | `-g <N>` | `1` | Number of GPUs to request for this job | `submit.required_gpus` |

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

## Rerun Job

```bash
cgm rerun <JOB_ID> [OPTIONS]
```

| Option | Short | Default | Description | Config Key |
| ------ | ----- | ------- | ----------- | --- |
| `--allow-gpus <ID,ID,...>` | `-G <ID,ID,...>` | ` ` | Override GPU IDs this job may use, comma-separated | ` ` |
| `--current <FIELDS>` | `-c <FIELDS>` | ` ` | Use current environment variables (`env`/`e`) and/or working directory (`dir`/`d`), comma-separated | ` ` |
| `--depends-on <ID,ID,...>` | `-D` | ` ` | Specify prerequisite jobs for the new job, comma-separated | ` ` |
| `--detach` | `-d` | `false` | Enable detach mode. Do not open the log viewer after submission | `rerun.detach` |
| `--follow` | ` ` | `true` | Open `less` to follow the new job log, blocks the terminal | `rerun.detach` |
| `--log <PATH>` | `-l <PATH>` | ` ` | Set the new job's log path | ` ` |
| `--require-gpus <N>` | `-g <N>` | ` ` | Override the number of GPUs for the new job | ` ` |

A rerun keeps the source job's allowed GPU IDs but does not inherit its dependencies.

## Cancel Job

```bash
cgm cancel <JOB_ID> [OPTIONS]
```

| Option | Short | Default | Description | Config Key |
| ------ | ----- | ------- | ----------- | --- |
| `--force` | `-f` | `false` | Force cancel. Terminates even if job is running | ` ` |

## Delete Job

```bash
cgm delete [JOB_ID] [OPTIONS]
```

| Option | Short | Default | Description | Config Key |
| ------ | ----- | ------- | ----------- | --- |
| `--all` | `-a` | `false` | Delete all terminated jobs (completed / failed / cancelled) | ` ` |
| `--status <STATUS>` | `-s <STATUS>` | ` ` | Delete by status, comma-separated. Values: `completed`, `failed`, `cancelled` | ` ` |

`JOB_ID` and `--all` / `--status` are mutually exclusive. Only one can be specified.

## View Status

```bash
cgm status [OPTIONS]
```

| Option | Short | Default | Description | Config Key |
| ------ | ----- | ------- | ----------- | --- |
| `--quiet` | | `false` | Exit code 0 when cgm is running, 1 when not running, 2 on check error | |

## View Job List

```bash
cgm list [OPTIONS]
```

| Option | Short | Default | Description | Config Key |
| ------ | ----- | ------- | ----------- | --- |
| `--all` | `-a` | `false` | Show all jobs | ` ` |
| `--limit <N>` | ` ` | `20` | Show the latest N jobs | `list.limit` |
| `--long-format` | `-l` | `false` | Show complete field names and complete commands | ` ` |

`--all` and `--limit` are mutually exclusive; only one can be specified.

## View Job Info

```bash
cgm info <JOB_ID> [OPTIONS]
```

| Option | Short | Default | Description | Config Key |
| ------ | ----- | ------- | ----------- | --- |
| `--env` | `-e` | `false` | Show all saved environment variables and their values | ` ` |

## View Job Log

```bash
cgm log <JOB_ID> [OPTIONS]
```

| Option | Short | Default | Description | Config Key |
| ------ | ----- | ------- | ----------- | --- |

## Configuration

```bash
cgm config <COMMAND> [OPTIONS]
```

| Option | Short | Default | Description |
| ------ | ----- | ------- | ----------- |

### Initialize Configuration

```bash
cgm config init [OPTIONS]
```

| Option | Short | Default | Description |
| ------ | ----- | ------- | ----------- |
| `--force` | `-f` | `false` | Overwrite an existing configuration file |
| `--system` | ` ` | `false` | Target the system configuration (root only) |

### Set Configuration Value

```bash
cgm config set <KEY> <VALUE> [OPTIONS]
```

| Option | Short | Default | Description |
| ------ | ----- | ------- | ----------- |
| `--system` | ` ` | `false` | Target the system configuration (root only) |

`set` creates the minimal required configuration even without running `init` first.

### Remove Configuration Value

```bash
cgm config unset <KEY> [OPTIONS]
```

| Option | Short | Default | Description |
| ------ | ----- | ------- | ----------- |
| `--system` | ` ` | `false` | Target the system configuration (root only) |

### Get Configuration Value

```bash
cgm config get <KEY> [OPTIONS]
```

| Option | Short | Default | Description |
| ------ | ----- | ------- | ----------- |
| `--effective` | `-e` | `false` | Show the merged effective value |
| `--system` | ` ` | `false` | View the system configuration (root only) |
| `--source` | `-s` | `false` | Show the source of the value |

### Show Configuration

```bash
cgm config show [OPTIONS]
```

| Option | Short | Default | Description |
| ------ | ----- | ------- | ----------- |
| `--effective` | `-e` | `false` | Show the merged effective configuration |
| `--system` | ` ` | `false` | View the system configuration (root only) |
| `--source` | `-s` | `false` | Show the source of each value |

Displays all configuration values in a table.

### Validate Configuration

```bash
cgm config validate [OPTIONS]
```

| Option | Short | Default | Description |
| ------ | ----- | ------- | ----------- |
| `--system` | ` ` | `false` | Target the system configuration (root only) |

### Show Configuration Path

```bash
cgm config path [OPTIONS]
```

| Option | Short | Default | Description |
| ------ | ----- | ------- | ----------- |
| `--system` | ` ` | `false` | Target the system configuration (root only) |

### Precedence

Runtime precedence is: explicit command-line option > the selected configuration file > built-in default.
