/*
 * Seen Platform Shim - Thin C wrapper for platform libraries
 *
 * This provides simplified C functions that Seen can link to directly.
 * Compile with:
 *   cc -shared -fPIC -o libseen_platform.so seen_platform_shim.c \
 *      $(pkg-config --cflags --libs sdl3 vulkan libpipewire-0.3 alsa libevdev libinput)
 *
 * Or for static linking:
 *   cc -c -fPIC seen_platform_shim.c -o seen_platform_shim.o
 */

#define _GNU_SOURCE
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

/* ============================================================================
 * SDL3 Shim
 * ============================================================================ */
#ifdef SEEN_USE_SDL3
#include <SDL3/SDL.h>
#include <SDL3/SDL_vulkan.h>

// Event buffer for simplified event handling
static SDL_Event g_event_buffer;

int32_t seen_sdl_init(uint32_t flags) {
    return SDL_Init(flags) ? 0 : -1;
}

void seen_sdl_quit(void) {
    SDL_Quit();
}

const char* seen_sdl_get_error(void) {
    return SDL_GetError();
}

void* seen_sdl_create_window(const char* title, int w, int h, uint64_t flags) {
    return SDL_CreateWindow(title, w, h, flags);
}

void seen_sdl_destroy_window(void* window) {
    SDL_DestroyWindow((SDL_Window*)window);
}

void seen_sdl_get_window_size(void* window, int* w, int* h) {
    SDL_GetWindowSize((SDL_Window*)window, w, h);
}

int32_t seen_sdl_set_window_size(void* window, int w, int h) {
    return SDL_SetWindowSize((SDL_Window*)window, w, h) ? 0 : -1;
}

int32_t seen_sdl_show_window(void* window) {
    return SDL_ShowWindow((SDL_Window*)window) ? 0 : -1;
}

int32_t seen_sdl_hide_window(void* window) {
    return SDL_HideWindow((SDL_Window*)window) ? 0 : -1;
}

// Simplified event polling - returns event type, fills out parameters
int32_t seen_sdl_poll_event(int32_t* event_type, int64_t* param1, int64_t* param2) {
    if (!SDL_PollEvent(&g_event_buffer)) {
        return 0;
    }

    *event_type = g_event_buffer.type;

    switch (g_event_buffer.type) {
        case SDL_EVENT_KEY_DOWN:
        case SDL_EVENT_KEY_UP:
            *param1 = g_event_buffer.key.scancode;
            *param2 = g_event_buffer.key.key;
            break;
        case SDL_EVENT_MOUSE_MOTION:
            *param1 = (int64_t)(g_event_buffer.motion.x * 1000);
            *param2 = (int64_t)(g_event_buffer.motion.y * 1000);
            break;
        case SDL_EVENT_MOUSE_BUTTON_DOWN:
        case SDL_EVENT_MOUSE_BUTTON_UP:
            *param1 = g_event_buffer.button.button;
            *param2 = g_event_buffer.button.clicks;
            break;
        case SDL_EVENT_WINDOW_RESIZED:
            *param1 = g_event_buffer.window.data1;
            *param2 = g_event_buffer.window.data2;
            break;
        default:
            *param1 = 0;
            *param2 = 0;
            break;
    }

    return 1;
}

uint16_t seen_sdl_get_mod_state(void) {
    return SDL_GetModState();
}

uint32_t seen_sdl_get_mouse_state(float* x, float* y) {
    return SDL_GetMouseState(x, y);
}

uint64_t seen_sdl_get_ticks(void) {
    return SDL_GetTicks();
}

void seen_sdl_delay(uint32_t ms) {
    SDL_Delay(ms);
}

// Vulkan surface creation via SDL
int32_t seen_sdl_vulkan_create_surface(void* window, void* instance, uint64_t* surface) {
    VkSurfaceKHR vk_surface;
    if (SDL_Vulkan_CreateSurface((SDL_Window*)window, (VkInstance)instance, NULL, &vk_surface)) {
        *surface = (uint64_t)vk_surface;
        return 0;
    }
    return -1;
}

const char* const* seen_sdl_vulkan_get_instance_extensions(uint32_t* count) {
    return SDL_Vulkan_GetInstanceExtensions(count);
}

// Mouse capture for FPS camera
void seen_sdl_set_relative_mouse(int32_t enabled) {
    SDL_SetRelativeMouseMode(enabled ? true : false);
}

void seen_sdl_warp_mouse(void* window, int32_t x, int32_t y) {
    SDL_WarpMouseInWindow((SDL_Window*)window, (float)x, (float)y);
}

// Get drawable size (may differ from window size on HiDPI)
void seen_sdl_get_drawable_size(void* window, int32_t* w, int32_t* h) {
    SDL_GetWindowSizeInPixels((SDL_Window*)window, w, h);
}

#endif /* SEEN_USE_SDL3 */

/* ============================================================================
 * Vulkan Shim
 * ============================================================================ */
#ifdef SEEN_USE_VULKAN
#include <vulkan/vulkan.h>

// Create Vulkan instance with simplified parameters
int32_t seen_vk_create_instance(
    const char* app_name, uint32_t app_version,
    const char* engine_name, uint32_t engine_version,
    uint32_t api_version,
    const char* const* extensions, uint32_t extension_count,
    const char* const* layers, uint32_t layer_count,
    uint64_t* out_instance
) {
    VkApplicationInfo app_info = {
        .sType = VK_STRUCTURE_TYPE_APPLICATION_INFO,
        .pApplicationName = app_name,
        .applicationVersion = app_version,
        .pEngineName = engine_name,
        .engineVersion = engine_version,
        .apiVersion = api_version
    };

    VkInstanceCreateInfo create_info = {
        .sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO,
        .pApplicationInfo = &app_info,
        .enabledExtensionCount = extension_count,
        .ppEnabledExtensionNames = extensions,
        .enabledLayerCount = layer_count,
        .ppEnabledLayerNames = layers
    };

    VkInstance instance;
    VkResult result = vkCreateInstance(&create_info, NULL, &instance);
    if (result == VK_SUCCESS) {
        *out_instance = (uint64_t)instance;
    }
    return result;
}

