#!/bin/bash -ex

. .world/build_config.sh

if [[ $Target == 'windows'  ]]; then
  # --meson-paths ensures we produce libvmkd.dll.a instead of vmdk.dll.a
  if [[ $Arcihtecture == '32' ]]; then
    RUST_OPTS="--target i686-pc-windows-gnu --meson-paths"
  else
    RUST_OPTS="--target x86_64-pc-windows-gnu --meson-paths"
  fi
fi

cargo cinstall --prefix="$INSTALL" --libdir=lib $RUST_OPTS
