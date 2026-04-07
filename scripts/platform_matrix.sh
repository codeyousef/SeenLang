#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")"/.. && pwd)"
OUTPUT_DIR="$ROOT_DIR/artifacts/platform-matrix"
STAGE3_BIN=""
TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
SMOKE_HARNESS="$ROOT_DIR/scripts/native_target_smoke.sh"
RUN_RUNTIME=0
PLATFORMS=(
  "linux-x86_64"
  "linux-arm64"
  "windows-x86_64"
  "macos-x86_64"
  "macos-arm64"
  "android-arm64"
  "ios-arm64"
  "ios-sim-arm64"
)
IOS_DEVICE_ID=""
IOS_BUNDLE_PREFIX="com.seen.matrix"
IOS_SIGN_IDENTITY=""
IOS_PROVISIONING_PROFILE=""
IOS_ENTITLEMENTS=""
APPLE_RUNTIME_CASES=(
  "hello_english"
  "test_comptime_target_predicates"
  "hash_map_basic"
  "string_hash_map_basic"
  "str_basic"
  "string_buffer_basic"
)

usage() {
  cat <<'USAGE'
platform_matrix.sh - Smoke test Stage3 toolchain across native platforms.

Usage: scripts/platform_matrix.sh [options]

Options:
  --stage3 <path>      Path to Stage3 Seen compiler (default: ./stage3_seen if present)
  --output-dir <dir>   Directory to store JSON reports (default: artifacts/platform-matrix)
  --platform <name>    Limit run to a single platform (repeatable)
  --with-runtime       Run supported runtime checks in addition to compile smoke
  --ios-device <id>    iOS physical device identifier for ios-arm64 runtime checks
  --ios-bundle-prefix <prefix>
                       Bundle identifier prefix for simulator/device runtime apps
  --ios-sign-identity <name>
                       Code-signing identity for ios-arm64 runtime apps
  --ios-provisioning-profile <path>
                       Provisioning profile for ios-arm64 runtime apps
  --ios-entitlements <path>
                       Optional entitlements plist for ios-arm64 runtime apps
  -h, --help           Show this help message
USAGE
}

SELECTED=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --stage3)
      STAGE3_BIN="$2"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --platform)
      SELECTED+=("$2")
      shift 2
      ;;
    --with-runtime)
      RUN_RUNTIME=1
      shift
      ;;
    --ios-device)
      IOS_DEVICE_ID="$2"
      shift 2
      ;;
    --ios-bundle-prefix)
      IOS_BUNDLE_PREFIX="$2"
      shift 2
      ;;
    --ios-sign-identity)
      IOS_SIGN_IDENTITY="$2"
      shift 2
      ;;
    --ios-provisioning-profile)
      IOS_PROVISIONING_PROFILE="$2"
      shift 2
      ;;
    --ios-entitlements)
      IOS_ENTITLEMENTS="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage
      exit 1
      ;;
  esac
 done

if [[ -z "$STAGE3_BIN" ]]; then
  if [[ -x "$ROOT_DIR/stage3_seen" ]]; then
    STAGE3_BIN="$ROOT_DIR/stage3_seen"
  else
    STAGE3_BIN="$(command -v seen_cli || true)"
    if [[ -z "$STAGE3_BIN" ]]; then
      echo "stage3_seen not found and seen_cli unavailable; pass --stage3" >&2
      exit 1
    fi
    echo "[matrix] stage3 binary missing; falling back to $STAGE3_BIN (development mode)"
  fi
fi

if [[ ! -f "$SMOKE_HARNESS" ]]; then
  echo "native smoke harness not found: $SMOKE_HARNESS" >&2
  exit 1
fi

mkdir -p "$OUTPUT_DIR/$TIMESTAMP"
REPORT_DIR="$OUTPUT_DIR/$TIMESTAMP"

