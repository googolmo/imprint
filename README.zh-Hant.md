<p align="center">
  <img src="assets/icon/AppIcon-preview.png" width="128" alt="Imprint">
</p>

# Imprint

[English](README.md) · [简体中文](README.zh-Hans.md) · [繁體中文](README.zh-Hant.md) · [日本語](README.ja.md)

將作業系統映像檔寫入 USB 與 SD 卡。用途與 [balenaEtcher](https://github.com/balena-io/etcher) 相同：選擇映像檔、選擇可移除磁碟、寫入、驗證。

原生桌面介面以 **[GPUI](https://gpui.rs)**（Zed 的 GPU UI）打造，並附命令列工具。支援 **macOS、Linux 與 Windows**。

**寫入會清除目標磁碟上的所有資料。** Imprint 預設隱藏內建 / 系統磁碟。命令列在沒有 `--yes` 時不會寫入。

[使用說明](docs/HOW_TO_USE.zh-Hant.md) · [發行版本](https://github.com/googolmo/imprint/releases)

## 功能

- 寫入 `.iso`、`.img`、`.dmg` 及其壓縮格式（`.gz`、`.bz2`、`.xz`、`.zst`、`.zip`）
- 樹莓派模式：下載官方映像檔，並設定主機名稱、使用者、Wi-Fi 與 SSH
- 自動偵測可移除 USB / SD 目標；**系統磁碟保持隱藏**
- 寫入，並可選擇逐位元組驗證
- 將映像檔拖放到視窗即可開啟
- 用同一映像檔再寫一塊磁碟，或另選新映像檔
- 套件版支援應用程式內更新
- 介面語言：English、简体中文、繁體中文、日本語、한국어、Deutsch、Español、Français、Português
- `imprint-cli`，適合腳本與救援環境

## 使用方法

完整步驟見 **[docs/HOW_TO_USE.zh-Hant.md](docs/HOW_TO_USE.zh-Hant.md)**。

### 桌面應用程式

1. 開啟 **Imprint**。
2. **映像檔** — 點選 **選擇**，使用 **檔案 → 打開映像檔…**，或將 `.iso` / `.img` / `.dmg`（或壓縮檔）拖到視窗上。
3. **目標** — 插入 USB 隨身碟或 SD 卡，點選 **選擇**，選取可移除磁碟。內建磁碟不會出現在清單中。
4. 點選 **寫入**。在管理員提示中授權（Touch ID / 密碼、polkit 或 UAC）。
5. 等待卸載 → 寫入 → 可選驗證。完成後即可用該磁碟開機。

**樹莓派：** 點選底部的樹莓派列（或 **檔案 → Raspberry Pi…**）。選擇機型、官方映像檔或本機檔案、首次開機選項（主機名稱、使用者、Wi-Fi、SSH），再選擇 SD 卡。

設定（齒輪按鈕，或 `⌘,` / `Ctrl+,`）：外觀、語言、驗證寫入、完成後退出、隱藏系統磁碟。

### 命令列

```bash
# 列出可移除磁碟
imprint-cli devices

# 寫入 — 必須加上 --yes；會提示管理員 / root 權限
imprint-cli flash ubuntu.iso --device /dev/rdisk4 --yes
```

裝置路徑：macOS 為 `/dev/rdiskN`，Linux 為 `/dev/sdX` 或 `/dev/nvmeXn1`，Windows 為 `\\.\PhysicalDriveN`。

## 快速開始（從原始碼）

```bash
# 列出可移除磁碟
cargo run -p imprint-cli -- devices

# 寫入 — 會提示管理員 / root 權限
cargo run -p imprint-cli -- flash ubuntu.iso --device /dev/rdisk4 --yes

# 圖形介面 — macOS 需要完整 Xcode（Metal 著色器編譯器），僅有命令列工具不夠
cargo run -p imprint-app
```

Linux 圖形介面編譯需要 Fontconfig、FreeType、Wayland/X11 與 Vulkan 的 **開發套件**（`*.pc` 檔）。只安裝執行階段函式庫不夠：

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

套件版本（`.dmg`、`.msi`、`.deb`、AppImage）附於 [GitHub Releases](https://github.com/googolmo/imprint/releases)。

## 權限

開啟原始磁碟需要提升權限。Imprint 在寫入時才會請求：

- macOS：系統授權對話框（在允許時可用 Touch ID / Apple Watch，否則輸入密碼）
- Linux：polkit（`pkexec`）或 `sudo`
- Windows：UAC 提示

也可以用 `sudo` /「以系統管理員身分執行」啟動。圖形介面不必以 root 啟動。

## 工作區

| Crate | 用途 |
|-------|------|
| `imprint-core` | 共用型別（`ImageRef`、`TargetDisk`、`FlashProgress`、錯誤） |
| `imprint-image` | 辨識 / 解壓縮 / 串流讀取映像檔 |
| `imprint-device` | 列舉磁碟、隱藏系統磁碟、卸載 / 退出 |
| `imprint-rpi` | 樹莓派目錄、下載、首次開機設定 |
| `imprint-flash` | 區塊寫入與驗證 |
| `imprint-ui` | GPUI 介面與主題 |
| `imprint-app` | 桌面程式（`imprint`） |
| `imprint-cli` | 命令列程式（`imprint-cli`） |

相依套件 **版本** 寫在根目錄 `Cargo.toml` 的 `[workspace.dependencies]`。**功能旗標**（GPUI 平台後端、clap derive 等）寫在各成員 crate 中。

GPUI 來自 **Zed 官方樹**（`github.com/zed-industries/zed`），在根目錄 `Cargo.toml` 以 git 相依引入。請勿使用 `gpui-unofficial`。若要改介面，請見 `AGENTS.md`。

## 授權

Imprint 原始碼為 **Apache-2.0**。詳見 [LICENSE](LICENSE) 與 [NOTICE](NOTICE)。

- **命令列**（`imprint-cli`）：Apache-2.0。不連結 GPUI。
- **桌面程式**（`imprint`）：來自 Zed 官方 git 樹的 GPUI 目前會帶入 `ztracing` / `zlog` / `ztracing_macro`（**GPL-3.0-or-later**）。在 [zed#55470](https://github.com/zed-industries/zed/issues/55470) 修復之前，散佈該二進位檔屬於組合作品，還須遵守 GPL-3.0。
