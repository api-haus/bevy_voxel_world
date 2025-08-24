#!/usr/bin/env bash

# based on https://github.com/mozilla/glean/blob/main/build-scripts/xc-universal-binary.sh

set -eux

PATH=$PATH:$HOME/.cargo/bin

PROFILE=debug
RELFLAG=
if [[ "$CONFIGURATION" != "Debug" ]]; then
    PROFILE=release
    RELFLAG=--release
fi

set -euvx
PROFILE_DIR=$PROFILE

# add homebrew bin path, as it's the most commonly used package manager on macOS
# this is needed for cmake on apple arm processors as it's not available by default
export PATH="$PATH:/opt/homebrew/bin"

# Make Cargo output cache files in Xcode's directories
export CARGO_TARGET_DIR="$DERIVED_FILE_DIR/cargo"

# Xcode places `/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin`
# at the front of the path, with makes the build fail with `ld: library 'System' not found`, upstream issue:
# <https://github.com/rust-lang/rust/issues/80817>.
#
# Work around it by resetting the path, so that we use the system `cc`.
export PATH="/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

IS_SIMULATOR=0
if [ "${LLVM_TARGET_TRIPLE_SUFFIX-}" = "-simulator" ]; then
  IS_SIMULATOR=1
fi

EXECUTABLES=
for arch in $ARCHS; do
  case "$arch" in
    x86_64)
      if [ $IS_SIMULATOR -eq 0 ]; then
        echo "Building for x86_64, but not a simulator build. What's going on?" >&2
        exit 2
      fi

      # Intel iOS simulator
      export CFLAGS_x86_64_apple_ios="-target x86_64-apple-ios"
      TARGET=x86_64-apple-ios
      ;;

    arm64)
      if [ $IS_SIMULATOR -eq 0 ]; then
        # Hardware iOS targets
        TARGET=aarch64-apple-ios
      else
        # M1 iOS simulator
        TARGET=aarch64-apple-ios-sim
      fi
  esac

  # Build the Rust iOS static library for the current arch/target
  cargo build $RELFLAG --lib --target $TARGET -p voxel_demo_app

  # Collect the static libraries
  EXECUTABLES="$EXECUTABLES $DERIVED_FILE_DIR/cargo/$TARGET/$PROFILE_DIR/libvoxel_demo_app.a"
done

# Combine static libraries into a universal one in the built products dir
lipo -create -output "$BUILT_PRODUCTS_DIR/libvoxel_demo_app.a" $EXECUTABLES
