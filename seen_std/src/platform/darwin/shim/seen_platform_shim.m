// Seen Platform Shim - macOS/iOS
// Objective-C bridge for Metal, AppKit, CoreAudio, and GameController frameworks
// Compiled as: clang -shared -fPIC -o libseen_platform.dylib seen_platform_shim.m \
//              -framework Metal -framework AppKit -framework CoreAudio \
//              -framework AudioToolbox -framework GameController

#import <Foundation/Foundation.h>

#if __has_include(<Metal/Metal.h>)
#define SEEN_HAS_METAL 1
#import <Metal/Metal.h>
#endif

#if TARGET_OS_OSX
#if __has_include(<AppKit/AppKit.h>)
#define SEEN_HAS_APPKIT 1
#import <AppKit/AppKit.h>
#endif
#endif

#ifdef SEEN_USE_UIKIT
#define SEEN_HAS_UIKIT 1
#import <UIKit/UIKit.h>
#endif

#if __has_include(<CoreAudio/CoreAudio.h>)
#define SEEN_HAS_COREAUDIO 1
#import <CoreAudio/CoreAudio.h>
#import <AudioToolbox/AudioToolbox.h>
#endif

#ifdef SEEN_USE_GAMECONTROLLER
#if __has_include(<GameController/GameController.h>)
#define SEEN_HAS_GAMECONTROLLER 1
#import <GameController/GameController.h>
#endif
#endif

#ifdef SEEN_USE_GAMECONTROLLER
#if __has_include(<CoreHaptics/CoreHaptics.h>)
#define SEEN_HAS_COREHAPTICS 1
#import <CoreHaptics/CoreHaptics.h>
#endif
#endif

// ============================================================================
// Metal Bindings
// ============================================================================

#ifdef SEEN_HAS_METAL

void* seen_mtl_create_system_default_device(void) {
    id<MTLDevice> device = MTLCreateSystemDefaultDevice();
    return (__bridge_retained void*)device;
}

const char* seen_mtl_device_name(void* device) {
    id<MTLDevice> mtlDevice = (__bridge id<MTLDevice>)device;
    return [mtlDevice.name UTF8String];
}

void* seen_mtl_create_command_queue(void* device) {
    id<MTLDevice> mtlDevice = (__bridge id<MTLDevice>)device;
    id<MTLCommandQueue> queue = [mtlDevice newCommandQueue];
    return (__bridge_retained void*)queue;
}

void* seen_mtl_create_buffer(void* device, int64_t length, int64_t options) {
    id<MTLDevice> mtlDevice = (__bridge id<MTLDevice>)device;
    id<MTLBuffer> buffer = [mtlDevice newBufferWithLength:(NSUInteger)length
                                                 options:(MTLResourceOptions)options];
    return (__bridge_retained void*)buffer;
}

void* seen_mtl_buffer_contents(void* buffer) {
    id<MTLBuffer> mtlBuffer = (__bridge id<MTLBuffer>)buffer;
    return [mtlBuffer contents];
}

int64_t seen_mtl_buffer_length(void* buffer) {
    id<MTLBuffer> mtlBuffer = (__bridge id<MTLBuffer>)buffer;
    return (int64_t)[mtlBuffer length];
}

void* seen_mtl_create_library_from_source(void* device, const char* source) {
    id<MTLDevice> mtlDevice = (__bridge id<MTLDevice>)device;
    NSError* error = nil;
    NSString* sourceStr = [NSString stringWithUTF8String:source];
    id<MTLLibrary> library = [mtlDevice newLibraryWithSource:sourceStr
                                                    options:nil
                                                      error:&error];
    if (error) {
        NSLog(@"[Seen Metal] Library compilation error: %@", error);
        return NULL;
    }
    return (__bridge_retained void*)library;
}

void* seen_mtl_create_function(void* library, const char* name) {
    id<MTLLibrary> mtlLibrary = (__bridge id<MTLLibrary>)library;
    NSString* nameStr = [NSString stringWithUTF8String:name];
    id<MTLFunction> function = [mtlLibrary newFunctionWithName:nameStr];
    return (__bridge_retained void*)function;
}

void* seen_mtl_create_compute_pipeline(void* device, void* function) {
    id<MTLDevice> mtlDevice = (__bridge id<MTLDevice>)device;
    id<MTLFunction> mtlFunction = (__bridge id<MTLFunction>)function;
    NSError* error = nil;
    id<MTLComputePipelineState> pipeline =
        [mtlDevice newComputePipelineStateWithFunction:mtlFunction error:&error];
    if (error) {
        NSLog(@"[Seen Metal] Compute pipeline error: %@", error);
        return NULL;
    }
    return (__bridge_retained void*)pipeline;
}

