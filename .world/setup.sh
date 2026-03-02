#!/bin/bash -ex

. .world/build_config.sh

# if we're building for windows, we need to install the windows toolchain
if [ "$Target" = 'windows' ]; then
# rustup lock wrapper
(
  # Acquire the lock (blocking mode — will wait until it's free)
  flock 9

  # Critical section
  rustup target add x86_64-pc-windows-gnu
  rustup target add i686-pc-windows-gnu

) 9>/tmp/.rustup.lock
fi

# pin version until we update to a newer Rust toolchain
cargo install cargo-c@0.10.16+cargo-0.91.0 --locked
