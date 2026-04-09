#!/usr/bin/env bash

resolve_android_ndk_root() {
  if [[ -n "${ANDROID_NDK_HOME:-}" && -d "${ANDROID_NDK_HOME}" ]]; then
    printf '%s\n' "$ANDROID_NDK_HOME"
    return 0
  fi

  if [[ -n "${ANDROID_NDK_ROOT:-}" && -d "${ANDROID_NDK_ROOT}" ]]; then
    printf '%s\n' "$ANDROID_NDK_ROOT"
    return 0
  fi

  local candidate
  local latest=""
  for candidate in \
    "${ANDROID_SDK_ROOT:-}" \
    "${ANDROID_HOME:-}" \
    "$HOME/Android/Sdk" \
    "$HOME/android-sdk" \
    "/opt/android-sdk" \
    "/usr/lib/android-sdk"; do
    [[ -n "$candidate" ]] || continue

    if [[ -d "$candidate/ndk" ]]; then
      latest="$(find "$candidate/ndk" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | (sort -V 2>/dev/null || sort) | tail -n 1)"
      if [[ -n "$latest" && -d "$latest" ]]; then
        printf '%s\n' "$latest"
        return 0
      fi
    fi

    if [[ -d "$candidate/ndk-bundle" ]]; then
      printf '%s\n' "$candidate/ndk-bundle"
      return 0
    fi
  done

  return 1
}

normalize_android_ndk_env() {
  local detected="${1:-}"
  if [[ -z "$detected" ]]; then
    detected="$(resolve_android_ndk_root || true)"
  fi

  if [[ -n "$detected" && -d "$detected" ]]; then
    if [[ -z "${ANDROID_NDK_HOME:-}" ]]; then
      export ANDROID_NDK_HOME="$detected"
    fi
    if [[ -z "${ANDROID_NDK_ROOT:-}" ]]; then
      export ANDROID_NDK_ROOT="$detected"
    fi
  fi

  [[ -n "${ANDROID_NDK_HOME:-}" && -d "${ANDROID_NDK_HOME}" && -n "${ANDROID_NDK_ROOT:-}" && -d "${ANDROID_NDK_ROOT}" ]]
}
