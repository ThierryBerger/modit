# modit task runner.
#
# The project spans several cargo workspaces (host, plus one per firmware crate)
# that cannot be merged -- see the comment at the top of Cargo.toml. These
# recipes hide that split so you never have to remember which directory to be
# in.

BUTTON := "crates/module-button"
COIN := "crates/module-coin"

# List the available recipes.
default:
    @just --list

# Check every workspace. This is the "did I break anything" command.
check: check-host check-firmware

# Check the host crates (shared + brain).
check-host:
    cargo check --workspace --all-targets

# Check every firmware crate. Needs the `esp` toolchain -- run `just setup` if
# this fails. MODIT_ID only has to be *a* value to type-check; flashing is what
# makes it real.
check-firmware:
    cd {{BUTTON}} && MODIT_ID=check cargo check --all-targets
    cd {{COIN}} && MODIT_ID=check cargo check --all-targets

# Run the brain against real hardware.
brain *ARGS:
    RUST_LOG=${RUST_LOG:-info} cargo run -p brain -- {{ARGS}}

# Run a scenario with no radio and no boards. Presses come from the keyboard.
# Try `just simulate --scenario simon`, or `--scenario arcade` for the coin
# module (type `$` at the prompt to post a coin).
simulate *ARGS:
    RUST_LOG=${RUST_LOG:-info} cargo run -p brain -- --simulate {{ARGS}}

# Build, flash and monitor the firmware for ONE board.
#
# The id distinguishes this board from the others and is baked into the binary,
# so flash each board separately:  just flash a   /   just flash b
flash id *ARGS:
    cd {{BUTTON}} && MODIT_ID={{id}} cargo run --release {{ARGS}}

# Build, flash and monitor the coin acceptor firmware.
#
# The id must match what the scenario expects -- `arcade` looks for `slot`, so
# that is usually `just flash-coin slot`. Read docs/HARDWARE.md first: this
# module involves 12 V, and getting the pulse line wrong costs a GPIO.
flash-coin id *ARGS:
    cd {{COIN}} && MODIT_ID={{id}} cargo run --release {{ARGS}}

# Watch the serial output of an already-flashed board.
monitor:
    espflash monitor

# Format every workspace.
fmt:
    cargo fmt --all
    cd {{BUTTON}} && cargo fmt --all
    cd {{COIN}} && cargo fmt --all

# Verify formatting without writing (what CI runs).
fmt-check:
    cargo fmt --all -- --check
    cd {{BUTTON}} && cargo fmt --all -- --check
    cd {{COIN}} && cargo fmt --all -- --check

# Lint every workspace.
clippy:
    cargo clippy --workspace --all-targets -- -D warnings
    cd {{BUTTON}} && MODIT_ID=check cargo clippy --all-targets -- -D warnings
    cd {{COIN}} && MODIT_ID=check cargo clippy --all-targets -- -D warnings

# Run the host tests.
test:
    cargo test --workspace

# Everything CI would run. Mirrors .github/workflows/ci.yml.
ci: fmt-check check clippy test build-firmware

# Build (not just check) every firmware: the panic handler resolves at link
# time, so `cargo check` cannot catch a missing custom_halt.
build-firmware:
    cd {{BUTTON}} && MODIT_ID=check cargo build --release
    cd {{COIN}} && MODIT_ID=check cargo build --release

# Regenerate the wiring schematics from schematics.py.
#
# Figures for a module that exists land in docs/hardware/, next to its page;
# figures for one still being designed land in plans/figures/, next to its plan.
#
# Deliberately NOT part of `just ci`: the outputs are committed, so CI does not
# need Python, and a contributor who only edits Rust never installs schemdraw.
# What CI *does* check is that the net tables still match the firmware pins --
# that is `wiring_tables_match_firmware`, in the normal test run.
schematics:
    python3 -m pip install --quiet --user schemdraw matplotlib
    python3 docs/hardware/schematics.py

# Build the documentation site into target/site/book.
#
# Two steps, deliberately separate: `stage.py` copies the markdown that already
# lives in docs/, plans/, README.md and CHECKME.md into a throwaway tree and
# generates the table of contents, then mdBook renders it. The site has no copy of
# any page -- that is the whole point, and why no page has to be written twice.
#
# Needs mdbook: `cargo install mdbook`.
site:
    python3 docs/site/stage.py
    mdbook build
    python3 docs/site/check_links.py

# Build the site and serve it locally with live reload.
#
# Re-run `just site` after editing a page: mdbook watches the *staged* tree, not
# docs/, so an edit in docs/ is not picked up on its own.
site-serve: site
    mdbook serve --open

# Install the firmware toolchain. Remember to `source ~/export-esp.sh` after.
setup:
    cargo install espup espflash
    espup install
    @echo ""
    @echo "Now run:  source ~/export-esp.sh"
    @echo "(needed once per shell, before any firmware command)"
