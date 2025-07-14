; ModuleID = 'core_test'
source_filename = "core_test"

@dynamic_format_str = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1

declare i32 @printf(ptr, ...)

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
  ret void
}
