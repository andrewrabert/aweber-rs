# AWeber CLI

A CLI tool and Rust library for the AWeber API.

## Skills

This project has custom skills in `.claude/skills/`. You MUST use them:

- **release** — Use for ANY version bump, tagging, or release task. This includes requests like "tag a release", "cut a release", "bump the version", or "release v0.x.x". Always invoke this skill before taking any action.
- **aweber-api-reference** — Use when working with AWeber API endpoints, adding or modifying API calls, checking parameter names, response schemas, or verifying endpoint paths and HTTP methods.

## Release Process

NEVER manually tag, bump versions, or create releases without invoking the `release` skill first. The skill defines the exact steps, commit message format, and tag format. Skipping it will produce incorrect releases.

Release notes go in annotated tag messages, not in a changelog file.

## Build & Test

```sh
cargo build
cargo test
cargo clippy
```
