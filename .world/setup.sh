#!/bin/bash -ex

. .world/build_config.sh

# don't build for 32-bit Windows for now
if [[ $Target == 'windows' && $Architecture == '32' ]]; then
  exit
fi

# if we're building for windows, we need to install the windows toolchain
if [ "$Target" = 'windows' ]; then
# rustup lock wrapper
(
  # Acquire the lock (blocking mode — will wait until it's free)
  flock 9

  # Critical section
  rustup target add x86_64-pc-windows-gnu

) 9>/tmp/.rustup.lock
fi

# use cargo-c's lock file so that updates in its dependencies don't affect us
cargo install cargo-c --locked
