# Seen Platform Shim Makefile
# Builds libseen_platform.so for Linux game development

CC ?= gcc
CFLAGS := -Wall -Wextra -fPIC -O2

# Feature flags - enable what you need
FEATURES ?= sdl3 vulkan alsa

# Build feature defines
DEFINES :=
LIBS :=
PKG_CONFIG_PKGS :=

ifneq (,$(findstring sdl3,$(FEATURES)))
    DEFINES += -DSEEN_USE_SDL3
    PKG_CONFIG_PKGS += sdl3
endif

ifneq (,$(findstring vulkan,$(FEATURES)))
    DEFINES += -DSEEN_USE_VULKAN
    PKG_CONFIG_PKGS += vulkan
endif

ifneq (,$(findstring alsa,$(FEATURES)))
    DEFINES += -DSEEN_USE_ALSA
    PKG_CONFIG_PKGS += alsa
endif

ifneq (,$(findstring pipewire,$(FEATURES)))
    DEFINES += -DSEEN_USE_PIPEWIRE
    PKG_CONFIG_PKGS += libpipewire-0.3
endif

ifneq (,$(findstring evdev,$(FEATURES)))
    DEFINES += -DSEEN_USE_EVDEV
    PKG_CONFIG_PKGS += libevdev
endif

ifneq (,$(findstring libinput,$(FEATURES)))
    DEFINES += -DSEEN_USE_LIBINPUT
    PKG_CONFIG_PKGS += libinput
endif

# Get pkg-config flags
ifneq ($(PKG_CONFIG_PKGS),)
    PKG_CFLAGS := $(shell pkg-config --cflags $(PKG_CONFIG_PKGS) 2>/dev/null)
    PKG_LIBS := $(shell pkg-config --libs $(PKG_CONFIG_PKGS) 2>/dev/null)
endif

CFLAGS += $(DEFINES) $(PKG_CFLAGS)
LDFLAGS := -shared
LIBS += $(PKG_LIBS)

# Output
BUILD_DIR := build
LIB_NAME := libseen_platform.so
STATIC_LIB_NAME := libseen_platform.a

# Sources
SRCS := seen_platform_shim.c
OBJS := $(SRCS:%.c=$(BUILD_DIR)/%.o)

.PHONY: all clean shared static install help check-deps

all: shared

shared: $(BUILD_DIR)/$(LIB_NAME)

static: $(BUILD_DIR)/$(STATIC_LIB_NAME)

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

$(BUILD_DIR)/%.o: %.c | $(BUILD_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(BUILD_DIR)/$(LIB_NAME): $(OBJS)
	$(CC) $(LDFLAGS) -o $@ $^ $(LIBS)
	@echo "Built: $@"
	@echo "Link with: -L$(BUILD_DIR) -lseen_platform"

$(BUILD_DIR)/$(STATIC_LIB_NAME): $(OBJS)
	ar rcs $@ $^
	@echo "Built: $@"

clean:
	rm -rf $(BUILD_DIR)

# Install to a prefix (default /usr/local)
PREFIX ?= /usr/local
install: $(BUILD_DIR)/$(LIB_NAME)
	install -d $(PREFIX)/lib
	install -m 755 $(BUILD_DIR)/$(LIB_NAME) $(PREFIX)/lib/
	ldconfig || true
	@echo "Installed to $(PREFIX)/lib/$(LIB_NAME)"

check-deps:
	@echo "Checking dependencies..."
	@for pkg in $(PKG_CONFIG_PKGS); do \
		if pkg-config --exists $$pkg 2>/dev/null; then \
			echo "  ✓ $$pkg"; \
		else \
			echo "  ✗ $$pkg (not found)"; \
		fi \
	done

help:
	@echo "Seen Platform Shim Build"
	@echo ""
	@echo "Usage: make [target] [FEATURES='feature1 feature2 ...']"
	@echo ""
	@echo "Targets:"
	@echo "  all        - Build shared library (default)"
	@echo "  shared     - Build shared library (.so)"
	@echo "  static     - Build static library (.a)"
	@echo "  clean      - Remove build artifacts"
	@echo "  install    - Install to PREFIX (default /usr/local)"
	@echo "  check-deps - Check if required packages are installed"
	@echo "  help       - Show this help"
	@echo ""
	@echo "Features (space-separated):"
	@echo "  sdl3      - SDL3 window/input support"
	@echo "  vulkan    - Vulkan graphics support"
	@echo "  alsa      - ALSA audio support"
	@echo "  pipewire  - PipeWire audio support"
	@echo "  evdev     - evdev input support"
	@echo "  libinput  - libinput support"
	@echo ""
	@echo "Examples:"
	@echo "  make FEATURES='sdl3 vulkan'        # SDL3 + Vulkan only"
	@echo "  make FEATURES='sdl3 vulkan alsa'   # Full game support"
	@echo "  make check-deps FEATURES='sdl3 vulkan alsa pipewire evdev'"
	@echo ""
	@echo "Currently enabled features: $(FEATURES)"