void seen_vk_destroy_instance(uint64_t instance) {
    vkDestroyInstance((VkInstance)instance, NULL);
}

int32_t seen_vk_enumerate_physical_devices(uint64_t instance, uint32_t* count, uint64_t* devices) {
    if (devices == NULL) {
        return vkEnumeratePhysicalDevices((VkInstance)instance, count, NULL);
    }

    VkPhysicalDevice* vk_devices = malloc(*count * sizeof(VkPhysicalDevice));
    VkResult result = vkEnumeratePhysicalDevices((VkInstance)instance, count, vk_devices);

    for (uint32_t i = 0; i < *count; i++) {
        devices[i] = (uint64_t)vk_devices[i];
    }

    free(vk_devices);
    return result;
}

void seen_vk_get_physical_device_properties(
    uint64_t device,
    uint32_t* device_type,
    char* device_name,  // Must be at least 256 bytes
    uint32_t* api_version
) {
    VkPhysicalDeviceProperties props;
    vkGetPhysicalDeviceProperties((VkPhysicalDevice)device, &props);

    *device_type = props.deviceType;
    strncpy(device_name, props.deviceName, 255);
    device_name[255] = '\0';
    *api_version = props.apiVersion;
}

int32_t seen_vk_get_physical_device_queue_family_count(uint64_t device) {
    uint32_t count = 0;
    vkGetPhysicalDeviceQueueFamilyProperties((VkPhysicalDevice)device, &count, NULL);
    return count;
}

uint32_t seen_vk_get_physical_device_queue_family_flags(uint64_t device, uint32_t index) {
    uint32_t count = 0;
    vkGetPhysicalDeviceQueueFamilyProperties((VkPhysicalDevice)device, &count, NULL);

    if (index >= count) return 0;

    VkQueueFamilyProperties* props = malloc(count * sizeof(VkQueueFamilyProperties));
    vkGetPhysicalDeviceQueueFamilyProperties((VkPhysicalDevice)device, &count, props);

    uint32_t flags = props[index].queueFlags;
    free(props);
    return flags;
}

int32_t seen_vk_get_physical_device_surface_support(
    uint64_t device, uint32_t queue_family, uint64_t surface, int32_t* supported
) {
    VkBool32 support;
    VkResult result = vkGetPhysicalDeviceSurfaceSupportKHR(
        (VkPhysicalDevice)device, queue_family, (VkSurfaceKHR)surface, &support
    );
    *supported = support;
    return result;
}

int32_t seen_vk_create_device(
    uint64_t physical_device,
    uint32_t queue_family_index,
    uint32_t queue_count,
    const char* const* extensions,
    uint32_t extension_count,
    uint64_t* out_device
) {
    float* priorities = malloc(queue_count * sizeof(float));
    for (uint32_t i = 0; i < queue_count; i++) {
        priorities[i] = 1.0f;
    }

    VkDeviceQueueCreateInfo queue_info = {
        .sType = VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
        .queueFamilyIndex = queue_family_index,
        .queueCount = queue_count,
        .pQueuePriorities = priorities
    };

    VkDeviceCreateInfo create_info = {
        .sType = VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO,
        .queueCreateInfoCount = 1,
        .pQueueCreateInfos = &queue_info,
        .enabledExtensionCount = extension_count,
        .ppEnabledExtensionNames = extensions
    };

    VkDevice device;
    VkResult result = vkCreateDevice((VkPhysicalDevice)physical_device, &create_info, NULL, &device);
    free(priorities);

    if (result == VK_SUCCESS) {
        *out_device = (uint64_t)device;
    }
    return result;
}

void seen_vk_destroy_device(uint64_t device) {
    vkDestroyDevice((VkDevice)device, NULL);
}

void seen_vk_get_device_queue(uint64_t device, uint32_t family, uint32_t index, uint64_t* queue) {
    VkQueue q;
    vkGetDeviceQueue((VkDevice)device, family, index, &q);
    *queue = (uint64_t)q;
}

int32_t seen_vk_device_wait_idle(uint64_t device) {
    return vkDeviceWaitIdle((VkDevice)device);
}

void seen_vk_destroy_surface(uint64_t instance, uint64_t surface) {
    vkDestroySurfaceKHR((VkInstance)instance, (VkSurfaceKHR)surface, NULL);
}

int32_t seen_vk_get_surface_capabilities(
    uint64_t device, uint64_t surface,
    uint32_t* min_image_count, uint32_t* max_image_count,
    uint32_t* current_width, uint32_t* current_height,
    uint32_t* current_transform
) {
    VkSurfaceCapabilitiesKHR caps;
    VkResult result = vkGetPhysicalDeviceSurfaceCapabilitiesKHR(
        (VkPhysicalDevice)device, (VkSurfaceKHR)surface, &caps
    );

    if (result == VK_SUCCESS) {
        *min_image_count = caps.minImageCount;
        *max_image_count = caps.maxImageCount;
        *current_width = caps.currentExtent.width;
        *current_height = caps.currentExtent.height;
        *current_transform = caps.currentTransform;
    }
    return result;
}

