#!/usr/bin/env bash
# Steam Runtime Compatibility Script for Seen Language
# Ensures AppImage works correctly within Steam Runtime environment

set -e

# Script directory (where this script is located)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Detect Steam Runtime environment
detect_steam_runtime() {
    # Check for Steam Runtime markers
    if [ -n "$STEAM_RUNTIME" ]; then
        echo "sniper"  # or soldier, based on STEAM_RUNTIME value
        return 0
    fi

    if [ -n "$SteamAppId" ]; then
        # Running as a Steam game
        echo "steam-game"
        return 0
    fi

    if [ -d "/run/host" ] && [ -f "/.flatpak-info" ]; then
        echo "flatpak"
        return 0
    fi

    if [ -d "$HOME/.steam/steam/ubuntu12_32/steam-runtime" ]; then
        echo "available"
        return 0
    fi

    echo "none"
    return 1
}

# Get Steam Runtime library paths
get_steam_runtime_paths() {
    local runtime_type="$1"
    local arch="${2:-x86_64}"

    local steam_runtime_base="$HOME/.steam/steam/ubuntu12_32/steam-runtime"

    case "$runtime_type" in
        sniper)
            # Sniper (newer) runtime paths
            echo "$HOME/.steam/steam/steamapps/common/SteamLinuxRuntime_sniper/sniper_platform_0.$(date +%Y%m%d)/files/lib/$arch-linux-gnu"
            ;;
        soldier)
            # Soldier runtime paths
            echo "$HOME/.steam/steam/steamapps/common/SteamLinuxRuntime_soldier/soldier_platform_*/files/lib/$arch-linux-gnu"
            ;;
        steam-game)
            # Running under Steam, use existing LD_LIBRARY_PATH
            echo "${LD_LIBRARY_PATH:-}"
            ;;
        *)
            # Use system paths
            echo "/usr/lib/$arch-linux-gnu:/usr/lib64:/usr/lib"
            ;;
    esac
}

