# Publishing Xiuliu

Xiuliu follows the same standard Cargo publishing setup as `mewt/murattal`.
The GitHub destination is `mewt/xiuliu`; both the crate and executable are `xiuliu`.
The prepared version is `0.2.0`.

Run these commands from the repository in a terminal with network access.

## One-command release

```sh
sh /Users/chumisfum/1day1word/publish.sh
```

The script checks GitHub login and crate availability/ownership, runs formatting,
lint, tests, and Cargo's publication dry run, commits the release files, creates
the public repository if needed, pushes `main`, publishes the crate, and pushes
the version tag. It reuses existing Cargo credentials. If Cargo requests login,
run `cargo login` locally and rerun the script. An existing published version
stops the script; it never overwrites a release or force-pushes Git history.

The equivalent manual steps follow.

## Check the destination

```sh
gh auth status
curl --fail-with-body https://crates.io/api/v1/crates/xiuliu
```

A crates.io JSON response saying the crate does not exist means the name is
currently unregistered. A network error does not establish availability. If the
crate exists, verify ownership before publishing; do not publish to another
project's crate. Name availability is only secured by a successful publication.

## Verify and push

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo package --locked
git init -b main
git add .gitignore .env.example AGENTS.md Cargo.toml Cargo.lock LICENSE README.md PUBLISHING.md publish.sh src tests assets
git commit -m "Release Xiuliu 0.2.0: daily Mandarin lessons and analytics"
gh repo create mewt/xiuliu --public --source=. --remote=origin --push
```

These Git commands are for the initial release. If the remote already exists,
inspect it before connecting or pushing; do not overwrite existing history.

## Publish to crates.io

```sh
cargo publish --locked --dry-run
cargo publish --locked
git tag v0.2.0
git push origin v0.2.0
```

Use the Cargo credentials already configured for publishing `murattal`. If Cargo
requires login, run `cargo login` in your own terminal. After publication, the
install command is `cargo install xiuliu --locked` and the executable is `xiuliu`.

Application code is MIT; bundled dictionary data has separate attribution and
share-alike terms documented in `assets/NOTICE.md`. Keep those files in releases.
