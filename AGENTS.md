# Agent Guide - jav-fs

This repository contains `jav-fs`, a Rust-based tool for scanning and managing video files, specifically focused on extracting IDs and handling SMB/UNC paths. It is designed to be fast, multi-threaded, and robust against various filename formats.

## Git

- DO NOT commit changes without asking for permission.

## Build, Lint, and Test Commands

All operations use standard Cargo commands. It is recommended to run these from the project root.

### Build
- **Build project:** `cargo build`
- **Build release version:** `cargo build --release` (recommended for actual usage due to performance optimizations)

### Lint and Format
- **Check formatting:** `cargo fmt -- --check`
- **Apply formatting:** `cargo fmt`
- **Lint with clippy:** `cargo clippy`
- **Fix clippy suggestions:** `cargo clippy --fix`

### Testing
- **Run all tests:** `cargo test`
- **Run a specific test:** `cargo test <test_function_name>`
  - Example: `cargo test test_convert_smb_url_to_unc_basic`
- **Run tests in a module:** `cargo test tests::<module_name>`
- **Run tests with output:** `cargo test -- --nocapture` (useful for debugging with `println!`)

### Running the Application
- **Execute from source:** `cargo run -- <URL> [ARGS]`
- **Example:** `cargo run -- smb://nas/video --threads 4`

## Code Style Guidelines

### Language & Edition
- **Rust:** Edition 2021 as specified in `Cargo.toml`. Avoid using deprecated features or editions.

### Imports
- Group imports in the following order, with a blank line between groups:
  1. Standard library (`std::...`)
  2. External dependencies (`clap`, `dashmap`, `url`, etc.)
  3. Local module imports (`use crate::...` or `use jav_fs::...`)
- Prefer explicit imports (e.g., `use std::sync::Arc`) over wildcard imports (`use std::sync::*`), except in test modules where `use super::*;` is the standard pattern.

### Formatting
- Strictly follow `rustfmt` defaults.
- Maximum line length is generally 100-120 characters, but rely on `cargo fmt` to handle this automatically.
- Use 4 spaces for indentation.

### Naming Conventions
- **Variables/Functions/Modules:** `snake_case` (e.g., `extract_id_from_filename`, `scan_path`).
- **Structs/Enums/Traits:** `PascalCase` (e.g., `Args`, `WalkState`).
- **Constants:** `SCREAMING_SNAKE_CASE`.
- **Booleans:** Prefix with `is_`, `has_`, or `can_` where appropriate (e.g., `is_video_file`, `has_auth`).

### Types and Ownership
- Use `String` for owned text data and `&str` for read-only string slices.
- Leverage Rust's type inference for local variables, but provide explicit types for complex generic structures (like `Arc<DashMap<String, String>>`) or public API signatures.
- **Thread Safety:** The project uses `Arc<T>` for shared ownership, `AtomicUsize` for shared counters, and thread-safe collections like `DashMap` for concurrent storage. Avoid `Mutex<T>` unless absolutely necessary for complex state synchronization.

### Error Handling
- Use `Result<T, E>` for recoverable errors and `Option<T>` for optional values.
- Prefer `map_err` to convert error types or add context to error strings.
  - Example: `Url::parse(url).map_err(|e| format!("Failed to parse URL: {}", e))?`
- Error messages should be descriptive, concise, and start with an uppercase letter.
- Use `unwrap()` or `expect()` sparingly. They are acceptable in:
  - Unit tests.
  - Initializing `Regex` objects that are known to be valid.
  - Situations where failure is genuinely impossible (e.g., getting a filename from a path that was just verified to be a file).

### Regular Expressions
- Regular expressions are used for identifying video files and extracting IDs.
- Currently, `Regex::new()` is called within functions. If a function is called in a tight loop (like during a scan), consider moving the `Regex` to a `once_cell::sync::Lazy` or `lazy_static!` for better performance.
- ID Extraction Pattern: `r"[[:alpha:]]+-\d+|[[:alpha:]]+\d+"` (matches alphanumeric IDs with or without dashes).

### Documentation and Comments
- Use `///` for doc comments on public functions, structs, and modules. Include a brief description of what it does, parameters, and return values.
- Use `//` for internal implementation notes.
- Focus comments on the *why* (the intent) rather than the *what* (the code itself), especially for non-obvious logic.
- Avoid "commenting out" code; delete it instead.

## Project Structure and Architecture

### Files
- `src/main.rs`: Entry point. Handles CLI argument parsing (using `clap`), SMB authentication (using `net use` on Windows), and orchestrates the scanning process.
- `src/lib.rs`: The core logic library. Contains URL conversion utilities, filename filters, and ID extraction logic. This is where most unit tests reside.
- `Cargo.toml`: Project metadata and dependencies.

### Concurrency Model
- The scanner uses `ignore::WalkBuilder` with `build_parallel()` to walk the filesystem concurrently.
- Results are collected into an `Arc<DashMap<String, String>>` to avoid global locks and maximize throughput.
- Progress is reported via `indicatif::ProgressBar`, updated from multiple threads using atomic counters.

### SMB Handling
- SMB URLs (`smb://host/share`) are converted to UNC paths (`\\host\share`) for native Windows file access.
- Authentication is handled by executing the `net use` command if credentials are provided in the URL.

## Development Workflow for Agents
1. **Analyze:** Before making changes, read `src/lib.rs` and `src/main.rs` to understand existing patterns.
2. **Implement:** Write idiomatic Rust code following the guidelines above.
3. **Test:** Add unit tests in `src/lib.rs` for any new logic. Run all tests with `cargo test`.
4. **Lint:** Run `cargo clippy` and `cargo fmt` before finishing.
5. **Verify:** If changes affect the CLI, run the application with `cargo run -- <args>` to ensure it behaves as expected.