int32_t seen_vk_create_swapchain(
    uint64_t device, uint64_t surface,
    uint32_t min_image_count, uint32_t format, uint32_t color_space,
    uint32_t width, uint32_t height,
    uint32_t image_usage, uint32_t present_mode,
    uint64_t old_swapchain,
    uint64_t* out_swapchain
) {
    VkSwapchainCreateInfoKHR create_info = {
        .sType = VK_STRUCTURE_TYPE_SWAPCHAIN_CREATE_INFO_KHR,
        .surface = (VkSurfaceKHR)surface,
        .minImageCount = min_image_count,
        .imageFormat = format,
        .imageColorSpace = color_space,
        .imageExtent = { width, height },
        .imageArrayLayers = 1,
        .imageUsage = image_usage,
        .imageSharingMode = VK_SHARING_MODE_EXCLUSIVE,
        .preTransform = VK_SURFACE_TRANSFORM_IDENTITY_BIT_KHR,
        .compositeAlpha = VK_COMPOSITE_ALPHA_OPAQUE_BIT_KHR,
        .presentMode = present_mode,
        .clipped = VK_TRUE,
        .oldSwapchain = (VkSwapchainKHR)old_swapchain
    };

    VkSwapchainKHR swapchain;
    VkResult result = vkCreateSwapchainKHR((VkDevice)device, &create_info, NULL, &swapchain);
    if (result == VK_SUCCESS) {
        *out_swapchain = (uint64_t)swapchain;
    }
    return result;
}

void seen_vk_destroy_swapchain(uint64_t device, uint64_t swapchain) {
    vkDestroySwapchainKHR((VkDevice)device, (VkSwapchainKHR)swapchain, NULL);
}

int32_t seen_vk_get_swapchain_images(uint64_t device, uint64_t swapchain, uint32_t* count, uint64_t* images) {
    if (images == NULL) {
        return vkGetSwapchainImagesKHR((VkDevice)device, (VkSwapchainKHR)swapchain, count, NULL);
    }

    VkImage* vk_images = malloc(*count * sizeof(VkImage));
    VkResult result = vkGetSwapchainImagesKHR((VkDevice)device, (VkSwapchainKHR)swapchain, count, vk_images);

    for (uint32_t i = 0; i < *count; i++) {
        images[i] = (uint64_t)vk_images[i];
    }

    free(vk_images);
    return result;
}

int32_t seen_vk_acquire_next_image(
    uint64_t device, uint64_t swapchain, uint64_t timeout,
    uint64_t semaphore, uint64_t fence, uint32_t* image_index
) {
    return vkAcquireNextImageKHR(
        (VkDevice)device, (VkSwapchainKHR)swapchain, timeout,
        (VkSemaphore)semaphore, (VkFence)fence, image_index
    );
}

int32_t seen_vk_create_image_view(
    uint64_t device, uint64_t image,
    uint32_t view_type, uint32_t format, uint32_t aspect_mask,
    uint64_t* out_view
) {
    VkImageViewCreateInfo create_info = {
        .sType = VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO,
        .image = (VkImage)image,
        .viewType = view_type,
        .format = format,
        .components = {
            VK_COMPONENT_SWIZZLE_IDENTITY,
            VK_COMPONENT_SWIZZLE_IDENTITY,
            VK_COMPONENT_SWIZZLE_IDENTITY,
            VK_COMPONENT_SWIZZLE_IDENTITY
        },
        .subresourceRange = {
            .aspectMask = aspect_mask,
            .baseMipLevel = 0,
            .levelCount = 1,
            .baseArrayLayer = 0,
            .layerCount = 1
        }
    };

    VkImageView view;
    VkResult result = vkCreateImageView((VkDevice)device, &create_info, NULL, &view);
    if (result == VK_SUCCESS) {
        *out_view = (uint64_t)view;
    }
    return result;
}

void seen_vk_destroy_image_view(uint64_t device, uint64_t view) {
    vkDestroyImageView((VkDevice)device, (VkImageView)view, NULL);
}

int32_t seen_vk_create_shader_module(uint64_t device, const uint32_t* code, uint32_t code_size, uint64_t* out_module) {
    VkShaderModuleCreateInfo create_info = {
        .sType = VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO,
        .codeSize = code_size,
        .pCode = code
    };

    VkShaderModule module;
    VkResult result = vkCreateShaderModule((VkDevice)device, &create_info, NULL, &module);
    if (result == VK_SUCCESS) {
        *out_module = (uint64_t)module;
    }
    return result;
}

void seen_vk_destroy_shader_module(uint64_t device, uint64_t module) {
    vkDestroyShaderModule((VkDevice)device, (VkShaderModule)module, NULL);
}

int32_t seen_vk_create_render_pass_simple(
    uint64_t device,
    uint32_t color_format,
    uint32_t final_layout,
    uint64_t* out_render_pass
) {
    VkAttachmentDescription attachment = {
        .format = color_format,
        .samples = VK_SAMPLE_COUNT_1_BIT,
        .loadOp = VK_ATTACHMENT_LOAD_OP_CLEAR,
        .storeOp = VK_ATTACHMENT_STORE_OP_STORE,
        .stencilLoadOp = VK_ATTACHMENT_LOAD_OP_DONT_CARE,
        .stencilStoreOp = VK_ATTACHMENT_STORE_OP_DONT_CARE,
        .initialLayout = VK_IMAGE_LAYOUT_UNDEFINED,
        .finalLayout = final_layout
    };

    VkAttachmentReference color_ref = {
        .attachment = 0,
        .layout = VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL
    };

    VkSubpassDescription subpass = {
        .pipelineBindPoint = VK_PIPELINE_BIND_POINT_GRAPHICS,
        .colorAttachmentCount = 1,
        .pColorAttachments = &color_ref
    };

    VkSubpassDependency dependency = {
        .srcSubpass = VK_SUBPASS_EXTERNAL,
        .dstSubpass = 0,
        .srcStageMask = VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT,
        .srcAccessMask = 0,
        .dstStageMask = VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT,
        .dstAccessMask = VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT
    };

    VkRenderPassCreateInfo create_info = {
        .sType = VK_STRUCTURE_TYPE_RENDER_PASS_CREATE_INFO,
        .attachmentCount = 1,
        .pAttachments = &attachment,
        .subpassCount = 1,
        .pSubpasses = &subpass,
        .dependencyCount = 1,
        .pDependencies = &dependency
    };

    VkRenderPass render_pass;
    VkResult result = vkCreateRenderPass((VkDevice)device, &create_info, NULL, &render_pass);
    if (result == VK_SUCCESS) {
        *out_render_pass = (uint64_t)render_pass;
    }
    return result;
}

