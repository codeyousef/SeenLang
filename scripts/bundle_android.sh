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

WORKDIR=$(mktemp -d)
trap 'rm -rf "$WORKDIR"' EXIT

FILES_ROOT=$(dirname "$OUTPUT")
BUNDLE_NAME=$(basename "$OUTPUT")
BASE_DIR="$WORKDIR/base"
LIB_DIR="$BASE_DIR/lib/arm64-v8a"
MANIFEST_DIR="$BASE_DIR/manifest"
METADATA_DIR="$BASE_DIR/manifest"
mkdir -p "$LIB_DIR" "$MANIFEST_DIR" "$BASE_DIR/dex" "$BASE_DIR/assets" "$BASE_DIR/res"

# BundleConfig describes module metadata; keep minimal for now.
cat > "$WORKDIR/BundleConfig.pb" <<'CONFIG'
modules {
  name: "base"
  module_type: BUNDLE_MODULE
  assets_config {
  }
}
CONFIG

if [[ -z "${ANDROID_NDK_HOME:-}" ]]; then
  echo "ANDROID_NDK_HOME must be set" >&2
  exit 1
fi

$SEEN_BIN build "$SOURCE" --backend llvm --target aarch64-linux-android --output "$LIB_DIR/libapp.so"

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

ZIP_TMP="$WORKDIR/bundle.zip"
(cd "$WORKDIR" && zip -qr "$ZIP_TMP" BundleConfig.pb base)
mv "$ZIP_TMP" "$OUTPUT"

echo "Created Android bundle at $OUTPUT"
