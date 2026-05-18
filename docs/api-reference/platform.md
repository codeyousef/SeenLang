# Platform Modules

Platform modules expose OS/window/audio/input bindings used by higher-level
framework and graphics code.

| Platform | Modules |
|----------|---------|
| Darwin | `platform/darwin/cocoa`, `platform/darwin/coreaudio`, `platform/darwin/gamecontroller`, `platform/darwin/metal`, `platform/darwin/sdl3`, `platform/darwin/window` |
| Linux | `platform/linux/alsa`, `platform/linux/evdev`, `platform/linux/libinput`, `platform/linux/pipewire`, `platform/linux/sdl3`, `platform/linux/steam`, `platform/linux/steam_wrapper`, `platform/linux/vulkan`, `platform/linux/wayland`, `platform/linux/window`, `platform/linux/x11` |
| Web | `platform/web/webgpu` |
| Windows | `platform/windows/win32`, `platform/windows/xinput` |

Notable wrappers include `CocoaWindow`, `WindowConfig`, `WindowEvent`,
`AlsaPcm`, `EvdevDevice`, `KeyboardEvent`, `PointerMotionEvent`,
`PointerButtonEvent`, `SteamApp`, and platform SDL extern bindings.

Compilation targets are documented separately in
[Compilation Targets](../targets.md). Linux platform APIs are shared by
`linux-x86_64`, `linux-arm64`, and `linux-riscv64`; RISC-V runtime feature
reporting intentionally exposes scalar SIMD fallback behavior until RVV support
is added.