void seen_vk_destroy_render_pass(uint64_t device, uint64_t render_pass) {
    vkDestroyRenderPass((VkDevice)device, (VkRenderPass)render_pass, NULL);
}

int32_t seen_vk_create_framebuffer(
    uint64_t device, uint64_t render_pass,
    uint64_t* attachments, uint32_t attachment_count,
    uint32_t width, uint32_t height,
    uint64_t* out_framebuffer
) {
    VkImageView* views = malloc(attachment_count * sizeof(VkImageView));
    for (uint32_t i = 0; i < attachment_count; i++) {
        views[i] = (VkImageView)attachments[i];
    }

    VkFramebufferCreateInfo create_info = {
        .sType = VK_STRUCTURE_TYPE_FRAMEBUFFER_CREATE_INFO,
        .renderPass = (VkRenderPass)render_pass,
        .attachmentCount = attachment_count,
        .pAttachments = views,
        .width = width,
        .height = height,
        .layers = 1
    };

    VkFramebuffer fb;
    VkResult result = vkCreateFramebuffer((VkDevice)device, &create_info, NULL, &fb);
    free(views);

    if (result == VK_SUCCESS) {
        *out_framebuffer = (uint64_t)fb;
    }
    return result;
}

void seen_vk_destroy_framebuffer(uint64_t device, uint64_t framebuffer) {
    vkDestroyFramebuffer((VkDevice)device, (VkFramebuffer)framebuffer, NULL);
}

int32_t seen_vk_create_command_pool(uint64_t device, uint32_t queue_family, uint32_t flags, uint64_t* out_pool) {
    VkCommandPoolCreateInfo create_info = {
        .sType = VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO,
        .flags = flags,
        .queueFamilyIndex = queue_family
    };

    VkCommandPool pool;
    VkResult result = vkCreateCommandPool((VkDevice)device, &create_info, NULL, &pool);
    if (result == VK_SUCCESS) {
        *out_pool = (uint64_t)pool;
    }
    return result;
}

void seen_vk_destroy_command_pool(uint64_t device, uint64_t pool) {
    vkDestroyCommandPool((VkDevice)device, (VkCommandPool)pool, NULL);
}

int32_t seen_vk_allocate_command_buffers(
    uint64_t device, uint64_t pool, uint32_t level, uint32_t count, uint64_t* buffers
) {
    VkCommandBufferAllocateInfo alloc_info = {
        .sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,
        .commandPool = (VkCommandPool)pool,
        .level = level,
        .commandBufferCount = count
    };

    VkCommandBuffer* vk_buffers = malloc(count * sizeof(VkCommandBuffer));
    VkResult result = vkAllocateCommandBuffers((VkDevice)device, &alloc_info, vk_buffers);

    for (uint32_t i = 0; i < count; i++) {
        buffers[i] = (uint64_t)vk_buffers[i];
    }

    free(vk_buffers);
    return result;
}

int32_t seen_vk_begin_command_buffer(uint64_t buffer, uint32_t flags) {
    VkCommandBufferBeginInfo begin_info = {
        .sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO,
        .flags = flags
    };
    return vkBeginCommandBuffer((VkCommandBuffer)buffer, &begin_info);
}

int32_t seen_vk_end_command_buffer(uint64_t buffer) {
    return vkEndCommandBuffer((VkCommandBuffer)buffer);
}

void seen_vk_cmd_begin_render_pass(
    uint64_t cmd, uint64_t render_pass, uint64_t framebuffer,
    int32_t x, int32_t y, uint32_t width, uint32_t height,
    float clear_r, float clear_g, float clear_b, float clear_a
) {
    VkClearValue clear_value = {
        .color = {{ clear_r, clear_g, clear_b, clear_a }}
    };

    VkRenderPassBeginInfo begin_info = {
        .sType = VK_STRUCTURE_TYPE_RENDER_PASS_BEGIN_INFO,
        .renderPass = (VkRenderPass)render_pass,
        .framebuffer = (VkFramebuffer)framebuffer,
        .renderArea = {{ x, y }, { width, height }},
        .clearValueCount = 1,
        .pClearValues = &clear_value
    };

    vkCmdBeginRenderPass((VkCommandBuffer)cmd, &begin_info, VK_SUBPASS_CONTENTS_INLINE);
}

void seen_vk_cmd_end_render_pass(uint64_t cmd) {
    vkCmdEndRenderPass((VkCommandBuffer)cmd);
}

void seen_vk_cmd_bind_pipeline(uint64_t cmd, uint32_t bind_point, uint64_t pipeline) {
    vkCmdBindPipeline((VkCommandBuffer)cmd, bind_point, (VkPipeline)pipeline);
}

void seen_vk_cmd_set_viewport(uint64_t cmd, float x, float y, float w, float h, float min_d, float max_d) {
    VkViewport viewport = { x, y, w, h, min_d, max_d };
    vkCmdSetViewport((VkCommandBuffer)cmd, 0, 1, &viewport);
}

void seen_vk_cmd_set_scissor(uint64_t cmd, int32_t x, int32_t y, uint32_t w, uint32_t h) {
    VkRect2D scissor = {{ x, y }, { w, h }};
    vkCmdSetScissor((VkCommandBuffer)cmd, 0, 1, &scissor);
}

void seen_vk_cmd_draw(uint64_t cmd, uint32_t vertex_count, uint32_t instance_count, uint32_t first_vertex, uint32_t first_instance) {
    vkCmdDraw((VkCommandBuffer)cmd, vertex_count, instance_count, first_vertex, first_instance);
}

int32_t seen_vk_create_semaphore(uint64_t device, uint64_t* out_semaphore) {
    VkSemaphoreCreateInfo create_info = {
        .sType = VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO
    };
    VkSemaphore sem;
    VkResult result = vkCreateSemaphore((VkDevice)device, &create_info, NULL, &sem);
    if (result == VK_SUCCESS) {
        *out_semaphore = (uint64_t)sem;
    }
    return result;
}

