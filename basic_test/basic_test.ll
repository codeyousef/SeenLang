; ModuleID = 'basic_test'
source_filename = "basic_test"

@dynamic_format_str = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1

declare i32 @printf(ptr, ...)

define void @main() {
entry:
  %printf_call_multi = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str, i64 30)
  ret void
}
