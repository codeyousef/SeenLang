#!/usr/bin/env bash
# Configure and verify single-threaded auxiliary tools inside hard build scopes.
# This file is sourced by containment entry points; it performs no work itself.

seen_serial_auxiliary_error() {
    printf 'RESOURCE STOP: serial auxiliary environment: %s\n' "$*" >&2
}

seen_serial_auxiliary_config_path() {
    local artifact_root=$1
    printf '%s\n' "$artifact_root/auxiliary-limits/ripgrep.conf"
}

seen_serial_auxiliary_validate_path() {
    local repo_root=$1
    local artifact_root=$2
    local config_path parent_relative canonical_parent

    config_path=$(seen_serial_auxiliary_config_path "$artifact_root")
    case "$config_path" in
        "$repo_root"/*) ;;
        *)
            seen_serial_auxiliary_error "ripgrep configuration escaped the repository"
            return 1
            ;;
    esac
    parent_relative=${config_path%/*}
    parent_relative=${parent_relative#"$repo_root"/}
    seen_artifact_assert_safe_relative_path "$parent_relative" || return 1
    seen_artifact_assert_no_symlink_components "$repo_root" \
        "$parent_relative" || return 1
    [ -d "${config_path%/*}" ] && [ ! -L "${config_path%/*}" ] || {
        seen_serial_auxiliary_error "ripgrep configuration parent is unsafe"
        return 1
    }
    canonical_parent=$(seen_artifact_canonical_dir "${config_path%/*}") || return 1
    [ "$canonical_parent" = "${config_path%/*}" ] || {
        seen_serial_auxiliary_error "ripgrep configuration parent is not canonical"
        return 1
    }
    [ -f "$config_path" ] && [ ! -L "$config_path" ] || {
        seen_serial_auxiliary_error "ripgrep configuration is not a regular non-symlink file"
        return 1
    }
}

seen_serial_auxiliary_verify_config() {
    local config_path=$1
    local line line_count=0

    while IFS= read -r line || [ -n "$line" ]; do
        line_count=$((line_count + 1))
        [ "$line" = "--threads=1" ] || {
            seen_serial_auxiliary_error "ripgrep configuration contains an unsafe option"
            return 1
        }
    done < "$config_path"
    [ "$line_count" -eq 1 ] || {
        seen_serial_auxiliary_error "ripgrep configuration must contain exactly one option"
        return 1
    }
}

seen_serial_auxiliary_export_values() {
    local config_path=$1

    RIPGREP_CONFIG_PATH=$config_path
    RAYON_NUM_THREADS=1
    OMP_NUM_THREADS=1
    OPENBLAS_NUM_THREADS=1
    MKL_NUM_THREADS=1
    NUMEXPR_NUM_THREADS=1
    VECLIB_MAXIMUM_THREADS=1
    BLIS_NUM_THREADS=1
    GOMAXPROCS=1
    RUST_TEST_THREADS=1
    CARGO_BUILD_JOBS=1
    SEEN_LLD_THREADS=1
    SEEN_THINLTO_JOBS=1
    export RIPGREP_CONFIG_PATH RAYON_NUM_THREADS OMP_NUM_THREADS \
        OPENBLAS_NUM_THREADS MKL_NUM_THREADS NUMEXPR_NUM_THREADS \
        VECLIB_MAXIMUM_THREADS BLIS_NUM_THREADS GOMAXPROCS \
        RUST_TEST_THREADS CARGO_BUILD_JOBS SEEN_LLD_THREADS \
        SEEN_THINLTO_JOBS
}

seen_serial_auxiliary_prepare() {
    local repo_root=$1
    local artifact_root=$2
    local config_dir config_path temporary

    config_path=$(seen_serial_auxiliary_config_path "$artifact_root")
    config_dir=${config_path%/*}
    case "$config_dir" in
        "$repo_root"/*) ;;
        *)
            seen_serial_auxiliary_error "configuration directory escaped the repository"
            return 1
            ;;
    esac
    [ ! -L "$config_dir" ] || {
        seen_serial_auxiliary_error "configuration directory is a symbolic link"
        return 1
    }
    mkdir -p -- "$config_dir" || return 1
    [ ! -L "$config_path" ] || {
        seen_serial_auxiliary_error "ripgrep configuration is a symbolic link"
        return 1
    }
    if [ -e "$config_path" ] && [ ! -f "$config_path" ]; then
        seen_serial_auxiliary_error "ripgrep configuration is not a regular file"
        return 1
    fi
    temporary=$(mktemp "$config_dir/.ripgrep.conf.XXXXXX") || return 1
    if ! printf '%s\n' '--threads=1' > "$temporary"; then
        rm -f -- "$temporary"
        return 1
    fi
    if ! chmod 600 "$temporary" || ! mv -f -- "$temporary" "$config_path"; then
        rm -f -- "$temporary"
        return 1
    fi
    seen_serial_auxiliary_validate_path "$repo_root" "$artifact_root" || return 1
    seen_serial_auxiliary_verify_config "$config_path" || return 1
    seen_serial_auxiliary_export_values "$config_path"
}

seen_serial_auxiliary_verify() {
    local repo_root=$1
    local artifact_root=$2
    local config_path variable

    config_path=$(seen_serial_auxiliary_config_path "$artifact_root")
    seen_serial_auxiliary_validate_path "$repo_root" "$artifact_root" || return 1
    [ "${RIPGREP_CONFIG_PATH:-}" = "$config_path" ] || {
        seen_serial_auxiliary_error "RIPGREP_CONFIG_PATH is missing or does not match"
        return 1
    }
    seen_serial_auxiliary_verify_config "$config_path" || return 1
    for variable in \
        RAYON_NUM_THREADS OMP_NUM_THREADS OPENBLAS_NUM_THREADS \
        MKL_NUM_THREADS NUMEXPR_NUM_THREADS VECLIB_MAXIMUM_THREADS \
        BLIS_NUM_THREADS GOMAXPROCS RUST_TEST_THREADS CARGO_BUILD_JOBS \
        SEEN_LLD_THREADS SEEN_THINLTO_JOBS; do

        [ "${!variable:-}" = "1" ] || {
            seen_serial_auxiliary_error "$variable must be exactly 1"
            return 1
        }
    done
}
