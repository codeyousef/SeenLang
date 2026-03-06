# Seen Platform Shim Makefile - macOS/iOS
# Builds libseen_platform.dylib for macOS game/app development

CC ?= clang
CFLAGS := -Wall -Wextra -fPIC -O2

# Feature flags - enable what you need
FEATURES ?= metal coreaudio gamecontroller

# Build feature defines
DEFINES :=
FRAMEWORKS :=

ifneq (,$(findstring metal,$(FEATURES)))
    DEFINES += -DSEEN_USE_METAL
    FRAMEWORKS += -framework Metal -framework MetalKit
endif

ifneq (,$(findstring appkit,$(FEATURES)))
    DEFINES += -DSEEN_USE_APPKIT
    FRAMEWORKS += -framework AppKit
endif

ifneq (,$(findstring coreaudio,$(FEATURES)))
    DEFINES += -DSEEN_USE_COREAUDIO
    FRAMEWORKS += -framework CoreAudio -framework AudioToolbox
endif

ifneq (,$(findstring gamecontroller,$(FEATURES)))
    DEFINES += -DSEEN_USE_GAMECONTROLLER
    FRAMEWORKS += -framework GameController -framework CoreHaptics
endif

ifneq (,$(findstring uikit,$(FEATURES)))
    DEFINES += -DSEEN_USE_UIKIT
    FRAMEWORKS += -framework UIKit
endif

ifneq (,$(findstring sdl3,$(FEATURES)))
    DEFINES += -DSEEN_USE_SDL3
    # SDL3 on macOS can be installed via Homebrew
    SDL3_CFLAGS := $(shell sdl3-config --cflags 2>/dev/null || pkg-config --cflags sdl3 2>/dev/null)
    SDL3_LIBS := $(shell sdl3-config --libs 2>/dev/null || pkg-config --libs sdl3 2>/dev/null)
    CFLAGS += $(SDL3_CFLAGS)
    FRAMEWORKS += $(SDL3_LIBS)
endif

CFLAGS += $(DEFINES)

# Output
BUILD_DIR := build
LIB_NAME := libseen_platform.dylib
STATIC_LIB_NAME := libseen_platform.a

# Sources (Objective-C)
SRCS := seen_platform_shim.m
OBJS := $(SRCS:%.m=$(BUILD_DIR)/%.o)

# iOS build
IOS_SDK := $(shell xcrun --sdk iphoneos --show-sdk-path 2>/dev/null || echo /Applications/Xcode.app/Contents/Developer/Platforms/iPhoneOS.platform/Developer/SDKs/iPhoneOS.sdk)
IOS_CC := $(shell xcrun --sdk iphoneos -f clang 2>/dev/null || echo clang) -target arm64-apple-ios17.0 -isysroot $(IOS_SDK)
IOS_BUILD_DIR := build-ios

.PHONY: all clean shared static ios install help check-deps

all: shared

shared: $(BUILD_DIR)/$(LIB_NAME)

static: $(BUILD_DIR)/$(STATIC_LIB_NAME)

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

$(BUILD_DIR)/%.o: %.m | $(BUILD_DIR)
	$(CC) $(CFLAGS) -fobjc-arc -c $< -o $@

$(BUILD_DIR)/$(LIB_NAME): $(OBJS)
	$(CC) -dynamiclib -o $@ $^ $(FRAMEWORKS) -framework Foundation
	@echo "Built: $@"
	@echo "Link with: -L$(BUILD_DIR) -lseen_platform"

$(BUILD_DIR)/$(STATIC_LIB_NAME): $(OBJS)
	ar rcs $@ $^
	@echo "Built: $@"

# iOS targets
ios: $(IOS_BUILD_DIR)/$(STATIC_LIB_NAME)

$(IOS_BUILD_DIR):
	mkdir -p $(IOS_BUILD_DIR)

$(IOS_BUILD_DIR)/%.o: %.m | $(IOS_BUILD_DIR)
	$(IOS_CC) $(CFLAGS) -fobjc-arc -c $< -o $@

$(IOS_BUILD_DIR)/$(STATIC_LIB_NAME): $(SRCS:%.m=$(IOS_BUILD_DIR)/%.o)
	ar rcs $@ $^
	@echo "Built iOS static lib: $@"

clean:
	rm -rf $(BUILD_DIR) $(IOS_BUILD_DIR)

# Install to a prefix (default /usr/local)
PREFIX ?= /usr/local
install: $(BUILD_DIR)/$(LIB_NAME)
	install -d $(PREFIX)/lib
	install -m 755 $(BUILD_DIR)/$(LIB_NAME) $(PREFIX)/lib/
	@echo "Installed to $(PREFIX)/lib/$(LIB_NAME)"

check-deps:
	@echo "Checking macOS framework availability..."
	@echo "  Metal:          $(shell test -d /System/Library/Frameworks/Metal.framework && echo 'Available' || echo 'Not found')"
	@echo "  CoreAudio:      $(shell test -d /System/Library/Frameworks/CoreAudio.framework && echo 'Available' || echo 'Not found')"
	@echo "  GameController: $(shell test -d /System/Library/Frameworks/GameController.framework && echo 'Available' || echo 'Not found')"
	@echo "  AppKit:         $(shell test -d /System/Library/Frameworks/AppKit.framework && echo 'Available' || echo 'Not found')"
	@echo ""
	@echo "Optional:"
	@echo "  SDL3:           $(shell command -v sdl3-config >/dev/null 2>&1 && echo 'Available' || echo 'Not found (install via: brew install sdl3)')"

help:
	@echo "Seen Platform Shim Build (macOS)"
	@echo ""
	@echo "Usage: make [target] [FEATURES='feature1 feature2 ...']"
	@echo ""
	@echo "Targets:"
	@echo "  all        - Build shared library (default)"
	@echo "  shared     - Build shared library (.dylib)"
	@echo "  static     - Build static library (.a)"
	@echo "  ios        - Build iOS static library (.a, arm64)"
	@echo "  clean      - Remove build artifacts"
	@echo "  install    - Install to PREFIX (default /usr/local)"
	@echo "  check-deps - Check if required frameworks are available"
	@echo "  help       - Show this help"
	@echo ""
	@echo "Features (space-separated):"
	@echo "  metal           - Metal GPU compute & rendering"
	@echo "  appkit          - AppKit window management"
	@echo "  coreaudio       - CoreAudio audio output/input"
	@echo "  gamecontroller  - Game Controller framework"
	@echo "  sdl3            - SDL3 cross-platform support"
	@echo ""
	@echo "Examples:"
	@echo "  make FEATURES='metal coreaudio'              # GPU + Audio"
	@echo "  make FEATURES='metal coreaudio gamecontroller sdl3'  # Full game support"
	@echo ""
	@echo "Currently enabled features: $(FEATURES)"