declare -a TARGET_PLATFORMS
if [[ ${#SELECTED[@]} -gt 0 ]]; then
  TARGET_PLATFORMS=("${SELECTED[@]}")
else
  TARGET_PLATFORMS=("${PLATFORMS[@]}")
fi

has_failures=0

run_linux() {
  local platform="$1"
  local report="$2"
  local smoke_dir="$REPORT_DIR/native-smoke-$platform"
  local smoke_status="failure"
  local smoke_artifact=""
  local smoke_note=""
  local smoke_log=""
  local runtime_status="success"
  local runtime_message=""
  local platform_status="success"
  local platform_message=""

  mkdir -p "$smoke_dir"

  if bash "$SMOKE_HARNESS" --compiler "$STAGE3_BIN" --output-dir "$smoke_dir" --target "$platform" >/tmp/platform_matrix_linux_smoke.log 2>&1; then
    :
  fi
  local smoke_summary
  smoke_summary="$(parse_smoke_summary "$smoke_dir" "$platform")"
  IFS=$'\t' read -r smoke_status smoke_artifact smoke_note <<< "$smoke_summary"
  smoke_log="$(find_smoke_log "$smoke_dir" "$platform")"
  if [[ -z "$smoke_log" ]]; then
    smoke_log="$smoke_dir/$platform/build.log"
  fi

  if [[ "$smoke_status" == "failure" ]]; then
    platform_status="failure"
    runtime_status="skipped"
    runtime_message="Linux runtime examples skipped because compile smoke failed"
    platform_message="$smoke_note"
  elif [[ "$smoke_status" == "unavailable" ]]; then
    platform_status="unavailable"
    runtime_status="skipped"
    runtime_message="Linux runtime examples skipped because compile smoke was unavailable"
    platform_message="$smoke_note"
  elif [[ "$RUN_RUNTIME" -eq 1 ]]; then
    if ! run_with_timeout_capture /tmp/linux_ecs_run.log "$STAGE3_BIN" run "$ROOT_DIR/examples/seen-ecs-min/main.seen"; then
      runtime_status="failure"
      runtime_message="failed to run seen-ecs-min"
    fi

    if [[ "$runtime_status" == "success" ]]; then
      if ! run_with_timeout_capture /tmp/linux_vulkan.log "$STAGE3_BIN" run "$ROOT_DIR/examples/seen-vulkan-min/main.seen"; then
        runtime_status="failure"
        runtime_message="failed to run seen-vulkan-min"
      fi
    fi

    if [[ "$runtime_status" == "failure" ]]; then
      platform_status="failure"
      platform_message="$runtime_message"
    else
      platform_message="$smoke_note"
    fi
  else
    runtime_status="skipped"
    runtime_message="Runtime checks skipped; pass --with-runtime to execute them"
    platform_message="$runtime_message"
  fi

  if [[ -z "$platform_message" ]]; then
    platform_message="$smoke_note"
  fi

  cat > "$report" <<JSON
{
  "platform": "$platform",
  "status": "$platform_status",
  "stage3": "${STAGE3_BIN}",
  "smoke_status": "$smoke_status",
  "smoke_artifact": "$(json_escape "$smoke_artifact")",
  "smoke_note": "$(json_escape "$smoke_note")",
  "smoke_log": "$(json_escape "$smoke_log")",
  "ecs_log": "$(json_escape_file /tmp/linux_ecs.log)",
  "ecs_run_log": "$(json_escape_file /tmp/linux_ecs_run.log)",
  "vulkan_log": "$(json_escape_file /tmp/linux_vulkan.log)",
  "runtime_status": "$runtime_status",
  "message": "$(json_escape "$platform_message")"
}
JSON
}

json_escape() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//"/\\"}"
  value="${value//$'\n'/ }"
  value="${value//$'\r'/ }"
  printf '%s' "$value"
}

json_escape_file() {
  local file_path="$1"
  if [[ ! -f "$file_path" ]]; then
    printf ''
    return
  fi
  sed 's/\\/\\\\/g; s/"/\\"/g' < "$file_path" | tr '\n' ' '
}

tsv_escape() {
  local value="$1"
  value="${value//$'\t'/ }"
  value="${value//$'\n'/ }"
  value="${value//$'\r'/ }"
  printf '%s' "$value"
}

parse_smoke_summary() {
  local smoke_root="$1"
  local platform="$2"
  local summary_file
  summary_file="$(find "$smoke_root" -name summary.tsv -print | head -n 1)"
  if [[ -z "$summary_file" || ! -f "$summary_file" ]]; then
    printf 'failure\t\t%s\n' "smoke summary not generated"
    return
  fi
  local row
  row="$(awk -F '\t' -v target="$platform" 'NR > 1 && $1 == target { print; exit }' "$summary_file")"
  if [[ -z "$row" ]]; then
    printf 'failure\t\t%s\n' "target missing from smoke summary"
    return
  fi
  local parsed_target
  local parsed_status
  local parsed_artifact
  local parsed_note
  IFS=$'\t' read -r parsed_target parsed_status parsed_artifact parsed_note <<< "$row"
  printf '%s\t%s\t%s\n' "$parsed_status" "$parsed_artifact" "$parsed_note"
}

find_smoke_log() {
  local smoke_root="$1"
  local platform="$2"
  find "$smoke_root" -path "*/$platform/build.log" -print | head -n 1
}

find_case_results_file() {
  local smoke_root="$1"
  local platform="$2"
  find "$smoke_root" -path "*/$platform/case-results.tsv" -print | head -n 1
}

find_case_artifact() {
  local smoke_root="$1"
  local platform="$2"
  local case_name="$3"
  local case_results_file
  case_results_file="$(find_case_results_file "$smoke_root" "$platform")"
  if [[ -z "$case_results_file" || ! -f "$case_results_file" ]]; then
    printf ''
    return
  fi
  awk -F '\t' -v case_name="$case_name" 'NR > 1 && $1 == case_name { print $4; exit }' "$case_results_file"
}

run_with_timeout_capture() {
  local log_file="$1"
  shift
  if command -v timeout >/dev/null 2>&1; then
    timeout 180 "$@" >"$log_file" 2>&1
  else
    "$@" >"$log_file" 2>&1
  fi
}

run_with_timeout_capture_append() {
  local log_file="$1"
  shift
  if command -v timeout >/dev/null 2>&1; then
    timeout 180 "$@" >>"$log_file" 2>&1
  else
    "$@" >>"$log_file" 2>&1
  fi
}

sanitize_identifier_component() {
  printf '%s' "$1" | tr -cd '[:alnum:]' | tr '[:upper:]' '[:lower:]'
}

runtime_case_app_name() {
  local case_name="$1"
  local app_name
  app_name="$(printf '%s' "$case_name" | tr -cd '[:alnum:]')"
  if [[ -z "$app_name" ]]; then
    app_name="SeenRuntimeCase"
  fi
  printf 'Seen%s' "$app_name"
}

runtime_case_bundle_id() {
  local platform="$1"
  local case_name="$2"
  local platform_component
  local case_component
  platform_component="$(sanitize_identifier_component "$platform")"
  case_component="$(sanitize_identifier_component "$case_name")"
  if [[ -z "$platform_component" ]]; then
    platform_component="apple"
  fi
  if [[ -z "$case_component" ]]; then
    case_component="runtimecase"
  fi
  printf '%s.%s.%s' "$IOS_BUNDLE_PREFIX" "$platform_component" "$case_component"
}

runtime_case_expected_output() {
  local case_name="$1"
  case "$case_name" in
    hello_english)
      printf 'Hello, World!'
      ;;
    test_comptime_target_predicates)
      printf 'PASS: comptime_target_predicates'
      ;;
    hash_map_basic)
      printf 'HashMap Test Results:'
      ;;
    *)
      printf ''
      ;;
  esac
}