# Setup Vulkan ICD paths for Steam environment
setup_vulkan_icd() {
    local app_share_dir="$1"

    # Common ICD locations
    local icd_paths=(
        "/usr/share/vulkan/icd.d"
        "/etc/vulkan/icd.d"
        "$HOME/.local/share/vulkan/icd.d"
    )

    # Add Steam-specific ICD paths if running under Steam
    if [ -n "$STEAM_RUNTIME" ] || [ -n "$SteamAppId" ]; then
        icd_paths+=(
            "$HOME/.steam/steam/steamapps/common/SteamLinuxRuntime_sniper/sniper_platform_*/files/share/vulkan/icd.d"
            "/usr/share/vulkan/icd.d"
        )
    fi

    # Build VK_ICD_FILENAMES
    local icd_files=""
    for icd_path in "${icd_paths[@]}"; do
        if [ -d "$icd_path" ]; then
            for icd_file in "$icd_path"/*.json; do
                if [ -f "$icd_file" ]; then
                    if [ -n "$icd_files" ]; then
                        icd_files="$icd_files:$icd_file"
                    else
                        icd_files="$icd_file"
                    fi
                fi
            done
        fi
    done

    # Add app-bundled ICDs first
    if [ -d "$app_share_dir/vulkan/icd.d" ]; then
        for icd_file in "$app_share_dir/vulkan/icd.d"/*.json; do
            if [ -f "$icd_file" ]; then
                icd_files="$icd_file:$icd_files"
            fi
        done
    fi

    echo "$icd_files"
}

# Setup SDL hints for Steam
setup_sdl_hints() {
    # Force X11 on Steam Deck or when overlay is needed
    if [ "${SteamDeck:-0}" = "1" ] || [ -n "$SteamAppId" ]; then
        export SDL_VIDEODRIVER="${SDL_VIDEODRIVER:-x11}"
    fi

    # Audio driver preference
    export SDL_AUDIODRIVER="${SDL_AUDIODRIVER:-pipewire,pulseaudio,alsa}"

    # Gamepad hints
    export SDL_GAMECONTROLLERCONFIG="${SDL_GAMECONTROLLERCONFIG:-}"
    export SDL_JOYSTICK_HIDAPI="${SDL_JOYSTICK_HIDAPI:-1}"
}

# Setup audio for Steam environment
setup_audio() {
    # PipeWire is preferred
    if command -v pipewire &> /dev/null || [ -S "$XDG_RUNTIME_DIR/pipewire-0" ]; then
        export PIPEWIRE_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
    fi

    # ALSA fallback config
    if [ -z "$ALSA_CONFIG_DIR" ]; then
        if [ -d "/usr/share/alsa" ]; then
            export ALSA_CONFIG_DIR="/usr/share/alsa"
        fi
    fi
}

# Setup input devices
setup_input() {
    # Ensure we can access input devices
    if [ ! -r "/dev/input/event0" ] 2>/dev/null; then
        # User might need to be in 'input' group
        echo "Warning: Cannot read /dev/input devices. Add user to 'input' group for gamepad support." >&2
    fi

    # libinput configuration
    export LIBINPUT_QUIRKS_DIR="${LIBINPUT_QUIRKS_DIR:-/usr/share/libinput}"
}

# Main setup function - call this before launching the game
setup_steam_compat() {
    local app_dir="${1:-$(dirname "$0")/..}"

    # Detect runtime
    local runtime_type
    runtime_type=$(detect_steam_runtime)

    echo "Steam Runtime: $runtime_type"

    # Setup library paths
    local runtime_paths
    runtime_paths=$(get_steam_runtime_paths "$runtime_type")

    # Add app libraries first
    if [ -d "$app_dir/lib" ]; then
        export LD_LIBRARY_PATH="$app_dir/lib:${LD_LIBRARY_PATH:-}"
    fi
    if [ -d "$app_dir/lib/seen-specific" ]; then
        export LD_LIBRARY_PATH="$app_dir/lib/seen-specific:${LD_LIBRARY_PATH}"
    fi

    # Setup Vulkan
    local vulkan_icds
    vulkan_icds=$(setup_vulkan_icd "$app_dir/share")
    if [ -n "$vulkan_icds" ]; then
        export VK_ICD_FILENAMES="$vulkan_icds"
    fi

    # Setup SDL
    setup_sdl_hints

    # Setup audio
    setup_audio

    # Setup input
    setup_input

    # Seen-specific environment
    export SEEN_LIB_PATH="${app_dir}/lib/seen"
    export SEEN_DATA_PATH="${app_dir}/share/seen"

    echo "Environment configured for Steam Runtime compatibility"
}

# Print current environment for debugging
print_env() {
    echo "=== Steam Runtime Environment ==="
    echo "STEAM_RUNTIME: ${STEAM_RUNTIME:-not set}"
    echo "SteamAppId: ${SteamAppId:-not set}"
    echo "SteamDeck: ${SteamDeck:-not set}"
    echo ""
    echo "=== Library Paths ==="
    echo "LD_LIBRARY_PATH: ${LD_LIBRARY_PATH:-not set}"
    echo ""
    echo "=== Vulkan ==="
    echo "VK_ICD_FILENAMES: ${VK_ICD_FILENAMES:-not set}"
    echo "VK_LAYER_PATH: ${VK_LAYER_PATH:-not set}"
    echo ""
    echo "=== SDL ==="
    echo "SDL_VIDEODRIVER: ${SDL_VIDEODRIVER:-not set}"
    echo "SDL_AUDIODRIVER: ${SDL_AUDIODRIVER:-not set}"
    echo ""
    echo "=== Audio ==="
    echo "PIPEWIRE_RUNTIME_DIR: ${PIPEWIRE_RUNTIME_DIR:-not set}"
    echo "ALSA_CONFIG_DIR: ${ALSA_CONFIG_DIR:-not set}"
    echo ""
    echo "=== Seen ==="
    echo "SEEN_LIB_PATH: ${SEEN_LIB_PATH:-not set}"
    echo "SEEN_DATA_PATH: ${SEEN_DATA_PATH:-not set}"
    echo "================================"
}

# If sourced, just export the functions
# If run directly, setup and optionally run a command
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    case "${1:-}" in
        --help|-h)
            echo "Usage: $0 [--setup|--print-env|--run COMMAND...]"
            echo ""
            echo "Options:"
            echo "  --setup      Setup environment and print configuration"
            echo "  --print-env  Print current environment variables"
            echo "  --run CMD    Setup environment and run command"
            echo ""
            echo "Example:"
            echo "  source $0     # Source to get functions"
            echo "  $0 --setup    # Setup and show config"
            echo "  $0 --run ./my-game  # Setup and run game"
            exit 0
            ;;
        --setup)
            setup_steam_compat "$(dirname "$0")/.."
            print_env
            ;;
        --print-env)
            print_env
            ;;
        --run)
            shift
            setup_steam_compat "$(dirname "$0")/.."
            exec "$@"
            ;;
        *)
            # Default: just setup
            setup_steam_compat "$(dirname "$0")/.."
            ;;
    esac
fi
