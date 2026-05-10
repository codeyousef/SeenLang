# Input

Modules: `input/gamepad`, `input/gamepad_unified`

Input modules define gamepad state and event surfaces.

| Type | Module | Purpose |
|------|--------|---------|
| `GamepadState` | `input/gamepad`, `input/gamepad_unified` | Current gamepad buttons/axes |
| `GamepadButtonEvent` | `input/gamepad` | Button transition |
| `GamepadAxisEvent` | `input/gamepad` | Axis motion |
| `GamepadConnectionEvent` | `input/gamepad` | Connect/disconnect event |
| `GamepadEventType` | `input/gamepad` | Event discriminant |
| `GamepadEvent` | `input/gamepad` | Unified event payload |
| `Gamepad` | `input/gamepad` | Device wrapper |
| `GamepadManager` | `input/gamepad`, `input/gamepad_unified` | Device manager |