normalize_runtime_field() {
  if [[ "$1" == "__EMPTY__" ]]; then
    printf ''
    return
  fi
  printf '%s' "$1"
}

validate_runtime_log_output() {
  local platform="$1"
  local case_name="$2"
  local log_file="$3"
  local expected_output=""
  local expected_target=""

  expected_output="$(runtime_case_expected_output "$case_name")"
  if [[ -n "$expected_output" ]] && ! grep -q "$expected_output" "$log_file"; then
    printf 'failure\t%s did not emit expected output: %s\n' "$case_name" "$expected_output"
    return
  fi

  if [[ "$case_name" == "test_comptime_target_predicates" ]]; then
    expected_target="TARGET:${platform}"
    if ! grep -q "$expected_target" "$log_file"; then
      printf 'failure\t%s did not report %s\n' "$case_name" "$expected_target"
      return
    fi
  fi

  printf 'success\tran %s\n' "$case_name"
}

init_runtime_case_results() {
  local results_file="$1"
  printf 'case\tstatus\tmessage\tlog\n' > "$results_file"
}

write_runtime_case_result() {
  local results_file="$1"
  local case_name="$2"
  local case_status="$3"
  local case_message="$4"
  local log_file="$5"
  printf '%s\t%s\t%s\t%s\n' \
    "$case_name" \
    "$case_status" \
    "$(tsv_escape "$case_message")" \
    "$(tsv_escape "$log_file")" >> "$results_file"
}

detect_ios_sign_identity() {
  if [[ -n "$IOS_SIGN_IDENTITY" ]]; then
    printf '%s' "$IOS_SIGN_IDENTITY"
    return
  fi
  security find-identity -v -p codesigning 2>/dev/null | awk -F '"' '/Apple Development|iPhone Developer/ { print $2; exit }'
}

detect_ios_device_id() {
  if [[ -n "$IOS_DEVICE_ID" ]]; then
    printf '%s' "$IOS_DEVICE_ID"
    return
  fi
  xcrun devicectl list devices --hide-headers --columns identifier 2>/dev/null | awk 'NF && $0 !~ /^No devices found/ { print $1; exit }'
}

run_macos_runtime_case() {
  local smoke_root="$1"
  local platform="$2"
  local case_name="$3"
  local log_file="$4"
  local artifact

  artifact="$(find_case_artifact "$smoke_root" "$platform" "$case_name")"
  if [[ -z "$artifact" || ! -x "$artifact" ]]; then
    printf 'failure\tmissing runtime artifact for %s\n' "$case_name"
    return
  fi

  if ! run_with_timeout_capture "$log_file" "$artifact"; then
    local case_tail
    case_tail="$(tail -n 1 "$log_file" 2>/dev/null || echo "$case_name execution failed")"
    if [[ "$platform" == "macos-x86_64" && ( "$case_tail" == *"Bad CPU type"* || "$case_tail" == *"Rosetta"* || "$case_tail" == *"rosetta"* ) ]]; then
      printf 'unavailable\tRosetta is required to run macOS x86_64 artifacts on this host\n'
      return
    fi
    printf 'failure\t%s runtime failed: %s\n' "$case_name" "$case_tail"
    return
  fi

  validate_runtime_log_output "$platform" "$case_name" "$log_file"
}

