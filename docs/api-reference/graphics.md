# Graphics

Modules: `graphics/gpu`, `graphics/renderer`, `graphics/shader`

Graphics modules define GPU resource wrappers, renderer configuration, shader
compilation metadata, and Vulkan-oriented rendering helpers.

| Type | Module | Purpose |
|------|--------|---------|
| `GpuBuffer` | `graphics/gpu` | GPU buffer handle |
| `GpuUniform` | `graphics/gpu` | Uniform-buffer handle |
| `GpuImage` | `graphics/gpu` | Image/texture handle |
| `GpuPipeline` | `graphics/gpu` | Pipeline handle |
| `GpuFence` | `graphics/gpu` | Synchronization fence |
| `ClearColor` | `graphics/renderer` | Frame clear color |
| `RendererConfig` | `graphics/renderer` | Renderer setup |
| `FrameData` | `graphics/renderer` | Per-frame data |
| `VulkanRenderer` | `graphics/renderer` | Vulkan renderer wrapper |
| `ShaderCompiler` | `graphics/shader` | Shader compiler helper |
| `ShaderReflectionData` | `graphics/shader` | Reflected shader metadata |
| `ShaderPermutation` | `graphics/shader` | Shader feature variant |

See also [GPU](gpu.md) and [SIMD and GPU](../simd-and-gpu.md).
