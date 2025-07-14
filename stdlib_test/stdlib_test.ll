; ModuleID = 'stdlib_test'
source_filename = "stdlib_test"

@dynamic_format_str = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1
@dynamic_format_str.1 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1
@dynamic_format_str.2 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1
@dynamic_format_str.3 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1

declare i32 @printf(ptr, ...)

define void @main() {
entry:
  %printf_call_multi = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str, i64 0)
  %printf_call_multi8 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.1, i64 0)
  %printf_call_multi10 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.2, i64 0)
  %printf_call_multi12 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.3, i64 99)
  ret void
}
