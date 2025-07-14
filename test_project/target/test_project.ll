; ModuleID = 'test_project'
source_filename = "test_project"

@.str = private unnamed_addr constant [17 x i8] c"Hello from LLVM!\00", align 1
@dynamic_format_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@dynamic_format_str.1 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1
@dynamic_format_str.2 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1
@dynamic_format_str.3 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1
@.str.4 = private unnamed_addr constant [17 x i8] c"x is less than y\00", align 1
@dynamic_format_str.5 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@.str.6 = private unnamed_addr constant [21 x i8] c"x is not less than y\00", align 1
@dynamic_format_str.7 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@dynamic_format_str.8 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1
@.str.9 = private unnamed_addr constant [15 x i8] c"For loop test:\00", align 1
@dynamic_format_str.10 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@dynamic_format_str.11 = private unnamed_addr constant [6 x i8] c"%lld\0A\00", align 1

declare i32 @printf(ptr, ...)

define void @main() {
entry:
  %puts = call i32 @puts(ptr nonnull dereferenceable(1) @.str)
  %printf_call_multi2 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.1, i64 10)
  %printf_call_multi4 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.2, i64 20)
  %printf_call_multi8 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.3, i64 30)
  %puts23 = call i32 @puts(ptr nonnull dereferenceable(1) @.str.4)
  br label %while.cond

while.cond:                                       ; preds = %while.body, %entry
  %count15 = phi i64 [ 0, %entry ], [ %addtmp18, %while.body ]
  %lttmp14 = icmp slt i64 %count15, 5
  br i1 %lttmp14, label %while.body, label %while.end

while.body:                                       ; preds = %while.cond
  %printf_call_multi16 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.8, i64 %count15)
  %addtmp18 = add i64 %count15, 1
  br label %while.cond

while.end:                                        ; preds = %while.cond
  %puts24 = call i32 @puts(ptr nonnull dereferenceable(1) @.str.9)
  br label %for.cond

for.cond:                                         ; preds = %for.body, %while.end
  %current22 = phi i64 [ 0, %while.end ], [ %nextval, %for.body ]
  %loopcond = icmp slt i64 %current22, 5
  br i1 %loopcond, label %for.body, label %for.exit

for.body:                                         ; preds = %for.cond
  %printf_call_multi21 = call i32 (ptr, ...) @printf(ptr noundef nonnull dereferenceable(1) @dynamic_format_str.11, i64 %current22)
  %nextval = add i64 %current22, 1
  br label %for.cond

for.exit:                                         ; preds = %for.cond
  ret void
}

; Function Attrs: nofree nounwind
declare noundef i32 @puts(ptr nocapture noundef readonly) #0

attributes #0 = { nofree nounwind }
