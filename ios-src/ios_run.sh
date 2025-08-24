#!/usr/bin/env bash
# Simple iOS runner for bevister using xcodebuild + devicectl/simctl
# Usage:
#   ios_run.sh device   # build, install and run on first connected iOS device (or IOS_DEVICE_ID)
#   ios_run.sh sim      # build, boot/create simulator if needed, install and run
set -euo pipefail

SCHEME=${SCHEME:-VoxelGame}
CONFIGURATION=${CONFIGURATION:-Debug}
DERIVED=${DERIVED_DATA_PATH:-build}
BUNDLE_ID_DEFAULT="im.pala.VoxelGame"
BUNDLE_ID=${BUNDLE_ID:-$BUNDLE_ID_DEFAULT}

# Ensure our paths are sane for Xcode shell phase quirks
export PATH="/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

build_for_device() {
  echo "[iOS] Building $SCHEME ($CONFIGURATION) for physical device..."
  if command -v xcpretty >/dev/null 2>&1; then
    xcodebuild \
      -project VoxelGame.xcodeproj \
      -scheme "$SCHEME" \
      -configuration "$CONFIGURATION" \
      -derivedDataPath "$DERIVED" \
      -destination 'generic/platform=iOS' \
      DEVELOPMENT_TEAM="${DEVELOPMENT_TEAM:-}" \
      CODE_SIGN_STYLE="${CODE_SIGN_STYLE:-Automatic}" \
      CODE_SIGN_IDENTITY="${CODE_SIGN_IDENTITY:-Apple Development}" \
      PROVISIONING_PROFILE_SPECIFIER="${PROVISIONING_PROFILE_SPECIFIER:-}" \
      -allowProvisioningUpdates \
      -allowProvisioningDeviceRegistration \
      build | xcpretty
  else
    xcodebuild \
      -project VoxelGame.xcodeproj \
      -scheme "$SCHEME" \
      -configuration "$CONFIGURATION" \
      -derivedDataPath "$DERIVED" \
      -destination 'generic/platform=iOS' \
      DEVELOPMENT_TEAM="${DEVELOPMENT_TEAM:-}" \
      CODE_SIGN_STYLE="${CODE_SIGN_STYLE:-Automatic}" \
      CODE_SIGN_IDENTITY="${CODE_SIGN_IDENTITY:-Apple Development}" \
      PROVISIONING_PROFILE_SPECIFIER="${PROVISIONING_PROFILE_SPECIFIER:-}" \
      -allowProvisioningUpdates \
      -allowProvisioningDeviceRegistration \
      build
  fi
}

build_for_sim() {
  echo "[iOS] Building $SCHEME ($CONFIGURATION) for iOS Simulator..."
  if command -v xcpretty >/dev/null 2>&1; then
    xcodebuild \
      -project VoxelGame.xcodeproj \
      -scheme "$SCHEME" \
      -configuration "$CONFIGURATION" \
      -derivedDataPath "$DERIVED" \
      -destination 'generic/platform=iOS Simulator' \
      build | xcpretty
  else
    xcodebuild \
      -project VoxelGame.xcodeproj \
      -scheme "$SCHEME" \
      -configuration "$CONFIGURATION" \
      -derivedDataPath "$DERIVED" \
      -destination 'generic/platform=iOS Simulator' \
      build
  fi
}

first_connected_device_udid() {
  # Prefer xctrace (stable across Xcode versions) and filter out Simulators
  xcrun xctrace list devices 2>/dev/null \
    | grep -v Simulator \
    | grep -Eo '\([0-9A-F-]{36}\)' \
    | head -n1 \
    | tr -d '()'
}

