/* SDK-only Vulkan leak baseline: create and destroy an instance and the
 * device the canary uses, with no Seen code (FEL-1111). */
#include <stdio.h>
#include <vulkan/vulkan.h>

int main(void) {
    VkApplicationInfo app = {.sType = VK_STRUCTURE_TYPE_APPLICATION_INFO, .apiVersion = VK_API_VERSION_1_2};
    VkInstanceCreateInfo instance_info = {.sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO,
                                          .pApplicationInfo = &app};
    VkInstance instance;
    VkResult status = vkCreateInstance(&instance_info, NULL, &instance);
    if (status != VK_SUCCESS) {
        printf("vkCreateInstance=%d\n", (int)status);
        return 1;
    }
    uint32_t count = 1;
    VkPhysicalDevice physical = VK_NULL_HANDLE;
    status = vkEnumeratePhysicalDevices(instance, &count, &physical);
    if ((status == VK_SUCCESS || status == VK_INCOMPLETE) && count > 0) {
        float priority = 1.0f;
        VkDeviceQueueCreateInfo queue = {.sType = VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
                                         .queueFamilyIndex = 0, .queueCount = 1,
                                         .pQueuePriorities = &priority};
        VkDeviceCreateInfo device_info = {.sType = VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO,
                                          .queueCreateInfoCount = 1, .pQueueCreateInfos = &queue};
        VkDevice device;
        status = vkCreateDevice(physical, &device_info, NULL, &device);
        if (status == VK_SUCCESS) {
            vkDeviceWaitIdle(device);
            vkDestroyDevice(device, NULL);
        }
    } else if (status == VK_SUCCESS) {
        status = VK_ERROR_INITIALIZATION_FAILED;
    }
    vkDestroyInstance(instance, NULL);
    printf("vulkan-control=%d\n", (int)status);
    return status == VK_SUCCESS ? 0 : 1;
}