run_macos_runtime_checks() {
  local smoke_root="$1"
  local platform="$2"
  local case_name
  local case_summary
  local case_status
  local case_message
  local case_log
  local case_results_file="$REPORT_DIR/runtime-${platform}-cases.tsv"
  local first_log="__EMPTY__"
  local second_log="__EMPTY__"
  local case_count=0

  init_runtime_case_results "$case_results_file"

  for case_name in "${APPLE_RUNTIME_CASES[@]}"; do
    case_log="$REPORT_DIR/runtime-${platform}-${case_name}.log"
    if [[ "$case_count" -eq 0 ]]; then
      first_log="$case_log"
    elif [[ "$case_count" -eq 1 ]]; then
      second_log="$case_log"
    fi

    case_summary="$(run_macos_runtime_case "$smoke_root" "$platform" "$case_name" "$case_log")"
    IFS=$'\t' read -r case_status case_message <<< "$case_summary"
    write_runtime_case_result "$case_results_file" "$case_name" "$case_status" "$case_message" "$case_log"
    if [[ "$case_status" != "success" ]]; then
      printf '%s\t%s\t%s\t%s\t%s\n' "$case_status" "$case_message" "$first_log" "$second_log" "$case_results_file"
      return
    fi
    case_count=$((case_count + 1))
  done

  printf 'success\tran %d Apple runtime cases\t%s\t%s\t%s\n' "$case_count" "$first_log" "$second_log" "$case_results_file"
}

find_ios_simulator_device() {
  local booted_device
  booted_device="$(xcrun simctl list devices booted 2>/dev/null | awk -F '[()]' '/iPhone/ { print $2; exit }')"
  if [[ -n "$booted_device" ]]; then
    printf '%s' "$booted_device"
    return
  fi
  xcrun simctl list devices available 2>/dev/null | awk -F '[()]' '/iPhone/ { print $2; exit }'
}

create_ios_app_bundle() {
  local artifact="$1"
  local app_dir="$2"
  local app_name="$3"
  local bundle_id="$4"
  local platform_kind="$5"
  local plist="$app_dir/Info.plist"
  local platform_name="iphonesimulator"
  local supported_platform="iPhoneSimulator"

  if [[ "$platform_kind" == "device" ]]; then
    platform_name="iphoneos"
    supported_platform="iPhoneOS"
  fi

  rm -rf "$app_dir"
  mkdir -p "$app_dir" || return 1
  cp "$artifact" "$app_dir/$app_name" || return 1
  chmod +x "$app_dir/$app_name" || return 1

  /usr/libexec/PlistBuddy -c "Add :CFBundleExecutable string $app_name" "$plist" >/dev/null 2>&1 || return 1
  /usr/libexec/PlistBuddy -c "Add :CFBundleIdentifier string $bundle_id" "$plist" >/dev/null 2>&1 || return 1
  /usr/libexec/PlistBuddy -c "Add :CFBundleName string $app_name" "$plist" >/dev/null 2>&1 || return 1
  /usr/libexec/PlistBuddy -c "Add :CFBundlePackageType string APPL" "$plist" >/dev/null 2>&1 || return 1
  /usr/libexec/PlistBuddy -c "Add :CFBundleShortVersionString string 1.0" "$plist" >/dev/null 2>&1 || return 1
  /usr/libexec/PlistBuddy -c "Add :CFBundleVersion string 1.0" "$plist" >/dev/null 2>&1 || return 1
  /usr/libexec/PlistBuddy -c "Add :MinimumOSVersion string 17.0" "$plist" >/dev/null 2>&1 || return 1
  /usr/libexec/PlistBuddy -c "Add :DTPlatformName string $platform_name" "$plist" >/dev/null 2>&1 || return 1
  /usr/libexec/PlistBuddy -c "Add :CFBundleSupportedPlatforms array" "$plist" >/dev/null 2>&1 || return 1
  /usr/libexec/PlistBuddy -c "Add :CFBundleSupportedPlatforms:0 string $supported_platform" "$plist" >/dev/null 2>&1 || return 1
  /usr/libexec/PlistBuddy -c "Add :UIDeviceFamily array" "$plist" >/dev/null 2>&1 || return 1
  /usr/libexec/PlistBuddy -c "Add :UIDeviceFamily:0 integer 1" "$plist" >/dev/null 2>&1 || return 1
  /usr/libexec/PlistBuddy -c "Add :UIDeviceFamily:1 integer 2" "$plist" >/dev/null 2>&1 || return 1

  if [[ "$platform_kind" == "device" ]]; then
    /usr/libexec/PlistBuddy -c "Add :UIRequiredDeviceCapabilities array" "$plist" >/dev/null 2>&1 || return 1
    /usr/libexec/PlistBuddy -c "Add :UIRequiredDeviceCapabilities:0 string arm64" "$plist" >/dev/null 2>&1 || return 1
  fi
}

