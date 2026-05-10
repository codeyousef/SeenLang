# Audio

Modules: `audio/audio`, `audio/openal`

The audio modules define backend-independent device/configuration types plus an
OpenAL-facing device wrapper.

| Type | Module | Purpose |
|------|--------|---------|
| `AudioBackend` | `audio/audio` | Backend selector |
| `AudioSampleFormat` | `audio/audio` | Sample format selector |
| `AudioConfig` | `audio/audio` | Device configuration |
| `AudioDevice` | `audio/audio` | Runtime audio device handle |
| `PipeWireState` | `audio/audio` | Linux PipeWire state |
| `ALSAState` | `audio/audio` | Linux ALSA state |
| `OpenALDevice` | `audio/openal` | OpenAL-backed device handle |

Platform-specific audio bindings live under `platform/linux/*` and
`platform/darwin/*`.
