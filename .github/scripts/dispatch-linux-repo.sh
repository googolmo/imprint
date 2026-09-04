#!/usr/bin/env bash
# Dispatch googolmo/repo `update-index` for this release tag.
#
# Usage (GitHub Actions):
#   GH_TOKEN=$LINUX_REPO_TOKEN TAG=v0.1.4 VERSION=0.1.4 \
#     .github/scripts/dispatch-linux-repo.sh
#
# Env:
#   GH_TOKEN / LINUX_REPO_TOKEN   PAT with Actions: write on googolmo/repo
#   TAG                           git tag (v0.1.4)
#   VERSION                       Packager.toml version
#   GITHUB_REPOSITORY             owner/name of the imprint repo
#   LINUX_REPO                    default googolmo/repo
#   LINUX_REPO_WORKFLOW           default update-index.yml

set -euo pipefail

token="${LINUX_REPO_TOKEN:-${GH_TOKEN:-}}"
if [[ -z "$token" ]]; then
  echo "LINUX_REPO_TOKEN is not set; cannot dispatch googolmo/repo update-index" >&2
  exit 1
fi
export GH_TOKEN="$token"

tag="${TAG:?TAG is required}"
version="${VERSION:?VERSION is required}"
github_repo="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
repo="${LINUX_REPO:-googolmo/repo}"
workflow="${LINUX_REPO_WORKFLOW:-update-index.yml}"

if ! gh workflow view "$workflow" --repo "$repo" >/dev/null 2>&1; then
  echo "workflow ${workflow} not found in ${repo}" >&2
  echo "available workflows:" >&2
  gh workflow list --repo "$repo" >&2 || true
  exit 1
fi

gh workflow run "$workflow" \
  --repo "$repo" \
  --field tag="$tag" \
  --field version="$version" \
  --field github_repo="$github_repo"
echo "dispatched ${repo} ${workflow} for ${tag}"