run_ios_simulator_case() {
  local smoke_root="$1"
  local platform="$2"
  local case_name="$3"
  local device_id="$4"
  local log_file="$5"
  local artifact
  local app_name
  local bundle_id
  local app_dir="$REPORT_DIR/runtime-${platform}-${case_name}.app"

  artifact="$(find_case_artifact "$smoke_root" "$platform" "$case_name")"
  if [[ -z "$artifact" || ! -x "$artifact" ]]; then
    printf 'failure\tmissing runtime artifact for %s\n' "$case_name"
    return
  fi

  app_name="$(runtime_case_app_name "$case_name")"
  bundle_id="$(runtime_case_bundle_id "$platform" "$case_name")"
  if ! create_ios_app_bundle "$artifact" "$app_dir" "$app_name" "$bundle_id" "simulator"; then
    printf 'failure\tfailed to create simulator app for %s\n' "$case_name"
    return
  fi

  xcrun simctl uninstall "$device_id" "$bundle_id" >/dev/null 2>&1 || true
  if ! xcrun simctl install "$device_id" "$app_dir" >/dev/null 2>&1; then
    xcrun simctl uninstall "$device_id" "$bundle_id" >/dev/null 2>&1 || true
    printf 'failure\tfailed to install %s into the iOS simulator\n' "$case_name"
    return
  fi
  if ! run_with_timeout_capture "$log_file" xcrun simctl launch --console "$device_id" "$bundle_id"; then
    local validation_summary
    local validation_status
    local validation_message
    local launch_tail
    validation_summary="$(validate_runtime_log_output "$platform" "$case_name" "$log_file")"
    IFS=$'\t' read -r validation_status validation_message <<< "$validation_summary"
    if [[ "$validation_status" == "success" ]]; then
      xcrun simctl uninstall "$device_id" "$bundle_id" >/dev/null 2>&1 || true
      printf '%s\t%s\n' "$validation_status" "$validation_message"
      return
    fi
    launch_tail="$(tail -n 1 "$log_file" 2>/dev/null || echo "$case_name execution failed")"
    xcrun simctl uninstall "$device_id" "$bundle_id" >/dev/null 2>&1 || true
    printf 'failure\t%s runtime failed: %s\n' "$case_name" "$launch_tail"
    return
  fi

  xcrun simctl uninstall "$device_id" "$bundle_id" >/dev/null 2>&1 || true
  validate_runtime_log_output "$platform" "$case_name" "$log_file"
}

run_ios_simulator_runtime_checks() {
  local smoke_root="$1"
  local platform="$2"
  local device_id
  local case_name
  local case_summary
  local case_status
  local case_message
  local case_log
  local case_results_file="$REPORT_DIR/runtime-${platform}-cases.tsv"
  local first_log="__EMPTY__"
  local second_log="__EMPTY__"
  local case_count=0

  init_runtime_case_results "$case_results_file"

  device_id="$(find_ios_simulator_device)"
  if [[ -z "$device_id" ]]; then
    printf 'unavailable\tno available iPhone simulator device was found\t%s\t%s\t%s\n' "$first_log" "$second_log" "$case_results_file"
    return
  fi

  xcrun simctl boot "$device_id" >/dev/null 2>&1 || true
  if ! xcrun simctl bootstatus "$device_id" -b >/dev/null 2>&1; then
    printf 'unavailable\tfailed to boot an iPhone simulator for runtime checks\t%s\t%s\t%s\n' "$first_log" "$second_log" "$case_results_file"
    return
  fi

  for case_name in "${APPLE_RUNTIME_CASES[@]}"; do
    case_log="$REPORT_DIR/runtime-${platform}-${case_name}.log"
    if [[ "$case_count" -eq 0 ]]; then
      first_log="$case_log"
    elif [[ "$case_count" -eq 1 ]]; then
      second_log="$case_log"
    fi

    case_summary="$(run_ios_simulator_case "$smoke_root" "$platform" "$case_name" "$device_id" "$case_log")"
    IFS=$'\t' read -r case_status case_message <<< "$case_summary"
    write_runtime_case_result "$case_results_file" "$case_name" "$case_status" "$case_message" "$case_log"
    if [[ "$case_status" != "success" ]]; then
      printf '%s\t%s\t%s\t%s\t%s\n' "$case_status" "$case_message" "$first_log" "$second_log" "$case_results_file"
      return
    fi
    case_count=$((case_count + 1))
  done

  printf 'success\tran %d Apple runtime cases in the iOS simulator\t%s\t%s\t%s\n' "$case_count" "$first_log" "$second_log" "$case_results_file"
}