void seen_vk_destroy_semaphore(uint64_t device, uint64_t semaphore) {
    vkDestroySemaphore((VkDevice)device, (VkSemaphore)semaphore, NULL);
}

int32_t seen_vk_create_fence(uint64_t device, uint32_t flags, uint64_t* out_fence) {
    VkFenceCreateInfo create_info = {
        .sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO,
        .flags = flags
    };
    VkFence fence;
    VkResult result = vkCreateFence((VkDevice)device, &create_info, NULL, &fence);
    if (result == VK_SUCCESS) {
        *out_fence = (uint64_t)fence;
    }
    return result;
}

void seen_vk_destroy_fence(uint64_t device, uint64_t fence) {
    vkDestroyFence((VkDevice)device, (VkFence)fence, NULL);
}

int32_t seen_vk_wait_for_fences(uint64_t device, uint32_t count, uint64_t* fences, int32_t wait_all, uint64_t timeout) {
    VkFence* vk_fences = malloc(count * sizeof(VkFence));
    for (uint32_t i = 0; i < count; i++) {
        vk_fences[i] = (VkFence)fences[i];
    }
    VkResult result = vkWaitForFences((VkDevice)device, count, vk_fences, wait_all, timeout);
    free(vk_fences);
    return result;
}

int32_t seen_vk_reset_fences(uint64_t device, uint32_t count, uint64_t* fences) {
    VkFence* vk_fences = malloc(count * sizeof(VkFence));
    for (uint32_t i = 0; i < count; i++) {
        vk_fences[i] = (VkFence)fences[i];
    }
    VkResult result = vkResetFences((VkDevice)device, count, vk_fences);
    free(vk_fences);
    return result;
}

int32_t seen_vk_queue_submit(
    uint64_t queue,
    uint64_t wait_semaphore, uint32_t wait_stage,
    uint64_t command_buffer,
    uint64_t signal_semaphore,
    uint64_t fence
) {
    VkSemaphore wait_sem = (VkSemaphore)wait_semaphore;
    VkSemaphore signal_sem = (VkSemaphore)signal_semaphore;
    VkCommandBuffer cmd = (VkCommandBuffer)command_buffer;
    VkPipelineStageFlags stage = wait_stage;

    VkSubmitInfo submit_info = {
        .sType = VK_STRUCTURE_TYPE_SUBMIT_INFO,
        .waitSemaphoreCount = wait_semaphore ? 1 : 0,
        .pWaitSemaphores = wait_semaphore ? &wait_sem : NULL,
        .pWaitDstStageMask = wait_semaphore ? &stage : NULL,
        .commandBufferCount = 1,
        .pCommandBuffers = &cmd,
        .signalSemaphoreCount = signal_semaphore ? 1 : 0,
        .pSignalSemaphores = signal_semaphore ? &signal_sem : NULL
    };

    return vkQueueSubmit((VkQueue)queue, 1, &submit_info, (VkFence)fence);
}

int32_t seen_vk_queue_present(uint64_t queue, uint64_t wait_semaphore, uint64_t swapchain, uint32_t image_index) {
    VkSemaphore sem = (VkSemaphore)wait_semaphore;
    VkSwapchainKHR sc = (VkSwapchainKHR)swapchain;

    VkPresentInfoKHR present_info = {
        .sType = VK_STRUCTURE_TYPE_PRESENT_INFO_KHR,
        .waitSemaphoreCount = 1,
        .pWaitSemaphores = &sem,
        .swapchainCount = 1,
        .pSwapchains = &sc,
        .pImageIndices = &image_index
    };

    return vkQueuePresentKHR((VkQueue)queue, &present_info);
}

/* --- Descriptor Sets --- */

int64_t seen_vk_create_descriptor_set_layout(uint64_t device, int32_t binding_count, int32_t* binding_indices, int32_t* binding_types, int32_t* binding_stages) {
    VkDescriptorSetLayoutBinding* bindings = (VkDescriptorSetLayoutBinding*)calloc(binding_count, sizeof(VkDescriptorSetLayoutBinding));
    for (int i = 0; i < binding_count; i++) {
        bindings[i].binding = binding_indices[i];
        bindings[i].descriptorType = binding_types[i];
        bindings[i].descriptorCount = 1;
        bindings[i].stageFlags = binding_stages[i];
    }
    VkDescriptorSetLayoutCreateInfo ci = {
        .sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
        .bindingCount = binding_count,
        .pBindings = bindings
    };
    VkDescriptorSetLayout layout;
    VkResult r = vkCreateDescriptorSetLayout((VkDevice)device, &ci, NULL, &layout);
    free(bindings);
    return r == VK_SUCCESS ? (int64_t)layout : 0;
}

int64_t seen_vk_create_descriptor_pool(uint64_t device, int32_t max_sets, int32_t type_count, int32_t* types, int32_t* counts) {
    VkDescriptorPoolSize* sizes = (VkDescriptorPoolSize*)calloc(type_count, sizeof(VkDescriptorPoolSize));
    for (int i = 0; i < type_count; i++) {
        sizes[i].type = types[i];
        sizes[i].descriptorCount = counts[i];
    }
    VkDescriptorPoolCreateInfo ci = {
        .sType = VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO,
        .maxSets = max_sets,
        .poolSizeCount = type_count,
        .pPoolSizes = sizes
    };
    VkDescriptorPool pool;
    VkResult r = vkCreateDescriptorPool((VkDevice)device, &ci, NULL, &pool);
    free(sizes);
    return r == VK_SUCCESS ? (int64_t)pool : 0;
}

int64_t seen_vk_allocate_descriptor_set(uint64_t device, uint64_t pool, uint64_t layout) {
    VkDescriptorSetLayout l = (VkDescriptorSetLayout)layout;
    VkDescriptorSetAllocateInfo ai = {
        .sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO,
        .descriptorPool = (VkDescriptorPool)pool,
        .descriptorSetCount = 1,
        .pSetLayouts = &l
    };
    VkDescriptorSet set;
    VkResult r = vkAllocateDescriptorSets((VkDevice)device, &ai, &set);
    return r == VK_SUCCESS ? (int64_t)set : 0;
}

