# modit task runner.
#
# The project spans two cargo workspaces (host + firmware) that cannot be
# merged -- see the comment at the top of Cargo.toml. These recipes hide that
# split so you never have to remember which directory to be in.

FIRMWARE := "crates/buttons"

# List the available recipes.
default:
    @just --list

# Check both workspaces. This is the "did I break anything" command.
check: check-host check-firmware

# Check the host crates (shared + brain).
check-host:
    cargo check --workspace --all-targets

# Check the firmware. Needs the `esp` toolchain -- run `just setup` if this fails.
check-firmware:
    cd {{FIRMWARE}} && cargo check --all-targets

# Run the brain against real hardware.
brain *ARGS:
    RUST_LOG=${RUST_LOG:-info} cargo run -p brain -- {{ARGS}}

# Build, flash and monitor the firmware. Connect exactly one board first.
flash *ARGS:
    cd {{FIRMWARE}} && cargo run --release {{ARGS}}

# Watch the serial output of an already-flashed board.
monitor:
    espflash monitor

# Format both workspaces.
fmt:
    cargo fmt --all
    cd {{FIRMWARE}} && cargo fmt --all

# Verify formatting without writing (what CI runs).
fmt-check:
    cargo fmt --all -- --check
    cd {{FIRMWARE}} && cargo fmt --all -- --check

# Lint both workspaces.
clippy:
    cargo clippy --workspace --all-targets -- -D warnings
    cd {{FIRMWARE}} && cargo clippy --all-targets -- -D warnings

# Run the host tests.
test:
    cargo test --workspace

# Everything CI would run.
ci: fmt-check check clippy test

# Install the firmware toolchain. Remember to `source ~/export-esp.sh` after.
setup:
    cargo install espup espflash
    espup install
    @echo ""
    @echo "Now run:  source ~/export-esp.sh"
    @echo "(needed once per shell, before any firmware command)"
