#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
package_android_apk.sh - build a debug-installable Android APK for a Seen source file.

Usage: scripts/package_android_apk.sh <seen_binary> <source.seen> <output.apk>

Requirements:
  - Android NDK available via ANDROID_NDK_HOME / ANDROID_NDK_ROOT or a standard Android SDK install
  - Android SDK with build-tools, platforms, and platform-tools installed
  - Java (javac)

The helper compiles the Seen source to libapp.so, generates a minimal Java
Activity plus JNI loader that calls the Seen program's exported main(), then
packages and signs a debug APK using the local Android SDK build tools.
USAGE
}

if [[ $# -lt 3 ]]; then
  usage >&2
  exit 1
fi

SEEN_BIN=$1
SOURCE=$2
OUTPUT=$3

if [[ ! -x "$SEEN_BIN" ]]; then
  echo "Seen binary not found or not executable: $SEEN_BIN" >&2
  exit 1
fi

if [[ ! -f "$SOURCE" ]]; then
  echo "Source file not found: $SOURCE" >&2
  exit 1
fi

if [[ ${OUTPUT##*.} != "apk" ]]; then
  echo "output file must end with .apk" >&2
  exit 1
fi

mkdir -p "$(dirname "$OUTPUT")"

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)
source "$SCRIPT_DIR/android_ndk_env.sh"

if ! normalize_android_ndk_env; then
  echo "Android NDK not found. Set ANDROID_NDK_HOME/ANDROID_NDK_ROOT or install it under the standard Android SDK path" >&2
  exit 1
fi

resolve_sdk_root() {
  if [[ -n "${ANDROID_SDK_ROOT:-}" && -d "${ANDROID_SDK_ROOT}" ]]; then
    printf '%s\n' "$ANDROID_SDK_ROOT"
    return
  fi
  if [[ -n "${ANDROID_HOME:-}" && -d "${ANDROID_HOME}" ]]; then
    printf '%s\n' "$ANDROID_HOME"
    return
  fi
  local sdk_from_ndk
  sdk_from_ndk=$(cd "$(dirname "$ANDROID_NDK_HOME")/.." && pwd)
  if [[ -d "$sdk_from_ndk" ]]; then
    printf '%s\n' "$sdk_from_ndk"
    return
  fi
  if [[ -d "$HOME/Android/Sdk" ]]; then
    printf '%s\n' "$HOME/Android/Sdk"
    return
  fi
  return 1
}

SDK_ROOT=$(resolve_sdk_root)

resolve_project_dir() {
  local source_dir
  source_dir=$(cd "$(dirname "$SOURCE")" && pwd)
  local dir="$source_dir"
  while true; do
    if [[ -f "$dir/AndroidManifest.xml" || -d "$dir/dex" || -d "$dir/res" || -d "$dir/assets" || -d "$dir/root" || -f "$dir/Seen.toml" ]]; then
      printf '%s\n' "$dir"
      return
    fi
    if [[ "$dir" == "/" || "$dir" == "$REPO_ROOT" ]]; then
      break
    fi
    local parent
    parent=$(dirname "$dir")
    if [[ "$parent" == "$dir" ]]; then
      break
    fi
    dir="$parent"
  done
  printf '%s\n' "$source_dir"
}

PROJECT_DIR=$(resolve_project_dir)

require_file() {
  local path="$1"
  if [[ ! -e "$path" ]]; then
    echo "Required file not found: $path" >&2
    exit 1
  fi
}

BUILD_TOOLS_DIR="$SDK_ROOT/build-tools/$(ls -1 "$SDK_ROOT/build-tools" | sort -V | tail -n 1)"
PLATFORM_DIR="$SDK_ROOT/platforms/$(ls -1 "$SDK_ROOT/platforms" | sort -V | tail -n 1)"
ANDROID_JAR="$PLATFORM_DIR/android.jar"
AAPT="$BUILD_TOOLS_DIR/aapt"
ZIPALIGN="$BUILD_TOOLS_DIR/zipalign"
APKSIGNER="$BUILD_TOOLS_DIR/apksigner"
D8="$BUILD_TOOLS_DIR/d8"
ADB="$SDK_ROOT/platform-tools/adb"
CLANG="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android24-clang"

require_file "$ANDROID_JAR"
require_file "$AAPT"
require_file "$ZIPALIGN"
require_file "$APKSIGNER"
require_file "$D8"
require_file "$CLANG"

if ! command -v javac >/dev/null 2>&1; then
  echo "javac is required to build the Android loader activity" >&2
  exit 1
fi

WORKDIR=$(mktemp -d)
trap 'rm -rf "$WORKDIR"' EXIT

BASE_DIR="$WORKDIR/base"
LIB_DIR="$BASE_DIR/lib/arm64-v8a"
ASSETS_DIR="$BASE_DIR/assets"
RES_DIR="$BASE_DIR/res"
ROOT_DIR="$BASE_DIR/root"
mkdir -p "$LIB_DIR" "$ASSETS_DIR" "$RES_DIR" "$ROOT_DIR"

copy_tree_if_exists() {
  local src=$1
  local dest=$2
  if [[ -d "$src" ]]; then
    mkdir -p "$dest"
    cp -a "$src/." "$dest/"
  fi
}

copy_tree_if_exists "$PROJECT_DIR/assets" "$ASSETS_DIR"
copy_tree_if_exists "$PROJECT_DIR/res" "$RES_DIR"
copy_tree_if_exists "$PROJECT_DIR/root" "$ROOT_DIR"
copy_tree_if_exists "$PROJECT_DIR/shaders" "$ASSETS_DIR/shaders"

MANIFEST_SRC="$PROJECT_DIR/AndroidManifest.xml"
if [[ ! -f "$MANIFEST_SRC" ]]; then
  MANIFEST_SRC="$WORKDIR/AndroidManifest.xml"
  cat > "$MANIFEST_SRC" <<'MANIFEST'
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="com.example.seenapp">
    <uses-sdk android:minSdkVersion="24" android:targetSdkVersion="34"/>
    <application android:label="Seen App" android:hasCode="true">
        <activity android:name=".MainActivity" android:exported="true">
            <intent-filter>
                <action android:name="android.intent.action.MAIN"/>
                <category android:name="android.intent.category.LAUNCHER"/>
            </intent-filter>
        </activity>
    </application>
</manifest>
MANIFEST
fi

PACKAGE_NAME=$(sed -n 's/.*package="\([^"]*\)".*/\1/p' "$MANIFEST_SRC" | head -n 1)
if [[ -z "$PACKAGE_NAME" ]]; then
  echo "Failed to extract package name from $MANIFEST_SRC" >&2
  exit 1
fi

ACTIVITY_NAME=$(sed -n 's/.*activity[^>]*android:name="\([^"]*\)".*/\1/p' "$MANIFEST_SRC" | head -n 1)
if [[ -z "$ACTIVITY_NAME" ]]; then
  ACTIVITY_NAME=".MainActivity"
fi

if [[ "$ACTIVITY_NAME" == .* ]]; then
  FULL_ACTIVITY_NAME="$PACKAGE_NAME$ACTIVITY_NAME"
else
  FULL_ACTIVITY_NAME="$ACTIVITY_NAME"
fi

ACTIVITY_CLASS_NAME=${FULL_ACTIVITY_NAME##*.}
PACKAGE_PATH=${PACKAGE_NAME//./\/}

jni_escape() {
  local value="$1"
  value=${value//_/\_1}
  value=${value//./_}
  printf '%s' "$value"
}

JNI_CLASS_NAME=$(jni_escape "$FULL_ACTIVITY_NAME")

JAVA_SRC_DIR="$WORKDIR/java-src/$PACKAGE_PATH"
JAVA_CLASSES_DIR="$WORKDIR/java-classes"
DEX_OUT_DIR="$WORKDIR/dex"
mkdir -p "$JAVA_SRC_DIR" "$JAVA_CLASSES_DIR" "$DEX_OUT_DIR"

cat > "$JAVA_SRC_DIR/$ACTIVITY_CLASS_NAME.java" <<EOF
package $PACKAGE_NAME;

import android.app.Activity;
import android.os.Bundle;
import android.util.Log;
import android.widget.TextView;

public final class $ACTIVITY_CLASS_NAME extends Activity {
  private static final String TAG = "SeenActivity";

  private static native int runSeenMain();

  @Override
  protected void onCreate(Bundle savedInstanceState) {
    super.onCreate(savedInstanceState);
    final TextView textView = new TextView(this);
    textView.setText("Running Seen...");
    setContentView(textView);
    Log.i(TAG, "Launching Seen activity for $FULL_ACTIVITY_NAME");

    try {
      System.loadLibrary("seen_android_loader");
      Log.i(TAG, "Loaded seen_android_loader");
    } catch (Throwable throwable) {
      Log.e(TAG, "Failed to load seen_android_loader", throwable);
      textView.setText("Loader error: " + throwable.getClass().getSimpleName());
      return;
    }

    new Thread(new Runnable() {
      @Override
      public void run() {
        try {
          final int exitCode = runSeenMain();
          Log.i(TAG, "Seen exit code: " + exitCode);
          runOnUiThread(new Runnable() {
            @Override
            public void run() {
              textView.setText("Seen exit code: " + exitCode);
            }
          });
        } catch (Throwable throwable) {
          Log.e(TAG, "runSeenMain failed", throwable);
          final String message = throwable.getClass().getSimpleName();
          runOnUiThread(new Runnable() {
            @Override
            public void run() {
              textView.setText("Seen runtime error: " + message);
            }
          });
        }
      }
    }).start();
  }
}
EOF

javac -source 8 -target 8 -bootclasspath "$ANDROID_JAR" -classpath "$ANDROID_JAR" -d "$JAVA_CLASSES_DIR" "$JAVA_SRC_DIR/$ACTIVITY_CLASS_NAME.java"
mapfile -t JAVA_CLASS_FILES < <(find "$JAVA_CLASSES_DIR" -name '*.class' -print | sort)
if [[ ${#JAVA_CLASS_FILES[@]} -eq 0 ]]; then
  echo "Failed to find compiled Java class files under $JAVA_CLASSES_DIR" >&2
  exit 1
fi
"$D8" --min-api 24 --output "$DEX_OUT_DIR" "${JAVA_CLASS_FILES[@]}"

if [[ ! -f "$DEX_OUT_DIR/classes.dex" ]]; then
  echo "Failed to generate classes.dex" >&2
  exit 1
fi

CLI_HELP="$($SEEN_BIN 2>&1 || true)"
CLI_SUBCOMMAND="compile"
if grep -q 'seen build <' <<< "$CLI_HELP"; then
  CLI_SUBCOMMAND="build"
fi

if [[ "$CLI_SUBCOMMAND" == "build" ]]; then
  "$SEEN_BIN" build "$SOURCE" --backend llvm --target android-arm64 --output "$LIB_DIR/libapp.so"
else
  "$SEEN_BIN" compile "$SOURCE" "$LIB_DIR/libapp.so" --backend llvm --target=android-arm64
fi

LOADER_C="$WORKDIR/seen_android_loader.c"
cat > "$LOADER_C" <<EOF
#include <jni.h>
#include <dlfcn.h>
#include <android/log.h>
#include <stdint.h>
#include <stdio.h>

typedef int (*seen_main_fn)(void);
typedef struct {
  int64_t len;
  char* data;
} SeenString;

void seen_print(SeenString message) {
  if (message.len > 0 && message.data != NULL) {
    fwrite(message.data, 1, (size_t)message.len, stdout);
  }
  fflush(stdout);
}

void seen_println(SeenString message) {
  seen_print(message);
  fputc('\n', stdout);
  fflush(stdout);
}

JNIEXPORT jint JNICALL Java_${JNI_CLASS_NAME}_runSeenMain(JNIEnv *env, jclass clazz) {
    (void) env;
    (void) clazz;

  __android_log_print(ANDROID_LOG_INFO, "SeenLoader", "runSeenMain invoked");

  void *self_handle = dlopen("libseen_android_loader.so", RTLD_NOW | RTLD_GLOBAL | RTLD_NOLOAD);
  if (!self_handle) {
    __android_log_print(ANDROID_LOG_ERROR, "SeenLoader", "dlopen(self) failed: %s", dlerror());
    return -1000;
  }

    void *handle = dlopen("libapp.so", RTLD_NOW | RTLD_GLOBAL);
    if (!handle) {
        __android_log_print(ANDROID_LOG_ERROR, "SeenLoader", "dlopen failed: %s", dlerror());
        return -1001;
    }

  __android_log_print(ANDROID_LOG_INFO, "SeenLoader", "libapp.so loaded");

    seen_main_fn entry = (seen_main_fn) dlsym(handle, "main");
    if (!entry) {
        __android_log_print(ANDROID_LOG_ERROR, "SeenLoader", "dlsym(main) failed: %s", dlerror());
        dlclose(handle);
        return -1002;
    }

  __android_log_print(ANDROID_LOG_INFO, "SeenLoader", "main symbol resolved");

    return entry();
}
EOF

"$CLANG" \
  -shared \
  -fPIC \
  -I"$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/include" \
  "$LOADER_C" \
  -o "$LIB_DIR/libseen_android_loader.so" \
  -llog \
  -ldl

UNSIGNED_APK="$WORKDIR/unsigned.apk"
ALIGNED_APK="$WORKDIR/aligned.apk"

"$AAPT" package \
  -f \
  -M "$MANIFEST_SRC" \
  -S "$RES_DIR" \
  -A "$ASSETS_DIR" \
  -I "$ANDROID_JAR" \
  -F "$UNSIGNED_APK"

cp "$DEX_OUT_DIR/classes.dex" "$BASE_DIR/classes.dex"

(
  cd "$BASE_DIR"
  zip -q "$UNSIGNED_APK" classes.dex
  zip -0qr "$UNSIGNED_APK" lib
  if find root -mindepth 1 -print -quit | grep -q .; then
    zip -qr "$UNSIGNED_APK" root
  fi
)

"$ZIPALIGN" -P 16 -f 4 "$UNSIGNED_APK" "$ALIGNED_APK"

DEBUG_KEYSTORE="$HOME/.android/debug.keystore"
if [[ ! -f "$DEBUG_KEYSTORE" ]]; then
  echo "Debug keystore not found: $DEBUG_KEYSTORE" >&2
  exit 1
fi

"$APKSIGNER" sign \
  --ks "$DEBUG_KEYSTORE" \
  --ks-key-alias androiddebugkey \
  --ks-pass pass:android \
  --key-pass pass:android \
  --out "$OUTPUT" \
  "$ALIGNED_APK"

echo "Created Android APK at $OUTPUT"
echo "Package: $PACKAGE_NAME"
echo "Activity: $FULL_ACTIVITY_NAME"
