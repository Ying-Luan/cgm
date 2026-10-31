<div align="center">

# Convenient GPU Manager

一个轻量级的 GPU 任务调度器

[![Crates.io Version](https://img.shields.io/crates/v/cgm)](https://crates.io/crates/cgm)
[![Crates.io Size](https://img.shields.io/crates/size/cgm)](https://crates.io/crates/cgm)
[![GitHub License](https://img.shields.io/github/license/Ying-Luan/cgm)](LICENSE)
[![Codacy Badge](https://app.codacy.com/project/badge/Grade/8853a13a90ac4417b70c544c297655ac)](https://app.codacy.com/gh/Ying-Luan/cgm/dashboard?utm_source=gh&utm_medium=referral&utm_content=&utm_campaign=Badge_grade)

[English](README.md) | [中文](README_zh.md)

[Usage Docs](USAGE.md) | [使用文档](USAGE_zh.md)

[GitHub](https://github.com/Ying-Luan/cgm) | [Gitee](https://gitee.com/ying_luan/cgm)

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
├── .github/
│   └── workflows/
│       └── release.yml
├── src/
│   ├── cli.rs               # CLI 模块入口，定义命令结构
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
│   │   └── utils.rs         # CLI 公共工具
│   ├── client.rs            # 客户端通信
│   ├── config.rs            # 配置模块入口
│   ├── config/
│   │   ├── load.rs          # 配置读取与合并
│   │   ├── manage.rs        # 配置管理
│   │   ├── path.rs          # 配置路径解析
│   │   └── validate.rs      # 配置校验
│   ├── constants.rs         # 全局常量
│   ├── daemon.rs            # 守护进程入口
│   ├── daemon/
│   │   ├── control.rs       # 取消/停止事件与发送
│   │   ├── executor.rs      # 任务执行与 mpsc 生命周期控制
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
│   ├── runtime.rs           # 守护进程模式、身份与运行元数据
│   ├── runtime/
│   │   ├── identity.rs      # Socket 对端凭据与权限校验
│   │   ├── paths.rs         # 状态路径与权限
│   │   └── socket.rs        # 机器级 abstract socket
│   ├── types.rs             # 类型模块入口
│   ├── types/
│   │   ├── config.rs        # 配置类型、作用域与来源
│   │   ├── daemon.rs        # 守护进程模式、状态与元数据
│   │   ├── gpu.rs           # GpuInfo、GpuState
│   │   ├── ipc.rs           # IPC 消息类型
│   │   ├── job.rs           # Job、JobStatus
│   │   └── scheduler.rs     # 调度器类型定义
│   └── utils.rs             # 共享工具函数
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

## 前置要求

* `Linux` 操作系统
* NVIDIA GPU 和驱动
* `Cargo`（从源码构建或使用 `cargo install` 时需要）
* `less`（仅查看日志时需要）
* `make`（仅使用 make 安装时需要）

## 安装

### 安装预编译版本（推荐）

> 国内用户可使用 Gitee 镜像：`curl --proto '=https' --tlsv1.2 -LsSf https://raw.giteeusercontent.com/ying_luan/cgm/raw/master/install.sh | CGM_MIRROR=gitee bash`

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://raw.githubusercontent.com/Ying-Luan/cgm/master/install.sh | bash
```

脚本默认系统安装到 `/usr/local/bin/cgm`。在 `bash` 前设置 `CGM_INSTALL_MODE=user` 可安装到 `~/.local/bin/cgm`。

### 使用 cargo 安装

```bash
cargo install cgm
```

### 从源码构建

> 国内用户可使用 Gitee 镜像：`git clone https://gitee.com/ying_luan/cgm.git`

方式一：使用 `make`

```bash
git clone https://github.com/Ying-Luan/cgm.git
cd cgm
make install MODE=user
# 或 make install MODE=system
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
cgm start --user
cgm config set submit.detach true        # 设置持久默认值
cgm submit -g 1 -- python main.py        # 提交任务
cgm rerun 1 -c e,d                       # 使用当前环境变量和工作目录重新运行任务
cgm list                                 # 列出任务
cgm log 1                                # 查看日志
cgm cancel 1                             # 取消任务
```

### 守护进程日志

```bash
cat ~/.local/state/cgm/cgm.out   # 个人守护进程
sudo cat /var/lib/cgm/cgm.err    # 系统守护进程
```