void seen_vk_update_descriptor_set_buffer(uint64_t device, uint64_t set, int32_t binding, uint64_t buffer, int64_t offset, int64_t range) {
    VkDescriptorBufferInfo bi = { .buffer = (VkBuffer)buffer, .offset = offset, .range = range };
    VkWriteDescriptorSet w = {
        .sType = VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET,
        .dstSet = (VkDescriptorSet)set,
        .dstBinding = binding,
        .descriptorCount = 1,
        .descriptorType = VK_DESCRIPTOR_TYPE_UNIFORM_BUFFER,
        .pBufferInfo = &bi
    };
    vkUpdateDescriptorSets((VkDevice)device, 1, &w, 0, NULL);
}

void seen_vk_update_descriptor_set_image(uint64_t device, uint64_t set, int32_t binding, uint64_t image_view, uint64_t sampler, int32_t layout) {
    VkDescriptorImageInfo ii = {
        .sampler = (VkSampler)sampler,
        .imageView = (VkImageView)image_view,
        .imageLayout = layout
    };
    VkWriteDescriptorSet w = {
        .sType = VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET,
        .dstSet = (VkDescriptorSet)set,
        .dstBinding = binding,
        .descriptorCount = 1,
        .descriptorType = VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER,
        .pImageInfo = &ii
    };
    vkUpdateDescriptorSets((VkDevice)device, 1, &w, 0, NULL);
}

void seen_vk_cmd_bind_descriptor_sets(uint64_t cmd, int32_t bind_point, uint64_t layout, int32_t first_set, uint64_t set) {
    VkDescriptorSet s = (VkDescriptorSet)set;
    vkCmdBindDescriptorSets((VkCommandBuffer)cmd, bind_point, (VkPipelineLayout)layout, first_set, 1, &s, 0, NULL);
}

/* --- Images and Samplers --- */

int64_t seen_vk_create_image(uint64_t device, int32_t width, int32_t height, int32_t format, int32_t usage, int32_t tiling) {
    VkImageCreateInfo ci = {
        .sType = VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO,
        .imageType = VK_IMAGE_TYPE_2D,
        .format = format,
        .extent = { width, height, 1 },
        .mipLevels = 1,
        .arrayLayers = 1,
        .samples = VK_SAMPLE_COUNT_1_BIT,
        .tiling = tiling,
        .usage = usage,
        .sharingMode = VK_SHARING_MODE_EXCLUSIVE,
        .initialLayout = VK_IMAGE_LAYOUT_UNDEFINED
    };
    VkImage image;
    VkResult r = vkCreateImage((VkDevice)device, &ci, NULL, &image);
    return r == VK_SUCCESS ? (int64_t)image : 0;
}

void seen_vk_destroy_image(uint64_t device, uint64_t image) {
    vkDestroyImage((VkDevice)device, (VkImage)image, NULL);
}

int64_t seen_vk_create_sampler(uint64_t device, int32_t filter, int32_t address_mode, float max_aniso) {
    VkSamplerCreateInfo ci = {
        .sType = VK_STRUCTURE_TYPE_SAMPLER_CREATE_INFO,
        .magFilter = filter,
        .minFilter = filter,
        .addressModeU = address_mode,
        .addressModeV = address_mode,
        .addressModeW = address_mode,
        .anisotropyEnable = max_aniso > 1.0f ? VK_TRUE : VK_FALSE,
        .maxAnisotropy = max_aniso,
        .borderColor = VK_BORDER_COLOR_FLOAT_OPAQUE_WHITE,
        .compareEnable = VK_FALSE,
        .mipmapMode = VK_SAMPLER_MIPMAP_MODE_LINEAR
    };
    VkSampler sampler;
    VkResult r = vkCreateSampler((VkDevice)device, &ci, NULL, &sampler);
    return r == VK_SUCCESS ? (int64_t)sampler : 0;
}

int64_t seen_vk_create_shadow_sampler(uint64_t device) {
    VkSamplerCreateInfo ci = {
        .sType = VK_STRUCTURE_TYPE_SAMPLER_CREATE_INFO,
        .magFilter = VK_FILTER_LINEAR,
        .minFilter = VK_FILTER_LINEAR,
        .addressModeU = VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_BORDER,
        .addressModeV = VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_BORDER,
        .addressModeW = VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_BORDER,
        .borderColor = VK_BORDER_COLOR_FLOAT_OPAQUE_WHITE,
        .compareEnable = VK_TRUE,
        .compareOp = VK_COMPARE_OP_LESS_OR_EQUAL,
        .mipmapMode = VK_SAMPLER_MIPMAP_MODE_LINEAR
    };
    VkSampler sampler;
    VkResult r = vkCreateSampler((VkDevice)device, &ci, NULL, &sampler);
    return r == VK_SUCCESS ? (int64_t)sampler : 0;
}

void seen_vk_destroy_sampler(uint64_t device, uint64_t sampler) {
    vkDestroySampler((VkDevice)device, (VkSampler)sampler, NULL);
}

