; ModuleID = 'stdlib_complete'
source_filename = "stdlib_complete"

@dynamic_format_str = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1
@dynamic_format_str.1 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1
@dynamic_format_str.2 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1
@dynamic_format_str.3 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1
@dynamic_format_str.4 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1
@dynamic_format_str.5 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1
@dynamic_format_str.6 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1

declare i32 @printf(ptr, ...)

define i64 @abs(i64 %x) {
entry:
  %lttmp = icmp slt i64 %x, 0
  %subtmp = sub i64 0, %x
  %common.ret.op = select i1 %lttmp, i64 %subtmp, i64 %x
  ret i64 %common.ret.op
}

define i64 @factorial(i64 %n) {
entry:
  %letmp = icmp slt i64 %n, 2
  br i1 %letmp, label %common.ret, label %else

common.ret:                                       ; preds = %entry, %else
  %common.ret.op = phi i64 [ %multmp, %else ], [ 1, %entry ]
  ret i64 %common.ret.op

else:                                             ; preds = %entry
  %subtmp = add i64 %n, -1
  %call = call i64 @factorial(i64 %subtmp)
  %multmp = mul i64 %call, %n
  br label %common.ret
}

define void @main() {
entry:
  %call = call i64 @factorial(i64 5)
  %printf_call_multi = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str, i64 %call)
  %call3 = call i64 @abs(i64 -10)
  %printf_call_multi5 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.1, i64 %call3)
  %array_literal = alloca [5 x i64], align 8
  store i64 1, ptr %array_literal, align 4
  %array_elem_1 = getelementptr inbounds [5 x i64], ptr %array_literal, i64 0, i64 1
  store i64 2, ptr %array_elem_1, align 4
  %array_elem_2 = getelementptr inbounds [5 x i64], ptr %array_literal, i64 0, i64 2
  store i64 3, ptr %array_elem_2, align 4
  %array_elem_3 = getelementptr inbounds [5 x i64], ptr %array_literal, i64 0, i64 3
  store i64 4, ptr %array_elem_3, align 4
  %array_elem_4 = getelementptr [5 x i64], ptr %array_literal, i64 0, i64 4
  store i64 5, ptr %array_elem_4, align 4
  %printf_call_multi15 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.2, i64 1)
  %printf_call_multi17 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.3, i64 5)
  %printf_call_multi23 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.4, i64 6)
  %printf_call_multi25 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.5, i64 12)
  %printf_call_multi27 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.6, i64 0)
  ret void
}