void* seen_mtl_create_command_buffer(void* queue) {
    id<MTLCommandQueue> mtlQueue = (__bridge id<MTLCommandQueue>)queue;
    id<MTLCommandBuffer> cmdBuffer = [mtlQueue commandBuffer];
    return (__bridge_retained void*)cmdBuffer;
}

void* seen_mtl_create_compute_encoder(void* commandBuffer) {
    id<MTLCommandBuffer> cmdBuffer = (__bridge id<MTLCommandBuffer>)commandBuffer;
    id<MTLComputeCommandEncoder> encoder = [cmdBuffer computeCommandEncoder];
    return (__bridge_retained void*)encoder;
}

void seen_mtl_compute_set_pipeline(void* encoder, void* pipeline) {
    id<MTLComputeCommandEncoder> enc = (__bridge id<MTLComputeCommandEncoder>)encoder;
    id<MTLComputePipelineState> pso = (__bridge id<MTLComputePipelineState>)pipeline;
    [enc setComputePipelineState:pso];
}

void seen_mtl_compute_set_buffer(void* encoder, void* buffer, int64_t offset, int64_t index) {
    id<MTLComputeCommandEncoder> enc = (__bridge id<MTLComputeCommandEncoder>)encoder;
    id<MTLBuffer> buf = (__bridge id<MTLBuffer>)buffer;
    [enc setBuffer:buf offset:(NSUInteger)offset atIndex:(NSUInteger)index];
}

void seen_mtl_compute_dispatch(void* encoder,
                                int64_t gridX, int64_t gridY, int64_t gridZ,
                                int64_t groupX, int64_t groupY, int64_t groupZ) {
    id<MTLComputeCommandEncoder> enc = (__bridge id<MTLComputeCommandEncoder>)encoder;
    MTLSize grid = MTLSizeMake((NSUInteger)gridX, (NSUInteger)gridY, (NSUInteger)gridZ);
    MTLSize group = MTLSizeMake((NSUInteger)groupX, (NSUInteger)groupY, (NSUInteger)groupZ);
    [enc dispatchThreadgroups:grid threadsPerThreadgroup:group];
}

void seen_mtl_end_encoding(void* encoder) {
    id<MTLCommandEncoder> enc = (__bridge id<MTLCommandEncoder>)encoder;
    [enc endEncoding];
}

void seen_mtl_commit(void* commandBuffer) {
    id<MTLCommandBuffer> cmdBuffer = (__bridge id<MTLCommandBuffer>)commandBuffer;
    [cmdBuffer commit];
}

void seen_mtl_wait_until_completed(void* commandBuffer) {
    id<MTLCommandBuffer> cmdBuffer = (__bridge id<MTLCommandBuffer>)commandBuffer;
    [cmdBuffer waitUntilCompleted];
}

void seen_mtl_release(void* object) {
    if (object) {
        CFRelease(object);
    }
}

#endif // SEEN_HAS_METAL

// ============================================================================
// CoreAudio Bindings
// ============================================================================

#ifdef SEEN_HAS_COREAUDIO

typedef struct {
    AudioQueueRef queue;
    AudioStreamBasicDescription format;
    int running;
} SeenAudioStream;

void* seen_audio_create_output(double sampleRate, int64_t channels, int64_t bufferSize) {
    SeenAudioStream* stream = calloc(1, sizeof(SeenAudioStream));
    if (!stream) return NULL;

    stream->format.mSampleRate = sampleRate;
    stream->format.mFormatID = kAudioFormatLinearPCM;
    stream->format.mFormatFlags = kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked;
    stream->format.mBitsPerChannel = 32;
    stream->format.mChannelsPerFrame = (UInt32)channels;
    stream->format.mFramesPerPacket = 1;
    stream->format.mBytesPerFrame = (UInt32)(channels * 4);
    stream->format.mBytesPerPacket = stream->format.mBytesPerFrame;

    return stream;
}

int64_t seen_audio_start(void* streamPtr) {
    SeenAudioStream* stream = (SeenAudioStream*)streamPtr;
    if (!stream) return -1;
    stream->running = 1;
    return 0;
}

int64_t seen_audio_stop(void* streamPtr) {
    SeenAudioStream* stream = (SeenAudioStream*)streamPtr;
    if (!stream) return -1;
    if (stream->queue) {
        AudioQueueStop(stream->queue, true);
    }
    stream->running = 0;
    return 0;
}

void seen_audio_destroy(void* streamPtr) {
    SeenAudioStream* stream = (SeenAudioStream*)streamPtr;
    if (!stream) return;
    if (stream->queue) {
        AudioQueueDispose(stream->queue, true);
    }
    free(stream);
}

