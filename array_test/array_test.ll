; ModuleID = 'array_test'
source_filename = "array_test"

@.str = private unnamed_addr constant [12 x i8] c"Array test:\00", align 1
@dynamic_format_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@dynamic_format_str.1 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1
@dynamic_format_str.2 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1
@dynamic_format_str.3 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1

declare i32 @printf(ptr, ...)

define void @main() {
entry:
  %puts = call i32 @puts(ptr nonnull dereferenceable(1) @.str)
  %array_literal = alloca [5 x i64], align 8
  store i64 1, ptr %array_literal, align 4
  %array_elem_1 = getelementptr [5 x i64], ptr %array_literal, i64 0, i64 1
  store i64 2, ptr %array_elem_1, align 4
  %array_elem_2 = getelementptr inbounds [5 x i64], ptr %array_literal, i64 0, i64 2
  store i64 3, ptr %array_elem_2, align 4
  %array_elem_3 = getelementptr inbounds [5 x i64], ptr %array_literal, i64 0, i64 3
  store i64 4, ptr %array_elem_3, align 4
  %array_elem_4 = getelementptr inbounds [5 x i64], ptr %array_literal, i64 0, i64 4
  store i64 5, ptr %array_elem_4, align 4
  %printf_call_multi6 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.1, i64 1)
  %printf_call_multi8 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.2, i64 2)
  %array_element11 = load i64, ptr %array_literal, align 4
  %array_element14 = load i64, ptr %array_elem_1, align 4
  %addtmp = add i64 %array_element14, %array_element11
  %printf_call_multi16 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.3, i64 %addtmp)
  ret void
}

; Function Attrs: nofree nounwind
declare noundef i32 @puts(ptr nocapture noundef readonly) #0

attributes #0 = { nofree nounwind }