ensure_simulator_booted() {
  # Return the UDID of a booted simulator; boot one if none
  local booted
  booted=$(xcrun simctl list devices | sed -n 's/.* (\([0-9A-F-]\{36\}\)) (Booted).*/\1/p' | head -n1 || true)
  if [[ -n "$booted" ]]; then
    echo "$booted"
    return 0
  fi

  # Try to boot a preferred simulator by name if it exists
  local preferred_name
  preferred_name=${IOS_SIM_NAME:-"iPhone 15"}
  if xcrun simctl list devices | grep -q "$preferred_name ("; then
    echo "[iOS] Booting simulator: $preferred_name"
    open -a Simulator || true
    xcrun simctl boot "$preferred_name" || true
    # Wait a bit for boot
    xcrun simctl bootstatus "$preferred_name" -b || true
  fi

  # If still not booted, pick any available Shutdown iPhone and boot it
  booted=$(xcrun simctl list devices | sed -n 's/.* (\([0-9A-F-]\{36\}\)) (Booted).*/\1/p' | head -n1 || true)
  if [[ -z "$booted" ]]; then
    echo "[iOS] No booted simulator found; attempting to boot any available iPhone..."
    local any_shutdown
    any_shutdown=$(xcrun simctl list devices | awk '/iPhone/ && /Shutdown/ {print}' | sed -n 's/.* (\([0-9A-F-]\{36\}\)) (Shutdown).*/\1/p' | head -n1 || true)
    if [[ -n "$any_shutdown" ]]; then
      open -a Simulator || true
      xcrun simctl boot "$any_shutdown" || true
      xcrun simctl bootstatus "$any_shutdown" -b || true
      booted=$any_shutdown
    fi
  fi

  # As a last resort: try to create one using the latest available iOS runtime
  if [[ -z "$booted" ]]; then
    echo "[iOS] Creating a new simulator (this may take a moment)..."
    local dev_type runtime new_name new_id
    # Prefer identifiers over display names to avoid parsing issues across Xcode versions
    dev_type=$(xcrun simctl list devicetypes | grep -E 'iPhone' | grep -Eo 'com\.apple\.CoreSimulator\.SimDeviceType\.[^) ]+' | head -n1 || true)
    runtime=$(xcrun simctl list runtimes | grep -E 'iOS .*\(available\)' | grep -Eo 'com\.apple\.CoreSimulator\.SimRuntime\.iOS-[^) ]+' | tail -1 || true)
    new_name=${IOS_SIM_NAME:-"bevister-sim"}
    if [[ -n "$dev_type" && -n "$runtime" ]]; then
      new_id=$(xcrun simctl create "$new_name" "$dev_type" "$runtime" | tr -d '\n' || true)
      if [[ -n "$new_id" ]]; then
        open -a Simulator || true
        xcrun simctl boot "$new_id" || true
        xcrun simctl bootstatus "$new_id" -b || true
        booted=$new_id
      fi
    fi
  fi

  if [[ -z "$booted" ]]; then
    echo "[iOS] ERROR: Could not boot or create a simulator automatically." >&2
    exit 1
  fi

  echo "$booted"
}

run_on_device() {
  build_for_device
  local app_path
  app_path="$DERIVED/Build/Products/$CONFIGURATION-iphoneos/$SCHEME.app"
  if [[ ! -d "$app_path" ]]; then
    echo "[iOS] ERROR: Built app not found at $app_path" >&2
    exit 1
  fi

  # Resolve the bundle identifier from the built app's Info.plist if available
  local bundle_id_resolved="$BUNDLE_ID"
  if [[ -f "$app_path/Info.plist" ]]; then
    local from_plist
    from_plist=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$app_path/Info.plist" 2>/dev/null || true)
    if [[ -z "$from_plist" ]]; then
      from_plist=$(defaults read "$app_path/Info" CFBundleIdentifier 2>/dev/null || true)
    fi
    if [[ -n "$from_plist" ]]; then
      bundle_id_resolved="$from_plist"
    fi
  fi

  local udid
  udid=${IOS_DEVICE_ID:-}
  if [[ -z "${udid}" ]]; then
    udid=$(first_connected_device_udid || true)
  fi
  if [[ -z "${udid}" ]]; then
    echo "[iOS] ERROR: No connected iOS device found. Set IOS_DEVICE_ID to override." >&2
    exit 1
  fi
  echo "[iOS] Installing to device $udid ..."
  xcrun devicectl device install app --device "$udid" "$app_path"
  echo "[iOS] Launching $bundle_id_resolved on device $udid ..."
  xcrun devicectl device process launch --terminate-existing --device "$udid" "$bundle_id_resolved"
}

run_on_sim() {
  build_for_sim
  local app_path
  app_path="$DERIVED/Build/Products/$CONFIGURATION-iphonesimulator/$SCHEME.app"
  if [[ ! -d "$app_path" ]]; then
    echo "[iOS] ERROR: Built simulator app not found at $app_path" >&2
    exit 1
  fi

  # Resolve bundle id from Info.plist
  local bundle_id_resolved="$BUNDLE_ID"
  if [[ -f "$app_path/Info.plist" ]]; then
    local from_plist
    from_plist=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$app_path/Info.plist" 2>/dev/null || true)
    if [[ -z "$from_plist" ]]; then
      from_plist=$(defaults read "$app_path/Info" CFBundleIdentifier 2>/dev/null || true)
    fi
    if [[ -n "$from_plist" ]]; then
      bundle_id_resolved="$from_plist"
    fi
  fi

  local booted
  booted=$(ensure_simulator_booted)
  echo "[iOS] Using simulator $booted"
  echo "[iOS] Installing app to simulator..."
  xcrun simctl install "$booted" "$app_path" || xcrun simctl install booted "$app_path"
  echo "[iOS] Launching $bundle_id_resolved on simulator..."
  xcrun simctl launch "$booted" "$bundle_id_resolved" || xcrun simctl launch booted "$bundle_id_resolved"
}

mode=${1:-}
case "$mode" in
  device)
    run_on_device
    ;;
  sim)
    run_on_sim
    ;;
  *)
    echo "Usage: $0 {device|sim}" >&2
    exit 2
    ;;
 esac

