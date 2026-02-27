#!/bin/bash -ex

. .world/build_config.sh

# don't build for 32-bit Windows for now
if [[ $Target == 'windows' && $Architecture == '32' ]]; then
  exit
fi

if [[ $Target == 'windows'  ]]; then
  # --meson-paths ensures we produce libvmkd.dll.a instead of vmdk.dll.a.
  RUST_OPTS="--target x86_64-pc-windows-gnu --meson-paths"
fi

cargo cinstall --prefix="$INSTALL" --libdir=lib $RUST_OPTS
