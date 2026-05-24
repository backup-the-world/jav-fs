set shell := ["bash", "-cu"]

# Install local Git hooks managed by Cocogitto.
setup:
    @command -v cog >/dev/null || { echo "Missing cocogitto. Install with: cargo install cocogitto"; exit 1; }
    cog install-hook --all --overwrite
    @echo "Installed local git hooks."

# Format Rust code in-place.
fmt:
    cargo fmt

# Check Rust formatting without modifying files.
fmt-check:
    cargo fmt -- --check

# Run Clippy for all targets and fail on warnings.
lint:
    cargo clippy --all-targets -- -D warnings

# Run tests for all targets.
test:
    cargo test --all-targets

# Full local quality gate.
check: fmt-check lint test

# Run the same code checks used by the pre-commit hook.
pre-commit:
    ./scripts/pre-commit.sh
