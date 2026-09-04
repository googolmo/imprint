<p align="center">
  <img src="assets/icon/AppIcon-preview.png" width="128" alt="Imprint">
</p>

# Imprint

[English](README.md) · [简体中文](README.zh-Hans.md) · [繁體中文](README.zh-Hant.md) · [日本語](README.ja.md)

OS イメージを USB ドライブと SD カードに書き込みます。[balenaEtcher](https://github.com/balena-io/etcher) と同じ用途です。イメージを選び、取り外し可能なディスクを選び、書き込み、検証します。

ネイティブなデスクトップ UI は **[GPUI](https://gpui.rs)**（Zed の GPU UI）で、CLI も同梱します。**macOS、Linux、Windows** で動作します。

**書き込むと対象ディスクの内容はすべて消去されます。** Imprint は内蔵 / システムドライブを非表示にします。CLI は `--yes` なしでは書き込みません。

[使い方](docs/HOW_TO_USE.ja.md) · [リリース](https://github.com/googolmo/imprint/releases)

## 機能

- `.iso`、`.img`、`.dmg`、および圧縮形式（`.gz`、`.bz2`、`.xz`、`.zst`、`.zip`）の書き込み
- Raspberry Pi モード：公式イメージをダウンロードし、ホスト名、ユーザー、Wi-Fi、SSH を設定
- 取り外し可能な USB / SD を自動検出。**システムディスクは表示しません**
- 書き込みと、任意のバイト単位検証
- イメージをウィンドウへドラッグ＆ドロップ
- 同じイメージを別のディスクへ再書き込み、または新しいイメージを選択
- パッケージ版ではアプリ内アップデート
- UI 言語：English、简体中文、繁體中文、日本語、한국어、Deutsch、Español、Français、Português
- スクリプトや復旧シェル向けの `imprint-cli`

## 使い方

手順の詳細は **[docs/HOW_TO_USE.ja.md](docs/HOW_TO_USE.ja.md)** を参照してください。

### デスクトップアプリ

1. **Imprint** を起動します。
2. **イメージ** — **選択** をクリックするか、**ファイル → イメージを開く…** を使うか、`.iso` / `.img` / `.dmg`（または圧縮アーカイブ）をウィンドウへドロップします。
3. **ターゲット** — USB メモリまたは SD カードを接続し、**選択** をクリックして取り外し可能なディスクを選びます。内蔵ディスクは一覧に出ません。
4. **書き込む** をクリックします。管理者プロンプト（Touch ID / パスワード、polkit、または UAC）を承認します。
5. アンマウント → 書き込み → 任意の検証が終わるまで待ちます。完了後、そのドライブから起動できます。

**Raspberry Pi：** 下部の Raspberry Pi バー（または **ファイル → Raspberry Pi…**）をクリックします。機種、公式イメージまたはローカルファイル、初回起動オプション（ホスト名、ユーザー、Wi-Fi、SSH）を選び、SD カードを指定します。

設定（歯車、または `⌘,` / `Ctrl+,`）：外観、言語、書き込みの検証、成功後に取り出す、システムドライブを隠す。

### コマンドライン

```bash
# 取り外し可能なディスクを一覧表示
imprint-cli devices

# 書き込み — --yes が必須。管理者 / root 権限の確認が出ます
imprint-cli flash ubuntu.iso --device /dev/rdisk4 --yes
```

デバイスパス：macOS は `/dev/rdiskN`、Linux は `/dev/sdX` または `/dev/nvmeXn1`、Windows は `\\.\PhysicalDriveN`。

## インストール

### macOS（Homebrew）

[googolmo/homebrew-tap](https://github.com/googolmo/homebrew-tap) の cask からインストールします。Homebrew は `.dmg` をこのプロジェクトの [GitHub Releases](https://github.com/googolmo/imprint/releases) から取得します。

```bash
brew install --cask googolmo/tap/imprint
```

**Imprint** は `/Applications` に入り、`imprint-cli` が `PATH` に追加されます。この cask は `homebrew/cask` にはありません。

[GitHub Releases](https://github.com/googolmo/imprint/releases) から `.dmg` を入手することもできます。

### Linux（Debian / Ubuntu）

[googolmo/repo](https://github.com/googolmo/repo) の APT ソースを入れます。`apt` は `.deb` をこのプロジェクトの [GitHub Releases](https://github.com/googolmo/imprint/releases) から取得します。

```bash
sudo mkdir -p /usr/share/keyrings
sudo curl -fsSL https://repo-cr4.pages.dev/keys/repo.gpg \
  -o /usr/share/keyrings/repo-archive-keyring.gpg
sudo chmod 644 /usr/share/keyrings/repo-archive-keyring.gpg
sudo curl -fsSL https://repo-cr4.pages.dev/ubuntu/mosumi-repo.sources \
  -o /etc/apt/sources.list.d/mosumi-repo.sources
sudo apt update
sudo apt install imprint
```

`mosumi-repo.sources` のスイートは `noble`（Ubuntu 24.04 / Debian 13+）です。Ubuntu 26.04（amd64 と arm64）は suite を `resolute` にします。

### Linux（Arch）

[googolmo/repo](https://github.com/googolmo/repo) の Pacman ソースファイルを入れます。`pacman` は `.pkg.tar.zst` をこのプロジェクトの [GitHub Releases](https://github.com/googolmo/imprint/releases) から取得します。

```bash
curl -fsSL https://repo-cr4.pages.dev/keys/repo.asc | sudo pacman-key --add -
sudo pacman-key --lsign-key 9DF42B7054F1CB5B
sudo curl -fsSL https://repo-cr4.pages.dev/pacman/mosumi-repo.conf \
  -o /etc/pacman.d/mosumi-repo.conf
echo -e '\nInclude = /etc/pacman.d/mosumi-repo.conf' | sudo tee -a /etc/pacman.conf
sudo pacman -Sy imprint
```

### Linux と Windows

パッケージ版は [GitHub Releases](https://github.com/googolmo/imprint/releases) にあります。Windows amd64（`.msi`）と arm64（NSIS）、Linux amd64 と arm64（Ubuntu 24.04/Debian 13+ と Ubuntu 26.04 向け `.deb`、AppImage、Arch の `.pkg.tar.zst`）。

## クイックスタート（ソースから）

```bash
# 取り外し可能なディスクを一覧表示
cargo run -p imprint-cli -- devices

# 書き込み — 管理者 / root 権限の確認が出ます
cargo run -p imprint-cli -- flash ubuntu.iso --device /dev/rdisk4 --yes

# GUI — macOS はフル Xcode（Metal シェーダーコンパイラ）が必要。CLT だけではビルドできません
cargo run -p imprint-app
```

Linux の GUI ビルドには Fontconfig、FreeType、Wayland/X11、Vulkan の **開発パッケージ**（`*.pc` ファイル）が必要です。実行時ライブラリだけでは足りません：

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

## 権限

生のディスクを開くには昇格した権限が必要です。Imprint は書き込み時に要求します。

- macOS：システムの承認ダイアログ（可能な場合は Touch ID / Apple Watch、それ以外はパスワード）
- Linux：polkit（`pkexec`）または `sudo`
- Windows：UAC プロンプト

`sudo` / 「管理者として実行」で起動しても構いません。GUI を root で始める必要はありません。

## ワークスペース

| Crate | 役割 |
|-------|------|
| `imprint-core` | 共有型（`ImageRef`、`TargetDisk`、`FlashProgress`、エラー） |
| `imprint-image` | イメージの判別 / 展開 / ストリーム読み取り |
| `imprint-device` | ディスク列挙、システムドライブの非表示、アンマウント / 取り出し |
| `imprint-rpi` | Raspberry Pi カタログ、ダウンロード、初回起動設定 |
| `imprint-flash` | ブロック書き込みと検証 |
| `imprint-ui` | GPUI ビューとテーマ |
| `imprint-app` | デスクトップバイナリ（`imprint`） |
| `imprint-cli` | CLI バイナリ（`imprint-cli`） |

依存関係の **バージョン** はルート `Cargo.toml` の `[workspace.dependencies]` にあります。**フィーチャー**（GPUI プラットフォームバックエンド、clap derive など）は各メンバー crate にあります。

GPUI は **Zed 公式ツリー**（`github.com/zed-industries/zed`）をルート `Cargo.toml` の git 依存として使います。`gpui-unofficial` は使わないでください。UI を変更する場合は `AGENTS.md` を参照してください。

## ライセンス

Imprint のソースコードは **Apache-2.0** です。[LICENSE](LICENSE) と [NOTICE](NOTICE) を参照してください。

- **CLI**（`imprint-cli`）：Apache-2.0。GPUI にはリンクしません。
- **デスクトップアプリ**（`imprint`）：Zed 公式 git ツリーの GPUI は現在 `ztracing` / `zlog` / `ztracing_macro`（**GPL-3.0-or-later**）を引き込みます。[zed#55470](https://github.com/zed-industries/zed/issues/55470) が直るまで、そのバイナリの配布は結合著作物として GPL-3.0 も遵守する必要があります。
