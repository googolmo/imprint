<p align="center">
  <img src="assets/icon/AppIcon-preview.png" width="128" alt="Imprint">
</p>

# Imprint

[English](README.md) · [简体中文](README.zh-Hans.md) · [繁體中文](README.zh-Hant.md) · [日本語](README.ja.md)

将操作系统镜像写入 USB 和 SD 卡。用途与 [balenaEtcher](https://github.com/balena-io/etcher) 相同：选择镜像、选择可移动磁盘、写入、校验。

原生桌面界面基于 **[GPUI](https://gpui.rs)**（Zed 的 GPU UI），另附命令行工具。支持 **macOS、Linux 和 Windows**。

**写入会清除目标磁盘上的全部数据。** Imprint 默认隐藏内置 / 系统磁盘。命令行在没有 `--yes` 时不会写入。

[使用说明](docs/HOW_TO_USE.zh-Hans.md) · [发布版本](https://github.com/googolmo/imprint/releases)

## 功能

- 写入 `.iso`、`.img`、`.dmg` 及其压缩格式（`.gz`、`.bz2`、`.xz`、`.zst`、`.zip`）
- 树莓派模式：下载官方镜像，并设置主机名、用户、Wi-Fi 和 SSH
- 自动检测可移动 USB / SD 目标；**系统磁盘保持隐藏**
- 写入，并可选择逐字节校验
- 将镜像拖放到窗口即可打开
- 用同一镜像再写一块盘，或另选新镜像
- 打包版本支持应用内更新
- 界面语言：English、简体中文、繁體中文、日本語、한국어、Deutsch、Español、Français、Português
- `imprint-cli`，适合脚本和救援环境

## 使用方法

完整步骤见 **[docs/HOW_TO_USE.zh-Hans.md](docs/HOW_TO_USE.zh-Hans.md)**。

### 桌面应用

1. 打开 **Imprint**。
2. **镜像** — 点击 **选择**，使用 **文件 → 打开镜像…**，或将 `.iso` / `.img` / `.dmg`（或压缩包）拖到窗口上。
3. **目标** — 插入 U 盘或 SD 卡，点击 **选择**，选中可移动磁盘。内置磁盘不会出现在列表中。
4. 点击 **写入**。在管理员提示中授权（Touch ID / 密码、polkit 或 UAC）。
5. 等待卸载 → 写入 → 可选校验。完成后即可用该盘启动。

**树莓派：** 点击底部的树莓派栏（或 **文件 → Raspberry Pi…**）。选择机型、官方镜像或本地文件、首次启动选项（主机名、用户、Wi-Fi、SSH），再选择 SD 卡。

设置（齿轮按钮，或 `⌘,` / `Ctrl+,`）：外观、语言、校验写入、完成后弹出、隐藏系统磁盘。

### 命令行

```bash
# 列出可移动磁盘
imprint-cli devices

# 写入 — 必须加上 --yes；会提示管理员 / root 权限
imprint-cli flash ubuntu.iso --device /dev/rdisk4 --yes
```

设备路径：macOS 为 `/dev/rdiskN`，Linux 为 `/dev/sdX` 或 `/dev/nvmeXn1`，Windows 为 `\\.\PhysicalDriveN`。

## 安装

### macOS（Homebrew）

```bash
brew tap googolmo/imprint https://github.com/googolmo/imprint
brew install --cask imprint
```

会将 **Imprint** 装到 `/Applications`，并把 `imprint-cli` 放到 `PATH` 中。需要带上仓库 URL，因为 cask 在本仓库里，不在 `homebrew/cask`。

也可以从 [GitHub Releases](https://github.com/googolmo/imprint/releases) 下载 `.dmg`。

### Linux 和 Windows

打包版本在 [GitHub Releases](https://github.com/googolmo/imprint/releases) 中提供：Windows x86-64（`.msi`）与 arm64（NSIS）；Linux x86-64 与 arm64（Ubuntu 22.04/Debian 与 Ubuntu 24.04 的 `.deb`、AppImage、Arch `.pkg.tar.zst`、以及 AUR `PKGBUILD`）。

## 快速开始（从源码）

```bash
# 列出可移动磁盘
cargo run -p imprint-cli -- devices

# 写入 — 会提示管理员 / root 权限
cargo run -p imprint-cli -- flash ubuntu.iso --device /dev/rdisk4 --yes

# 图形界面 — macOS 需要完整 Xcode（Metal 着色器编译器），仅有命令行工具不够
cargo run -p imprint-app
```

Linux 图形界面编译需要 Fontconfig、FreeType、Wayland/X11 和 Vulkan 的 **开发包**（`*.pc` 文件）。只装运行时库不够：

```bash
sudo apt-get install -y --no-install-recommends \
  pkg-config \
  libfontconfig-dev \
  libfreetype6-dev \
  libxkbcommon-dev \
  libxkbcommon-x11-dev \
  libwayland-dev \
  libx11-dev \
  libx11-xcb-dev \
  libasound2-dev \
  libvulkan-dev \
  libgl1-mesa-dev
```

## 权限

打开原始磁盘需要提升权限。Imprint 在写入时才会请求：

- macOS：系统授权对话框（在允许时可用 Touch ID / Apple Watch，否则输入密码）
- Linux：polkit（`pkexec`）或 `sudo`
- Windows：UAC 提示

也可以用 `sudo` /「以管理员身份运行」启动。图形界面不必以 root 启动。

## 工作区

| Crate | 作用 |
|-------|------|
| `imprint-core` | 共享类型（`ImageRef`、`TargetDisk`、`FlashProgress`、错误） |
| `imprint-image` | 识别 / 解压 / 流式读取镜像 |
| `imprint-device` | 枚举磁盘、隐藏系统盘、卸载 / 弹出 |
| `imprint-rpi` | 树莓派目录、下载、首次启动配置 |
| `imprint-flash` | 块写入与校验 |
| `imprint-ui` | GPUI 界面与主题 |
| `imprint-app` | 桌面程序（`imprint`） |
| `imprint-cli` | 命令行程序（`imprint-cli`） |

依赖 **版本** 写在根目录 `Cargo.toml` 的 `[workspace.dependencies]`。**特性**（GPUI 平台后端、clap derive 等）写在各成员 crate 中。

GPUI 来自 **Zed 官方仓库**（`github.com/zed-industries/zed`），在根目录 `Cargo.toml` 中以 git 依赖引入。不要使用 `gpui-unofficial`。改界面请看 `AGENTS.md`。

## 许可证

Imprint 源码为 **Apache-2.0**。详见 [LICENSE](LICENSE) 与 [NOTICE](NOTICE)。

- **命令行**（`imprint-cli`）：Apache-2.0。不链接 GPUI。
- **桌面程序**（`imprint`）：来自 Zed 官方 git 树的 GPUI 目前会拉入 `ztracing` / `zlog` / `ztracing_macro`（**GPL-3.0-or-later**）。在 [zed#55470](https://github.com/zed-industries/zed/issues/55470) 修复之前，分发该二进制属于组合作品，还需遵守 GPL-3.0。