run_ios_device_case() {
  local smoke_root="$1"
  local platform="$2"
  local case_name="$3"
  local device_id="$4"
  local sign_identity="$5"
  local log_file="$6"
  local artifact
  local app_name
  local bundle_id
  local app_dir="$REPORT_DIR/runtime-${platform}-${case_name}.app"
  local -a sign_cmd

  artifact="$(find_case_artifact "$smoke_root" "$platform" "$case_name")"
  if [[ -z "$artifact" || ! -x "$artifact" ]]; then
    printf 'failure\tmissing runtime artifact for %s\n' "$case_name"
    return
  fi

  app_name="$(runtime_case_app_name "$case_name")"
  bundle_id="$(runtime_case_bundle_id "$platform" "$case_name")"
  : > "$log_file"
  if ! create_ios_app_bundle "$artifact" "$app_dir" "$app_name" "$bundle_id" "device" >>"$log_file" 2>&1; then
    printf 'failure\tfailed to create device app for %s\n' "$case_name"
    return
  fi
  if ! cp "$IOS_PROVISIONING_PROFILE" "$app_dir/embedded.mobileprovision" >>"$log_file" 2>&1; then
    printf 'failure\tfailed to embed provisioning profile for %s\n' "$case_name"
    return
  fi

  sign_cmd=(codesign --force --deep --sign "$sign_identity")
  if [[ -n "$IOS_ENTITLEMENTS" ]]; then
    sign_cmd+=(--entitlements "$IOS_ENTITLEMENTS")
  fi
  sign_cmd+=("$app_dir")
  if ! "${sign_cmd[@]}" >>"$log_file" 2>&1; then
    printf 'failure\tfailed to sign %s for ios-arm64 runtime execution\n' "$case_name"
    return
  fi

  xcrun devicectl device uninstall app --device "$device_id" "$bundle_id" >>"$log_file" 2>&1 || true
  if ! xcrun devicectl device install app --device "$device_id" "$app_dir" >>"$log_file" 2>&1; then
    xcrun devicectl device uninstall app --device "$device_id" "$bundle_id" >>"$log_file" 2>&1 || true
    printf 'failure\tfailed to install %s onto ios-arm64 device %s\n' "$case_name" "$device_id"
    return
  fi
  if ! run_with_timeout_capture_append "$log_file" xcrun devicectl device process launch --console --terminate-existing --device "$device_id" "$bundle_id"; then
    local launch_tail
    launch_tail="$(tail -n 1 "$log_file" 2>/dev/null || echo "$case_name execution failed")"
    xcrun devicectl device uninstall app --device "$device_id" "$bundle_id" >>"$log_file" 2>&1 || true
    printf 'failure\t%s runtime failed on ios-arm64 device %s: %s\n' "$case_name" "$device_id" "$launch_tail"
    return
  fi

  xcrun devicectl device uninstall app --device "$device_id" "$bundle_id" >>"$log_file" 2>&1 || true
  validate_runtime_log_output "$platform" "$case_name" "$log_file"
}

run_ios_device_runtime_checks() {
  local smoke_root="$1"
  local platform="$2"
  local device_id=""
  local sign_identity=""
  local case_name
  local case_summary
  local case_status
  local case_message
  local case_log
  local case_results_file="$REPORT_DIR/runtime-${platform}-cases.tsv"
  local first_log="__EMPTY__"
  local second_log="__EMPTY__"
  local case_count=0

  init_runtime_case_results "$case_results_file"

  if ! command -v xcrun >/dev/null 2>&1; then
    printf 'unavailable\txcrun is required for ios-arm64 device runtime checks\t%s\t%s\t%s\n' "$first_log" "$second_log" "$case_results_file"
    return
  fi
  if ! xcrun devicectl --version >/dev/null 2>&1; then
    printf 'unavailable\tdevicectl is required for ios-arm64 device runtime checks\t%s\t%s\t%s\n' "$first_log" "$second_log" "$case_results_file"
    return
  fi

  device_id="$(detect_ios_device_id)"
  if [[ -z "$device_id" ]]; then
    printf 'unavailable\tno connected iOS device was found; pass --ios-device to target a provisioned device explicitly\t%s\t%s\t%s\n' "$first_log" "$second_log" "$case_results_file"
    return
  fi

  if [[ -z "$IOS_PROVISIONING_PROFILE" ]]; then
    printf 'unavailable\tios-arm64 runtime checks require --ios-provisioning-profile for signed device apps\t%s\t%s\t%s\n' "$first_log" "$second_log" "$case_results_file"
    return
  fi
  if [[ ! -f "$IOS_PROVISIONING_PROFILE" ]]; then
    printf 'unavailable\tios-arm64 provisioning profile not found: %s\t%s\t%s\t%s\n' "$IOS_PROVISIONING_PROFILE" "$first_log" "$second_log" "$case_results_file"
    return
  fi
  if [[ -n "$IOS_ENTITLEMENTS" && ! -f "$IOS_ENTITLEMENTS" ]]; then
    printf 'unavailable\tios-arm64 entitlements file not found: %s\t%s\t%s\t%s\n' "$IOS_ENTITLEMENTS" "$first_log" "$second_log" "$case_results_file"
    return
  fi

  sign_identity="$(detect_ios_sign_identity)"
  if [[ -z "$sign_identity" ]]; then
    printf 'unavailable\tno valid Apple Development signing identity was found for ios-arm64 runtime checks\t%s\t%s\t%s\n' "$first_log" "$second_log" "$case_results_file"
    return
  fi

  for case_name in "${APPLE_RUNTIME_CASES[@]}"; do
    case_log="$REPORT_DIR/runtime-${platform}-${case_name}.log"
    if [[ "$case_count" -eq 0 ]]; then
      first_log="$case_log"
    elif [[ "$case_count" -eq 1 ]]; then
      second_log="$case_log"
    fi

    case_summary="$(run_ios_device_case "$smoke_root" "$platform" "$case_name" "$device_id" "$sign_identity" "$case_log")"
    IFS=$'\t' read -r case_status case_message <<< "$case_summary"
    write_runtime_case_result "$case_results_file" "$case_name" "$case_status" "$case_message" "$case_log"
    if [[ "$case_status" != "success" ]]; then
      printf '%s\t%s\t%s\t%s\t%s\n' "$case_status" "$case_message" "$first_log" "$second_log" "$case_results_file"
      return
    fi
    case_count=$((case_count + 1))
  done

  printf 'success\tran %d Apple runtime cases on ios-arm64 device %s\t%s\t%s\t%s\n' "$case_count" "$device_id" "$first_log" "$second_log" "$case_results_file"
}

