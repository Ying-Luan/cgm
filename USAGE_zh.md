<div align="center">

# CGM 命令参考

[English](USAGE.md) | [中文](USAGE_zh.md)

</div>

---

## 目录

* [启动守护进程](#启动守护进程)
* [关闭守护进程](#关闭守护进程)
* [提交任务](#提交任务)
  * [关于环境变量引用](#关于环境变量引用)
* [取消任务](#取消任务)
* [删除任务](#删除任务)
* [查看状态](#查看状态)
* [查看任务列表](#查看任务列表)
* [查看任务日志](#查看任务日志)

## 启动守护进程

```bash
sudo cgm start [options]
```

| 参数名称 | 简写 | 默认值 | 说明 |
| ------- | ---- | ----- | ---- |
| `--force` | `-f` | `false` | 重新建立数据库并强制启动。用于数据库文件损坏的情况 |
| `--gpus <GPU_LIST>` | `-g <GPU_LIST>` | `all` | 指定 CGM 管理的 GPU 列表，逗号分隔，如 "0,1,2,3" 或 "all" 表示全部 GPU |
| `--interval <N>` | `-i <N>` | `10` | 设置调度间隔（秒） |
| `--scheduler <NAME>` | `-s <NAME>` | `greedy` | 指定调度策略。可选值：greedy、fifo |
| `--threshold <N>` | `-t <N>` | `10` | 设置 GPU 内存使用率阈值(%)。超过此值认为被外部占用 |

## 关闭守护进程

```bash
sudo cgm stop [options]
```

| 参数名称 | 简写 | 默认值 | 说明 |
| ------- | ---- | ----- | ---- |
| `--force` | `-f` | `false` | 强制停止。即使有任务正在运行也立即关闭 |

## 提交任务

```bash
cgm submit [options] -- <command>
```

| 参数名称 | 简写 | 默认值 | 说明 |
| ------- | ---- | ----- | ---- |
| `--detach` | `-d` | `false` | 开启分离模式。任务提交后立即返回，不阻塞终端 |
| `--gpus <N>` | `-g <N>` | `1` | 指定该任务需要申请的显卡数量 |
| `--log <PATH>` | `-l <PATH>` | ` ` | 指定日志文件路径 |

### 示例

```bash
cgm submit -g 1 -- python main.py
```

### 关于环境变量引用

如果希望任务运行时才展开环境变量（例如 `CUDA_VISIBLE_DEVICES`），需要用单引号包裹，阻止当前 shell 提前展开。

错误示例（会被当前 shell 展开）：

```bash
cgm submit -- echo $CUDA_VISIBLE_DEVICES
```

正确示例：

```bash
cgm submit -- echo '$CUDA_VISIBLE_DEVICES'
```

单引号内的内容不会被 shell 解析，`$CUDA_VISIBLE_DEVICES` 会作为字面字符串传给 `cgm`，由任务执行时的 shell 展开。

## 取消任务

```bash
cgm cancel <JOB_ID> [options]
```

| 参数名称 | 简写 | 默认值 | 说明 |
| ------- | ---- | ----- | ---- |
| `--force` | `-f` | `false` | 强制停止。即使任务正在运行也停止 |

## 删除任务

```bash
sudo cgm delete [JOB_ID] [options]
```

| 参数名称 | 简写 | 默认值 | 说明 |
| ------- | ---- | ----- | ---- |
| `--all` | `-a` | `false` | 删除所有已终止的任务（completed / failed / cancelled） |
| `--status <STATUS>` | `-s <STATUS>` | ` ` | 按状态删除，逗号分隔。可选值：`completed`、`failed`、`cancelled` |

`JOB_ID` 与 `--all` / `--status` 互斥，三者只能指定其一。

## 查看状态

```bash
cgm status [options]
```

| 参数名称 | 简写 | 默认值 | 说明 |
| ------- | ---- | ----- | ---- |

## 查看任务列表

```bash
cgm list [options]
```

| 参数名称 | 简写 | 默认值 | 说明 |
| ------- | ---- | ----- | ---- |
| `--all` | `-a` | `false` | 显示全部任务 |
| `--limit <N>` | `-l <N>` | `20` | 显示最新的 N 条任务 |

`--all` 与 `--limit` 互斥，只能指定其一。

## 查看任务日志

```bash
cgm log <JOB_ID> [options]
```

| 参数名称 | 简写 | 默认值 | 说明 |
| ------- | ---- | ----- | ---- |
