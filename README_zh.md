<div align="center">

# Convenient GPU Manager

一个轻量级的 GPU 任务调度器

[![GitHub License](https://img.shields.io/github/license/Ying-Luan/cgm)](LICENSE)
[![Codacy Badge](https://app.codacy.com/project/badge/Grade/8853a13a90ac4417b70c544c297655ac)](https://app.codacy.com/gh/Ying-Luan/cgm/dashboard?utm_source=gh&utm_medium=referral&utm_content=&utm_campaign=Badge_grade)

[English](README.md) | [中文](README_zh.md)

</div>

---

## 特性

* 多用户共享 GPU 资源，自动排队调度
* 自动分配空闲 GPU 并设置 `CUDA_VISIBLE_DEVICES`
* 提交时快照环境变量，兼容 conda、venv 等虚拟环境
* 支持分离模式，提交后关闭终端不影响执行
* 完整的任务生命周期管理（提交、取消、删除、查看日志）
* 自动检测外部 GPU 占用，避免资源冲突

## 项目结构

```text
cgm/
├── src/
│   ├── cli.rs               # CLI 模块入口，定义命令结构
│   ├── cli/
│   │   ├── cancel.rs        # cgm cancel
│   │   ├── delete.rs        # cgm delete
│   │   ├── list.rs          # cgm list
│   │   ├── log.rs           # cgm log
│   │   ├── start.rs         # cgm start
│   │   ├── status.rs        # cgm status
│   │   ├── stop.rs          # cgm stop
│   │   └── submit.rs        # cgm submit
│   ├── client.rs            # 客户端通信
│   ├── constants.rs         # 全局常量
│   ├── daemon.rs            # 守护进程入口
│   ├── daemon/
│   │   ├── process.rs       # 守护进程启停
│   │   ├── scheduler.rs     # 调度器 trait 与公共函数
│   │   ├── scheduler/
│   │   │   ├── fifo.rs      # FIFO 调度器
│   │   │   └── greedy.rs    # Greedy 调度器
│   │   └── server.rs        # Socket 请求处理
│   ├── db.rs                # 数据库入口
│   ├── db/
│   │   ├── operations.rs    # SQLite CRUD
│   │   └── schema.rs        # 数据库 Schema
│   ├── hardware.rs          # 硬件模块入口
│   ├── hardware/
│   │   └── gpu.rs           # GPU 状态管理
│   ├── macros.rs            # 宏定义
│   ├── main.rs              # 程序入口
│   ├── monitor.rs           # 状态展示（show_status、show_list）
│   ├── os.rs                # 用户信息与权限检查
│   ├── types.rs             # 类型模块入口
│   └── types/
│       ├── gpu.rs           # GpuInfo、GpuState
│       ├── ipc.rs           # IPC 消息类型
│       ├── job.rs           # Job、JobStatus
│       └── scheduler.rs     # 调度器类型定义
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

## 前置要求

* `Linux` 操作系统
* NVIDIA GPU 和驱动
* `Cargo`（仅从源码构建时需要）
* `less`（仅查看日志时需要）
* `make`（仅使用 make 安装时需要）

## 安装

### 从源码构建

方式一：使用 `make`

```bash
git clone https://github.com/Ying-Luan/cgm.git
cd cgm
make install
```

方式二：手动编译

```bash
git clone https://github.com/Ying-Luan/cgm.git
cd cgm
cargo build --release
sudo cp target/release/cgm /usr/local/bin/
sudo chmod +x /usr/local/bin/cgm
```

## 用法

完整命令文档见 [USAGE_zh.md](USAGE_zh.md)

常用命令速查：

```bash
sudo cgm start                           # 启动守护进程
cgm submit -g 1 -- python main.py        # 提交任务
cgm list                                 # 列出任务
cgm log 1                                # 查看日志
cgm cancel 1                             # 取消任务
```

### 守护进程日志

```bash
cat /tmp/cgm/cgm.out   # 标准输出日志
cat /tmp/cgm/cgm.err   # 错误日志
```
