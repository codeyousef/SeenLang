#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 3 ]]; then
  echo "Usage: bundle_android.sh <seen_binary> <source.seen> <output .aab>" >&2
  exit 1
fi

SEEN_BIN=$1
SOURCE=$2
OUTPUT=$3
if [[ ${OUTPUT##*.} != "aab" ]]; then
  echo "output file must end with .aab" >&2
  exit 1
fi

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)

WORKDIR=$(mktemp -d)
trap 'rm -rf "$WORKDIR"' EXIT

FILES_ROOT=$(dirname "$OUTPUT")
BUNDLE_NAME=$(basename "$OUTPUT")
BASE_DIR="$WORKDIR/base"
LIB_DIR="$BASE_DIR/lib/arm64-v8a"
MANIFEST_DIR="$BASE_DIR/manifest"
mkdir -p "$LIB_DIR" "$MANIFEST_DIR" "$BASE_DIR/dex" "$BASE_DIR/assets" "$BASE_DIR/res" "$BASE_DIR/root"

if [[ -z "${ANDROID_NDK_HOME:-}" ]]; then
  echo "ANDROID_NDK_HOME must be set" >&2
  exit 1
fi

PROJECT_DIR=$(cd "$(dirname "$SOURCE")" && pwd)

ABI_DIR="arm64-v8a"
case "${ANDROID_ABI:-}" in
  arm64-v8a|armeabi-v7a|x86|x86_64)
    ABI_DIR="$ANDROID_ABI"
    ;;
  *) ;;
esac

$SEEN_BIN build "$SOURCE" --backend llvm --target aarch64-linux-android --output "$LIB_DIR/libapp.so"

if [[ "$ABI_DIR" != "arm64-v8a" ]]; then
  mkdir -p "$BASE_DIR/lib/$ABI_DIR"
  mv "$LIB_DIR/libapp.so" "$BASE_DIR/lib/$ABI_DIR/libapp.so"
  rmdir "$LIB_DIR"
  LIB_DIR="$BASE_DIR/lib/$ABI_DIR"
fi

copy_tree_if_exists() {
  local src=$1
  local dest=$2
  if [[ -d "$src" ]]; then
    mkdir -p "$dest"
    cp -a "$src/." "$dest/"
  fi
}

copy_tree_if_exists "$PROJECT_DIR/assets" "$BASE_DIR/assets"
copy_tree_if_exists "$PROJECT_DIR/res" "$BASE_DIR/res"
copy_tree_if_exists "$PROJECT_DIR/root" "$BASE_DIR/root"
copy_tree_if_exists "$PROJECT_DIR/dex" "$BASE_DIR/dex"

if [[ -f "$PROJECT_DIR/AndroidManifest.xml" ]]; then
  cp "$PROJECT_DIR/AndroidManifest.xml" "$MANIFEST_DIR/AndroidManifest.xml"
else
  cat > "$MANIFEST_DIR/AndroidManifest.xml" <<'MANIFEST'
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="com.example.seenapp">
    <application android:label="SeenApp">
        <activity android:name=".MainActivity" android:exported="true">
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>
</manifest>
MANIFEST
fi

if [[ -f "$PROJECT_DIR/resources.pb" ]]; then
  cp "$PROJECT_DIR/resources.pb" "$BASE_DIR/resources.pb"
elif [[ -f "$PROJECT_DIR/base/resources.pb" ]]; then
  mkdir -p "$BASE_DIR"
  cp "$PROJECT_DIR/base/resources.pb" "$BASE_DIR/resources.pb"
fi

if compgen -G "$BASE_DIR/dex/*.dex" > /dev/null; then
  :
else
  echo "error: no dex files found under $BASE_DIR/dex. Provide classes.dex before bundling." >&2
  exit 1
fi

if [[ -f "$PROJECT_DIR/BundleConfig.pb" ]]; then
  cp "$PROJECT_DIR/BundleConfig.pb" "$WORKDIR/BundleConfig.pb"
else
  cat > "$WORKDIR/BundleConfig.pb" <<'CONFIG'
modules {
  name: "base"
  module_type: BUNDLE_MODULE
  assets_config {}
}
CONFIG
fi

ZIP_TMP="$WORKDIR/bundle.zip"
(cd "$WORKDIR" && zip -qr "$ZIP_TMP" BundleConfig.pb base)
mv "$ZIP_TMP" "$OUTPUT"

if [[ -n "${SEEN_ANDROID_KEYSTORE:-}" ]]; then
  JARSIGNER=${SEEN_ANDROID_JARSIGNER:-jarsigner}
  CMD=("$JARSIGNER" -keystore "$SEEN_ANDROID_KEYSTORE")
  if [[ -n "${SEEN_ANDROID_KEYSTORE_TYPE:-}" ]]; then
    CMD+=(-storetype "$SEEN_ANDROID_KEYSTORE_TYPE")
  fi
  if [[ -n "${SEEN_ANDROID_KEYSTORE_PROVIDER:-}" ]]; then
    CMD+=(-provider "$SEEN_ANDROID_KEYSTORE_PROVIDER")
  fi
  if [[ -n "${SEEN_ANDROID_KEYSTORE_PASS:-}" ]]; then
    CMD+=(-storepass "$SEEN_ANDROID_KEYSTORE_PASS")
    if [[ -n "${SEEN_ANDROID_KEY_PASS:-}" ]]; then
      CMD+=(-keypass "$SEEN_ANDROID_KEY_PASS")
    else
      CMD+=(-keypass "$SEEN_ANDROID_KEYSTORE_PASS")
    fi
  elif [[ -n "${SEEN_ANDROID_KEY_PASS:-}" ]]; then
    CMD+=(-keypass "$SEEN_ANDROID_KEY_PASS")
  fi
  CMD+=(-sigalg "${SEEN_ANDROID_SIGALG:-SHA256withRSA}")
  CMD+=(-digestalg "${SEEN_ANDROID_DIGESTALG:-SHA-256}")
  if [[ -n "${SEEN_ANDROID_TIMESTAMP_URL:-}" ]]; then
    CMD+=(-tsa "$SEEN_ANDROID_TIMESTAMP_URL")
  fi
  CMD+=("$OUTPUT" "${SEEN_ANDROID_KEY_ALIAS:-seenapp}")
  "${CMD[@]}"
fi

echo "Created Android bundle at $OUTPUT"
