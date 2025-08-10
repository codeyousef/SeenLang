target triple = "x86_64-unknown-linux-gnu"
; Module: cli_build


; String constants
@.str.0 = private unnamed_addr constant [35 x i8] c"Seen Compiler v1.0.0 (Self-hosted)\00", align 1
@.str.1 = private unnamed_addr constant [20 x i8] c"Bootstrap: Complete\00", align 1

; Standard library function declarations
declare i32 @printf(i8*, ...)
declare i32 @puts(i8*)

; Seen print function wrappers
define void @print(i8* %str) {
entry:
  %result = call i32 @printf(i8* %str)
  ret void
}

define void @println(i8* %str) {
entry:
  %result = call i32 @puts(i8* %str)
  ret void
}

define i32 @main() nounwind {
entry:
  call void @println(i8* getelementptr inbounds ([35 x i8], [35 x i8]* @.str.0, i32 0, i32 0))
  call void @println(i8* getelementptr inbounds ([20 x i8], [20 x i8]* @.str.1, i32 0, i32 0))
  ret i32 0
}