run_native_smoke_report() {
  local platform="$1"
  local report="$2"
  local smoke_dir="$REPORT_DIR/native-smoke-$platform"
  local smoke_status="failure"
  local smoke_artifact=""
  local smoke_note=""
  local smoke_log=""
  local runtime_status="skipped"
  local runtime_message=""
  local runtime_hello_log=""
  local runtime_comptime_log=""
  local runtime_case_results=""
  local platform_status="success"
  local platform_message=""

  mkdir -p "$smoke_dir"
  if bash "$SMOKE_HARNESS" --compiler "$STAGE3_BIN" --output-dir "$smoke_dir" --target "$platform" >/tmp/platform_matrix_${platform}.log 2>&1; then
    :
  fi
  local smoke_summary
  smoke_summary="$(parse_smoke_summary "$smoke_dir" "$platform")"
  IFS=$'\t' read -r smoke_status smoke_artifact smoke_note <<< "$smoke_summary"
  smoke_log="$(find_smoke_log "$smoke_dir" "$platform")"
  if [[ -z "$smoke_log" ]]; then
    smoke_log="$smoke_dir/$platform/build.log"
  fi

  if [[ "$smoke_status" == "failure" ]]; then
    platform_status="failure"
    runtime_status="skipped"
    runtime_message="Runtime checks skipped because compile smoke failed"
    platform_message="$smoke_note"
  elif [[ "$smoke_status" == "unavailable" ]]; then
    platform_status="unavailable"
    runtime_status="skipped"
    runtime_message="Runtime checks skipped because compile smoke was unavailable"
    platform_message="$smoke_note"
  elif [[ "$RUN_RUNTIME" -eq 1 ]]; then
    case "$platform" in
      macos-*)
        if [[ "$(uname -s)" != "Darwin" ]]; then
          runtime_status="unavailable"
          runtime_message="macOS runtime checks require a Darwin host"
          platform_status="unavailable"
          platform_message="$runtime_message"
        else
          local runtime_summary
          runtime_summary="$(run_macos_runtime_checks "$smoke_dir" "$platform")"
          IFS=$'\t' read -r runtime_status runtime_message runtime_hello_log runtime_comptime_log runtime_case_results <<< "$runtime_summary"
          runtime_hello_log="$(normalize_runtime_field "$runtime_hello_log")"
          runtime_comptime_log="$(normalize_runtime_field "$runtime_comptime_log")"
          runtime_case_results="$(normalize_runtime_field "$runtime_case_results")"
          if [[ "$runtime_status" == "failure" ]]; then
            platform_status="failure"
            platform_message="$runtime_message"
          elif [[ "$runtime_status" == "unavailable" ]]; then
            platform_status="unavailable"
            platform_message="$runtime_message"
          else
            platform_status="success"
            platform_message="$smoke_note"
          fi
        fi
        ;;
      ios-sim-*)
        if [[ "$(uname -s)" != "Darwin" ]]; then
          runtime_status="unavailable"
          runtime_message="iOS simulator runtime checks require a Darwin host"
          platform_status="unavailable"
          platform_message="$runtime_message"
        elif ! command -v xcrun >/dev/null 2>&1; then
          runtime_status="unavailable"
          runtime_message="xcrun is required for iOS simulator runtime checks"
          platform_status="unavailable"
          platform_message="$runtime_message"
        else
          local runtime_summary
          runtime_summary="$(run_ios_simulator_runtime_checks "$smoke_dir" "$platform")"
          IFS=$'\t' read -r runtime_status runtime_message runtime_hello_log runtime_comptime_log runtime_case_results <<< "$runtime_summary"
          runtime_hello_log="$(normalize_runtime_field "$runtime_hello_log")"
          runtime_comptime_log="$(normalize_runtime_field "$runtime_comptime_log")"
          runtime_case_results="$(normalize_runtime_field "$runtime_case_results")"
          if [[ "$runtime_status" == "failure" ]]; then
            platform_status="failure"
            platform_message="$runtime_message"
          elif [[ "$runtime_status" == "unavailable" ]]; then
            platform_status="unavailable"
            platform_message="$runtime_message"
          else
            platform_status="success"
            platform_message="$smoke_note"
          fi
        fi
        ;;
      ios-*)
        if [[ "$(uname -s)" != "Darwin" ]]; then
          runtime_status="unavailable"
          runtime_message="ios-arm64 device runtime checks require a Darwin host"
          platform_status="unavailable"
          platform_message="$runtime_message"
        else
          local runtime_summary
          runtime_summary="$(run_ios_device_runtime_checks "$smoke_dir" "$platform")"
          IFS=$'\t' read -r runtime_status runtime_message runtime_hello_log runtime_comptime_log runtime_case_results <<< "$runtime_summary"
          runtime_hello_log="$(normalize_runtime_field "$runtime_hello_log")"
          runtime_comptime_log="$(normalize_runtime_field "$runtime_comptime_log")"
          runtime_case_results="$(normalize_runtime_field "$runtime_case_results")"
          if [[ "$runtime_status" == "failure" ]]; then
            platform_status="failure"
            platform_message="$runtime_message"
          elif [[ "$runtime_status" == "unavailable" ]]; then
            platform_status="unavailable"
            platform_message="$runtime_message"
          else
            platform_status="success"
            platform_message="$smoke_note"
          fi
        fi
        ;;
      windows-*)
        runtime_status="skipped"
        runtime_message="Windows runtime checks are not available on this host"
        platform_status="success"
        platform_message="$smoke_note"
        ;;
      android-*)
        runtime_status="skipped"
        runtime_message="Android runtime checks are not wired into platform_matrix yet"
        platform_status="success"
        platform_message="$smoke_note"
        ;;
      *)
        runtime_status="skipped"
        runtime_message="Runtime checks are not implemented for this platform"
        platform_status="success"
        platform_message="$smoke_note"
        ;;
    esac
  else
    runtime_status="skipped"
    runtime_message="Runtime checks skipped; pass --with-runtime to execute supported runtime lanes"
    platform_status="success"
    platform_message="$smoke_note"
  fi

  cat > "$report" <<JSON
{
  "platform": "$platform",
  "status": "$platform_status",
  "stage3": "${STAGE3_BIN}",
  "smoke_status": "$smoke_status",
  "smoke_artifact": "$(json_escape "$smoke_artifact")",
  "smoke_note": "$(json_escape "$smoke_note")",
  "smoke_log": "$(json_escape "$smoke_log")",
  "runtime_status": "$runtime_status",
  "runtime_message": "$(json_escape "$runtime_message")",
  "runtime_hello_log": "$(json_escape "$runtime_hello_log")",
  "runtime_comptime_log": "$(json_escape "$runtime_comptime_log")",
  "runtime_case_results": "$(json_escape "$runtime_case_results")",
  "message": "$(json_escape "$platform_message")"
}
JSON
}

for platform in "${TARGET_PLATFORMS[@]}"; do
  report="$REPORT_DIR/${platform}.json"
  echo "[matrix] running $platform"
  case "$platform" in
    linux-*)
      run_linux "$platform" "$report"
      ;;
    windows-*|macos-*|android-*|ios-*)
      run_native_smoke_report "$platform" "$report"
      ;;
    web-*)
      cat > "$report" <<JSON
{
  "platform": "$platform",
  "status": "pending",
  "message": "Web harness not implemented yet",
  "stage3": "${STAGE3_BIN}",
  "notes": "Native rollout intentionally excludes WASM until native validation is stable"
}
JSON
      ;;
    *)
      echo "Unknown platform $platform" >&2
      exit 1
      ;;
  esac

  report_status="$(awk -F '"' '/^  "status":/ { print $4; exit }' "$report")"
  if [[ "$report_status" == "failure" ]]; then
    has_failures=1
  fi
 done

echo "Platform reports written to $REPORT_DIR"
if [[ "$has_failures" -ne 0 ]]; then
  exit 1
fi
