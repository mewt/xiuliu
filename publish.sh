#!/bin/sh
# Run with: sh /Users/chumisfum/1day1word/publish.sh
set -eu

fail() { printf '\n%s\n' "$*" >&2; exit 1; }
cd "$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"

for command in cargo git gh curl python3; do
    command -v "$command" >/dev/null 2>&1 || fail "Install $command before publishing."
done

release_tmp=$(mktemp -d)
trap 'rm -rf "$release_tmp"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
cargo metadata --locked --no-deps --format-version 1 > "$release_tmp/metadata.json"
release_name=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["packages"][0]["name"])' "$release_tmp/metadata.json")
release_version=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["packages"][0]["version"])' "$release_tmp/metadata.json")
[ "$release_name" = xiuliu ] || fail 'This script publishes only xiuliu.'
release_repo=mewt/xiuliu
release_tag="v$release_version"

printf '\nPreparing %s %s for GitHub and crates.io\n' "$release_name" "$release_version"
if ! gh auth status >/dev/null 2>&1; then
    gh auth login --hostname github.com --web --git-protocol https
fi
release_login=$(gh api user --jq .login)
[ "$release_login" = mewt ] || fail 'Switch GitHub to the mewt account, then rerun this script.'

# A network error must never be mistaken for an available crate name.
release_status=$(curl --silent --show-error --location --connect-timeout 15 --max-time 45 \
    --user-agent "xiuliu-release-script ($release_login)" \
    --output "$release_tmp/crate.json" --write-out '%{http_code}' \
    "https://crates.io/api/v1/crates/$release_name")
case "$release_status" in
    404) printf 'Crate name is currently unregistered.\n' ;;
    200)
        curl --fail --silent --show-error --location --connect-timeout 15 --max-time 45 \
            --user-agent "xiuliu-release-script ($release_login)" \
            "https://crates.io/api/v1/crates/$release_name/owners" > "$release_tmp/owners.json"
        python3 - "$release_tmp/owners.json" "$release_tmp/crate.json" "$release_version" <<'PY'
import json, sys
owners = json.load(open(sys.argv[1]))["users"]
if not any(owner.get("login") == "mewt" for owner in owners):
    sys.exit("The crate exists without mewt as a direct owner. Verify ownership before publishing.")
versions = json.load(open(sys.argv[2]))["versions"]
if any(version["num"] == sys.argv[3] for version in versions):
    sys.exit("This version is already published. Check crates.io before making another release.")
PY
        ;;
    *) fail "Crates.io returned HTTP $release_status; publication stopped." ;;
esac

if [ -e .git ]; then
    [ "$(git rev-parse --show-toplevel)" = "$(pwd -P)" ] || fail 'Run from the Xiuliu repository.'
    [ "$(git symbolic-ref --short HEAD)" = main ] || fail 'Switch to main before publishing.'
    git diff --cached --quiet || fail 'Existing staged changes found. Commit or unstage them first.'
    if git remote get-url origin >/dev/null 2>&1; then
        for release_url in "$(git remote get-url origin)" "$(git remote get-url --push origin)"; do
            case "$release_url" in
                https://github.com/mewt/xiuliu|https://github.com/mewt/xiuliu.git|git@github.com:mewt/xiuliu.git) ;;
                *) fail "Unexpected origin: $release_url. Inspect it before publishing." ;;
            esac
        done
    fi
fi

cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo publish --locked --dry-run --allow-dirty

if [ ! -e .git ]; then
    git init -b main
fi
git add .gitignore .env.example AGENTS.md Cargo.toml Cargo.lock LICENSE README.md PUBLISHING.md publish.sh src tests assets
if ! git diff --cached --quiet; then
    git commit -m "Release Xiuliu $release_version"
fi
if git rev-parse --verify "refs/tags/$release_tag" >/dev/null 2>&1; then
    [ "$(git rev-parse "$release_tag^{commit}")" = "$(git rev-parse HEAD)" ] || fail "$release_tag already points to another commit."
fi

if ! gh repo view "$release_repo" --json nameWithOwner >/dev/null 2>&1; then
    # Creation fails safely if the lookup failed for a reason other than absence.
    gh repo create "$release_repo" --public --description 'Learn Mandarin in your terminal, one word a day.'
fi
if ! git remote get-url origin >/dev/null 2>&1; then
    git remote add origin "https://github.com/$release_repo.git"
fi
git push --set-upstream origin main

# Reuse Cargo credentials configured for murattal; never print tokens.
if ! cargo publish --locked; then
    fail 'Cargo publication failed; GitHub was pushed. If authentication is missing, run cargo login, then rerun sh publish.sh.'
fi
if ! git rev-parse --verify "refs/tags/$release_tag" >/dev/null 2>&1; then
    git tag "$release_tag"
fi
git push origin "$release_tag"
printf '\nPublished: https://github.com/%s\nCrate: https://crates.io/crates/%s\nInstall: cargo install %s --locked\n' "$release_repo" "$release_name" "$release_name"
