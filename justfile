# Default recipe: list available commands
default:
    @just --list

# Run a candidate with the dashboard already up
dev candidate='spike-gtk4' *args='':
    cargo run -p {{candidate}} -- --visible --no-portal {{args}}

# Run a candidate as it is meant to run: hidden, waiting for the global shortcut.
#
# The first run opens GNOME's permission dialog for the shortcut. Accepting it once is
# what makes every later run silent.
daemon candidate='spike-gtk4' *args='':
    cargo run -p {{candidate}} -- {{args}}

# Show, hide, or toggle a running candidate
show:
    cargo run -q -p glimtctl -- show
hide:
    cargo run -q -p glimtctl -- hide
toggle:
    cargo run -q -p glimtctl -- toggle

# Measure one candidate. Release, because a debug build measures the profile.
bench candidate='spike-gtk4' iterations='200' mode='window':
    cargo build --release -p {{candidate}} -p glimt-bench
    ./target/release/glimt-bench --candidate {{candidate}} --iterations {{iterations}} --mode {{mode}}

# Measure both, one after the other rather than at once: two daemons would compete for
# the same bus name, and a loaded machine would be measured instead of a toolkit.
bench-all iterations='200':
    just bench spike-gtk4 {{iterations}}
    just bench spike-raw {{iterations}}

# Print the two reports side by side
compare:
    @jq -s '.[] | {candidate, mode, geometry, cold_ms: (.cold_start_us/1000), \
        pss_mb: (.idle.pss_kb/1024), rss_mb: (.idle.rss_kb/1024), \
        p50_ms: (.latency.p50_us/1000), p95_ms: (.latency.p95_us/1000), \
        p99_ms: (.latency.p99_us/1000)}' bench-results/*.json

# The layer-shell check, without leaving GNOME and without taking the screen.
#
# sway is a wlroots compositor, so inside it the protocol mutter lacks is present, and
# the headless backend means it opens no window at all. A candidate that works here
# works on niri, Hyprland, sway and COSMIC. cage will not do: it is a kiosk compositor
# and leaves layer-shell out on purpose.
nested args='--mode layer-shell' candidate='spike-raw':
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --release -p {{candidate}}
    conf="$(mktemp)"
    log="$(mktemp)"
    trap 'rm -f "$conf" "$log"' EXIT
    printf 'exec "%s/target/release/%s %s --no-portal --visible > %s 2>&1; swaymsg exit"\n' \
        "$PWD" "{{candidate}}" "{{args}}" "$log" > "$conf"
    WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 XDG_CURRENT_DESKTOP=sway \
        timeout 25 sway --config "$conf" >/dev/null 2>&1 || true
    cat "$log"

# The same for the GTK candidate, which reaches layer-shell through its own library
nested-gtk4:
    just nested --layer-shell spike-gtk4

# Which Wayland protocols the compositor of the day offers
protocols:
    @wayland-info | grep -oE "interface: '[a-z_0-9]+'" | sort -u

# Format all code
fmt:
    treefmt

# Check formatting
fmt-check:
    treefmt --fail-on-change --no-cache

# Run clippy lints
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Run all tests
test:
    cargo test --all-features

# Verify Cargo.lock is in step with the manifests
lock-check:
    cargo metadata --locked --format-version 1 > /dev/null

# Run CI checks locally
ci: fmt-check lint test lock-check
    @echo "All CI checks passed!"
