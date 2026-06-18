# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## Project

SpectraForge — a Rust project (edition 2024), currently a fresh scaffold (`src/main.rs` is the default Hello World, no dependencies yet).

## Commands

```bash
cargo run            # build and run
cargo build          # build (use --release for optimized)
cargo test           # run all tests
cargo test <name>    # run a single test by name
cargo clippy         # lint
cargo fmt            # format
```

## Architecture

Single binary crate. No structure has emerged yet — update this file as modules and dependencies are added.
