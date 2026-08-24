#!/usr/bin/env bash
# Shallow-clone official Zed outside this workspace so GPUI can inherit
# Zed's [workspace.dependencies]. Nested workspaces do not work.
set -euo pipefail
REV="${ZED_REV:-4c4b19a2cf90a613cd377b2aebc3de6438a7da9f}"
DEST="${IMPRINT_ZED_SRC:-${HOME}/.cache/imprint/zed}"
mkdir -p "$(dirname "$DEST")"
if [[ ! -d "$DEST/.git" ]]; then
  git clone --depth 1 --filter=blob:none https://github.com/zed-industries/zed.git "$DEST"
fi
git -C "$DEST" fetch --depth 1 origin "$REV"
rm -f "$DEST/.git/index.lock"
git -C "$DEST" checkout --force --detach "$REV"
echo "zed $REV ready at $DEST"
echo "Cargo.toml gpui path should be: $DEST/crates/gpui"
