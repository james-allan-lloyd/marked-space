#!/usr/bin/env bash
# Tag the version in Cargo.toml as a release. Pushing v<major>.<minor>.<patch>
# is what triggers .github/workflows/rust-release.yml.
set -xe
VERSION=$(grep -m 1 '^version' Cargo.toml | sed 's/^version = "\(.*\)"$/\1/')
MAJOR=$(echo $VERSION | cut -d. -f1)
MINOR=$(echo $VERSION | cut -d. -f2)
PATCH=$(echo $VERSION | cut -d. -f3)

if [[ $(git branch --show-current) != main ]]; then
  echo "Need to run on main branch"
  exit 1
fi

if [[ -n $(git status --porcelain) ]]; then
  echo "Working tree is dirty: commit or stash before releasing"
  exit 1
fi

# Tag what's on the remote, not a stale or unpushed local main.
git fetch origin main
if [[ $(git rev-parse HEAD) != $(git rev-parse FETCH_HEAD) ]]; then
  echo "main is not in sync with origin/main: pull (or push) before releasing"
  exit 1
fi

echo $VERSION

# The version tag is immutable, so this fails if $VERSION was already released.
# That's the reminder to bump the version in Cargo.toml first.
git tag v$VERSION

# The major and minor tags always move to the release being cut: consumers pin
# to them (uses: james-allan-lloyd/marked-space@v1, and the image tag in
# action.yml), so a release that leaves them behind reaches nobody.
git tag -f v$MAJOR.$MINOR
git tag -f v$MAJOR

# Pushed separately, and only the moving tags are forced. Without --force the
# remote rejects them for already existing, while v$VERSION goes through on its
# own: the release then builds and publishes, but every consumer pinned to v1
# stays on the previous version.
git push origin v$VERSION
git push origin --force v$MAJOR.$MINOR v$MAJOR
