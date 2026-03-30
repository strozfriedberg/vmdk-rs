#!/bin/bash -ex

. .world/build_config.sh

if [[ $Target == 'windows' ]]; then
  if [[ $Architecture == '32' ]]; then
    RUST_OPTS="--target i686-pc-windows-gnu --config target.i686-pc-windows-gnu.runner='wine' --meson-paths"
  else
    RUST_OPTS="--target x86_64-pc-windows-gnu --config target.x86_64-pc-windows-gnu.runner='wine' --meson-paths"
  fi
fi

cargo clippy --all-features --all-targets
cargo ctest --prefix="$INSTALL" --libdir=lib $RUST_OPTS