void seen_vk_cmd_pipeline_barrier_image(uint64_t cmd, uint64_t image, int32_t old_layout, int32_t new_layout, int32_t src_stage, int32_t dst_stage) {
    VkImageMemoryBarrier barrier = {
        .sType = VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER,
        .oldLayout = old_layout,
        .newLayout = new_layout,
        .srcQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED,
        .dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED,
        .image = (VkImage)image,
        .subresourceRange = {
            .aspectMask = VK_IMAGE_ASPECT_COLOR_BIT,
            .baseMipLevel = 0,
            .levelCount = 1,
            .baseArrayLayer = 0,
            .layerCount = 1
        }
    };
    // Determine access masks from layouts
    if (old_layout == VK_IMAGE_LAYOUT_UNDEFINED) barrier.srcAccessMask = 0;
    else if (old_layout == VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL) barrier.srcAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT;
    else if (old_layout == VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL) barrier.srcAccessMask = VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT;

    if (new_layout == VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL) barrier.dstAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT;
    else if (new_layout == VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL) barrier.dstAccessMask = VK_ACCESS_SHADER_READ_BIT;
    else if (new_layout == VK_IMAGE_LAYOUT_DEPTH_STENCIL_ATTACHMENT_OPTIMAL) {
        barrier.subresourceRange.aspectMask = VK_IMAGE_ASPECT_DEPTH_BIT;
        barrier.dstAccessMask = VK_ACCESS_DEPTH_STENCIL_ATTACHMENT_READ_BIT | VK_ACCESS_DEPTH_STENCIL_ATTACHMENT_WRITE_BIT;
    }

    vkCmdPipelineBarrier((VkCommandBuffer)cmd, src_stage, dst_stage, 0, 0, NULL, 0, NULL, 1, &barrier);
}

/* --- Compute Pipelines --- */

int64_t seen_vk_create_compute_pipeline(uint64_t device, uint64_t shader_module, uint64_t layout) {
    VkComputePipelineCreateInfo ci = {
        .sType = VK_STRUCTURE_TYPE_COMPUTE_PIPELINE_CREATE_INFO,
        .stage = {
            .sType = VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
            .stage = VK_SHADER_STAGE_COMPUTE_BIT,
            .module = (VkShaderModule)shader_module,
            .pName = "main"
        },
        .layout = (VkPipelineLayout)layout
    };
    VkPipeline pipeline;
    VkResult r = vkCreateComputePipelines((VkDevice)device, VK_NULL_HANDLE, 1, &ci, NULL, &pipeline);
    return r == VK_SUCCESS ? (int64_t)pipeline : 0;
}

void seen_vk_cmd_dispatch(uint64_t cmd, int32_t gx, int32_t gy, int32_t gz) {
    vkCmdDispatch((VkCommandBuffer)cmd, gx, gy, gz);
}

void seen_vk_cmd_bind_compute_pipeline(uint64_t cmd, uint64_t pipeline) {
    vkCmdBindPipeline((VkCommandBuffer)cmd, VK_PIPELINE_BIND_POINT_COMPUTE, (VkPipeline)pipeline);
}

/* --- Push Constants --- */

void seen_vk_cmd_push_constants(uint64_t cmd, uint64_t layout, int32_t stages, int32_t offset, int32_t size, uint64_t data_ptr) {
    vkCmdPushConstants((VkCommandBuffer)cmd, (VkPipelineLayout)layout, stages, offset, size, (const void*)data_ptr);
}

/* --- Buffer Copy --- */

void seen_vk_cmd_copy_buffer(uint64_t cmd, uint64_t src, uint64_t dst, int64_t size) {
    VkBufferCopy region = { .srcOffset = 0, .dstOffset = 0, .size = size };
    vkCmdCopyBuffer((VkCommandBuffer)cmd, (VkBuffer)src, (VkBuffer)dst, 1, &region);
}

/* --- Timestamp Queries --- */

int64_t seen_vk_create_query_pool(uint64_t device, int32_t count) {
    VkQueryPoolCreateInfo ci = {
        .sType = VK_STRUCTURE_TYPE_QUERY_POOL_CREATE_INFO,
        .queryType = VK_QUERY_TYPE_TIMESTAMP,
        .queryCount = count
    };
    VkQueryPool pool;
    VkResult r = vkCreateQueryPool((VkDevice)device, &ci, NULL, &pool);
    return r == VK_SUCCESS ? (int64_t)pool : 0;
}

void seen_vk_destroy_query_pool(uint64_t device, uint64_t pool) {
    vkDestroyQueryPool((VkDevice)device, (VkQueryPool)pool, NULL);
}

void seen_vk_cmd_write_timestamp(uint64_t cmd, int32_t stage, uint64_t pool, int32_t query) {
    vkCmdWriteTimestamp((VkCommandBuffer)cmd, stage, (VkQueryPool)pool, query);
}

void seen_vk_get_query_results(uint64_t device, uint64_t pool, int32_t first, int32_t count, int64_t* results) {
    vkGetQueryPoolResults((VkDevice)device, (VkQueryPool)pool, first, count,
                          count * sizeof(uint64_t), (uint64_t*)results, sizeof(uint64_t),
                          VK_QUERY_RESULT_64_BIT | VK_QUERY_RESULT_WAIT_BIT);
}

float seen_vk_get_timestamp_period(uint64_t physical_device) {
    VkPhysicalDeviceProperties props;
    vkGetPhysicalDeviceProperties((VkPhysicalDevice)physical_device, &props);
    return props.limits.timestampPeriod;
}

/* --- Image memory --- */

int64_t seen_vk_get_image_memory_requirements(uint64_t device, uint64_t image, int64_t* size_out, int64_t* alignment_out, uint32_t* type_bits_out) {
    VkMemoryRequirements reqs;
    vkGetImageMemoryRequirements((VkDevice)device, (VkImage)image, &reqs);
    *size_out = reqs.size;
    *alignment_out = reqs.alignment;
    *type_bits_out = reqs.memoryTypeBits;
    return 0;
}

int32_t seen_vk_bind_image_memory(uint64_t device, uint64_t image, uint64_t memory, int64_t offset) {
    return vkBindImageMemory((VkDevice)device, (VkImage)image, (VkDeviceMemory)memory, offset);
}

void seen_vk_cmd_copy_buffer_to_image(uint64_t cmd, uint64_t buffer, uint64_t image, int32_t width, int32_t height) {
    VkBufferImageCopy region = {
        .bufferOffset = 0,
        .bufferRowLength = 0,
        .bufferImageHeight = 0,
        .imageSubresource = {
            .aspectMask = VK_IMAGE_ASPECT_COLOR_BIT,
            .mipLevel = 0,
            .baseArrayLayer = 0,
            .layerCount = 1
        },
        .imageOffset = { 0, 0, 0 },
        .imageExtent = { width, height, 1 }
    };
    vkCmdCopyBufferToImage((VkCommandBuffer)cmd, (VkBuffer)buffer, (VkImage)image,
                           VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, 1, &region);
}

