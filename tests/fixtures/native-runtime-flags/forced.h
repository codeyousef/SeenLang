#ifndef FEL748_NATIVE_FLAGS_FORCED_H
#define FEL748_NATIVE_FLAGS_FORCED_H
#include <fel748_separate.h>
#include <fel748_combined.h>
#include <fel748_spaces.h>
#include <fel748_system.h>
#include <fel748_environment.h>
#include <string.h>
#ifdef FEL748_EXPECT_AFTER
#include <fel748_after.h>
#if FEL748_AFTER != 67
#error system-after search directory was not consumed
#endif
#endif
#ifdef FEL748_EXPECT_FRAMEWORK
#include <Fel748/value.h>
#if FEL748_FRAMEWORK != 71
#error framework search directory was not consumed
#endif
#endif
#if FEL748_SEPARATE != 11 || FEL748_COMBINED != 23 || FEL748_SPACES != 37 || FEL748_SYSTEM != 41
#error native include flag paths were not consumed correctly
#endif
#if FEL748_ENVIRONMENT != 619
#error native environment include precedence changed
#endif
#if FEL748_IMACROS != 53
#error native imacros header was not consumed
#endif
#ifndef FEL748_QUOTED_TEXT
#error quoted macro argument was dropped
#endif
long long seen_gpu_native_flags_value(void) {
    return strcmp(FEL748_QUOTED_TEXT, "quoted macro with spaces:748") == 0 ? 748 : -1;
}
#endif
