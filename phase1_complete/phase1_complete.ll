; ModuleID = 'phase1_complete'
source_filename = "phase1_complete"

@dynamic_format_str = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1
@dynamic_format_str.1 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1
@dynamic_format_str.2 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1
@dynamic_format_str.3 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1
@dynamic_format_str.4 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1
@dynamic_format_str.5 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1

declare i32 @printf(ptr, ...)

define i64 @fibonacci(i64 %n) {
entry:
  %letmp = icmp slt i64 %n, 2
  br i1 %letmp, label %common.ret, label %else

common.ret:                                       ; preds = %entry, %else
  %common.ret.op = phi i64 [ %addtmp, %else ], [ %n, %entry ]
  ret i64 %common.ret.op

else:                                             ; preds = %entry
  %subtmp = add i64 %n, -1
  %call = call i64 @fibonacci(i64 %subtmp)
  %subtmp6 = add i64 %n, -2
  %call7 = call i64 @fibonacci(i64 %subtmp6)
  %addtmp = add i64 %call7, %call
  br label %common.ret
}

define void @main() {
entry:
  %printf_call_multi = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str, i64 15)
  %printf_call_multi8 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.1, i64 50)
  %printf_call_multi10 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.2, i64 1)
  %array_literal = alloca [5 x i64], align 8
  store i64 1, ptr %array_literal, align 4
  %array_elem_1 = getelementptr inbounds [5 x i64], ptr %array_literal, i64 0, i64 1
  store i64 2, ptr %array_elem_1, align 4
  %array_elem_2 = getelementptr inbounds [5 x i64], ptr %array_literal, i64 0, i64 2
  store i64 3, ptr %array_elem_2, align 4
  %array_elem_3 = getelementptr inbounds [5 x i64], ptr %array_literal, i64 0, i64 3
  store i64 4, ptr %array_elem_3, align 4
  %array_elem_4 = getelementptr inbounds [5 x i64], ptr %array_literal, i64 0, i64 4
  store i64 5, ptr %array_elem_4, align 4
  %printf_call_multi16 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.3, i64 1)
  %printf_call_multi18 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.4, i64 5)
  %call = call i64 @fibonacci(i64 6)
  %printf_call_multi20 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.5, i64 %call)
  ret void
}