void seen_vk_cmd_reset_query_pool(uint64_t cmd, uint64_t pool, int32_t first, int32_t count) {
    vkCmdResetQueryPool((VkCommandBuffer)cmd, (VkQueryPool)pool, first, count);
}

void seen_vk_destroy_descriptor_pool(uint64_t device, uint64_t pool) {
    vkDestroyDescriptorPool((VkDevice)device, (VkDescriptorPool)pool, NULL);
}

void seen_vk_destroy_descriptor_set_layout(uint64_t device, uint64_t layout) {
    vkDestroyDescriptorSetLayout((VkDevice)device, (VkDescriptorSetLayout)layout, NULL);
}

#endif /* SEEN_USE_VULKAN */

/* ============================================================================
 * ALSA Shim
 * ============================================================================ */
#ifdef SEEN_USE_ALSA
#include <alsa/asoundlib.h>

int32_t seen_alsa_open(void** pcm, const char* name, int32_t stream, int32_t mode) {
    return snd_pcm_open((snd_pcm_t**)pcm, name, stream, mode);
}

int32_t seen_alsa_close(void* pcm) {
    return snd_pcm_close((snd_pcm_t*)pcm);
}

int32_t seen_alsa_set_params(void* pcm, int32_t format, int32_t access, int32_t channels, int32_t rate, int32_t soft_resample, int32_t latency) {
    return snd_pcm_set_params((snd_pcm_t*)pcm, format, access, channels, rate, soft_resample, latency);
}

int64_t seen_alsa_writei(void* pcm, const void* buffer, uint64_t frames) {
    return snd_pcm_writei((snd_pcm_t*)pcm, buffer, frames);
}

int32_t seen_alsa_prepare(void* pcm) {
    return snd_pcm_prepare((snd_pcm_t*)pcm);
}

int32_t seen_alsa_drain(void* pcm) {
    return snd_pcm_drain((snd_pcm_t*)pcm);
}

int32_t seen_alsa_recover(void* pcm, int32_t err, int32_t silent) {
    return snd_pcm_recover((snd_pcm_t*)pcm, err, silent);
}

const char* seen_alsa_strerror(int32_t errnum) {
    return snd_strerror(errnum);
}

#endif /* SEEN_USE_ALSA */

/* ============================================================================
 * PipeWire Shim
 * ============================================================================ */
#ifdef SEEN_USE_PIPEWIRE
#include <pipewire/pipewire.h>
#include <spa/param/audio/format-utils.h>

void seen_pw_init(void) {
    pw_init(NULL, NULL);
}

void seen_pw_deinit(void) {
    pw_deinit();
}

const char* seen_pw_get_library_version(void) {
    return pw_get_library_version();
}

void* seen_pw_thread_loop_new(const char* name) {
    return pw_thread_loop_new(name, NULL);
}

void seen_pw_thread_loop_destroy(void* loop) {
    pw_thread_loop_destroy((struct pw_thread_loop*)loop);
}

int32_t seen_pw_thread_loop_start(void* loop) {
    return pw_thread_loop_start((struct pw_thread_loop*)loop);
}

void seen_pw_thread_loop_stop(void* loop) {
    pw_thread_loop_stop((struct pw_thread_loop*)loop);
}

void seen_pw_thread_loop_lock(void* loop) {
    pw_thread_loop_lock((struct pw_thread_loop*)loop);
}

void seen_pw_thread_loop_unlock(void* loop) {
    pw_thread_loop_unlock((struct pw_thread_loop*)loop);
}

void* seen_pw_thread_loop_get_loop(void* thread_loop) {
    return pw_thread_loop_get_loop((struct pw_thread_loop*)thread_loop);
}

#endif /* SEEN_USE_PIPEWIRE */

/* ============================================================================
 * evdev Shim
 * ============================================================================ */
#ifdef SEEN_USE_EVDEV
#include <libevdev/libevdev.h>
#include <fcntl.h>
#include <unistd.h>

void* seen_evdev_new(void) {
    return libevdev_new();
}

void seen_evdev_free(void* dev) {
    libevdev_free((struct libevdev*)dev);
}

int32_t seen_evdev_set_fd(void* dev, int32_t fd) {
    return libevdev_set_fd((struct libevdev*)dev, fd);
}

const char* seen_evdev_get_name(void* dev) {
    return libevdev_get_name((struct libevdev*)dev);
}

int32_t seen_evdev_get_id_vendor(void* dev) {
    return libevdev_get_id_vendor((struct libevdev*)dev);
}

int32_t seen_evdev_get_id_product(void* dev) {
    return libevdev_get_id_product((struct libevdev*)dev);
}

int32_t seen_evdev_has_event_type(void* dev, int32_t type) {
    return libevdev_has_event_type((struct libevdev*)dev, type);
}

int32_t seen_evdev_has_event_code(void* dev, int32_t type, int32_t code) {
    return libevdev_has_event_code((struct libevdev*)dev, type, code);
}

int32_t seen_evdev_next_event(void* dev, int32_t flags, int32_t* type, int32_t* code, int32_t* value) {
    struct input_event ev;
    int rc = libevdev_next_event((struct libevdev*)dev, flags, &ev);
    if (rc == LIBEVDEV_READ_STATUS_SUCCESS || rc == LIBEVDEV_READ_STATUS_SYNC) {
        *type = ev.type;
        *code = ev.code;
        *value = ev.value;
    }
    return rc;
}

int32_t seen_evdev_open_device(const char* path) {
    return open(path, O_RDONLY | O_NONBLOCK);
}

int32_t seen_evdev_close_device(int32_t fd) {
    return close(fd);
}

#endif /* SEEN_USE_EVDEV */