#endif // SEEN_HAS_COREAUDIO

// ============================================================================
// Game Controller Bindings
// ============================================================================

#ifdef SEEN_HAS_GAMECONTROLLER

int64_t seen_gc_get_controller_count(void) {
    return (int64_t)[[GCController controllers] count];
}

void* seen_gc_get_controller(int64_t index) {
    NSArray<GCController*>* controllers = [GCController controllers];
    if (index < 0 || index >= (int64_t)controllers.count) return NULL;
    return (__bridge void*)controllers[index];
}

const char* seen_gc_get_controller_name(void* controller) {
    GCController* gc = (__bridge GCController*)controller;
    return [gc.vendorName UTF8String];
}

int seen_gc_is_button_pressed(void* controller, int64_t button) {
    GCController* gc = (__bridge GCController*)controller;
    GCExtendedGamepad* gamepad = gc.extendedGamepad;
    if (!gamepad) return 0;

    switch (button) {
        case 0: return gamepad.buttonA.pressed;
        case 1: return gamepad.buttonB.pressed;
        case 2: return gamepad.buttonX.pressed;
        case 3: return gamepad.buttonY.pressed;
        case 4: return gamepad.dpad.up.pressed;
        case 5: return gamepad.dpad.down.pressed;
        case 6: return gamepad.dpad.left.pressed;
        case 7: return gamepad.dpad.right.pressed;
        case 8: return gamepad.leftShoulder.pressed;
        case 9: return gamepad.rightShoulder.pressed;
        case 10: return gamepad.leftThumbstickButton.pressed;
        case 11: return gamepad.rightThumbstickButton.pressed;
        case 12: return gamepad.buttonMenu.pressed;
        case 13: return gamepad.buttonOptions.pressed;
        case 14: return gamepad.buttonHome.pressed;
        default: return 0;
    }
}

double seen_gc_get_axis_value(void* controller, int64_t axis) {
    GCController* gc = (__bridge GCController*)controller;
    GCExtendedGamepad* gamepad = gc.extendedGamepad;
    if (!gamepad) return 0.0;

    switch (axis) {
        case 0: return gamepad.leftThumbstick.xAxis.value;
        case 1: return gamepad.leftThumbstick.yAxis.value;
        case 2: return gamepad.rightThumbstick.xAxis.value;
        case 3: return gamepad.rightThumbstick.yAxis.value;
        case 4: return gamepad.leftTrigger.value;
        case 5: return gamepad.rightTrigger.value;
        default: return 0.0;
    }
}

int seen_gc_has_haptics(void* controller) {
    GCController* gc = (__bridge GCController*)controller;
    return gc.haptics != nil;
}

void seen_gc_play_haptic(void* controller, double intensity, double duration) {
#if defined(SEEN_HAS_COREHAPTICS) && defined(SEEN_HAS_GAMECONTROLLER)
    GCController* gc = (__bridge GCController*)controller;
    if (!gc.haptics) return;

    CHHapticEngine* engine = [gc.haptics createEngineWithLocality:GCHapticsLocalityDefault];
    NSError* error = nil;
    [engine startAndReturnError:&error];
    if (error) return;

    CHHapticEventParameter* intensityParam = [[CHHapticEventParameter alloc]
        initWithParameterID:CHHapticEventParameterIDHapticIntensity value:intensity];
    CHHapticEventParameter* sharpnessParam = [[CHHapticEventParameter alloc]
        initWithParameterID:CHHapticEventParameterIDHapticSharpness value:0.5];

    CHHapticEvent* event = [[CHHapticEvent alloc]
        initWithEventType:CHHapticEventTypeHapticContinuous
        parameters:@[intensityParam, sharpnessParam]
        relativeTime:0.0
        duration:duration];

    CHHapticPattern* pattern = [[CHHapticPattern alloc] initWithEvents:@[event] parameters:@[] error:&error];
    if (error) { [engine stopWithCompletionHandler:nil]; return; }

    id<CHHapticPatternPlayer> player = [engine createPlayerWithPattern:pattern error:&error];
    if (error) { [engine stopWithCompletionHandler:nil]; return; }

    [player startAtTime:CHHapticTimeImmediate error:nil];
#else
    (void)controller; (void)intensity; (void)duration;
#endif
}

int seen_gc_has_motion(void* controller) {
    GCController* gc = (__bridge GCController*)controller;
    return gc.motion != nil;
}

void seen_gc_start_discovery(void) {
    [GCController startWirelessControllerDiscoveryWithCompletionHandler:nil];
}

void seen_gc_stop_discovery(void) {
    [GCController stopWirelessControllerDiscovery];
}

#endif // SEEN_HAS_GAMECONTROLLER
