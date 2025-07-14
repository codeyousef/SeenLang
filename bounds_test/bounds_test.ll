; ModuleID = 'bounds_test'
source_filename = "bounds_test"

@dynamic_format_str = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1
@dynamic_format_str.1 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1

declare i32 @printf(ptr, ...)

define void @main() {
entry:
  %array_literal = alloca [3 x i64], align 8
  store i64 1, ptr %array_literal, align 4
  %array_elem_1 = getelementptr [3 x i64], ptr %array_literal, i64 0, i64 1
  store i64 2, ptr %array_elem_1, align 4
  %array_elem_2 = getelementptr inbounds [3 x i64], ptr %array_literal, i64 0, i64 2
  store i64 3, ptr %array_elem_2, align 4
  %printf_call_multi = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str, i64 1)
  %array_element8 = load i64, ptr %array_elem_1, align 4
  %printf_call_multi11 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.1, i64 %array_element8)
  ret void
}
