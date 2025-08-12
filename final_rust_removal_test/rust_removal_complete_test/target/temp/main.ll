target triple = "x86_64-unknown-linux-gnu"
; Module: cli_build


; String constants
@.str.0 = private unnamed_addr constant [14 x i8] c"Hello, World!\00", align 1
@.str.1 = private unnamed_addr constant [42 x i8] c"Welcome to the Seen programming language!\00", align 1

; Standard library function declarations
declare i32 @printf(i8*, ...)
declare i32 @puts(i8*)
declare i64 @strlen(i8*)
declare i8* @malloc(i32)
declare void @free(i8*)
declare void @llvm.memcpy.p0i8.p0i8.i32(i8*, i8*, i32, i1)
declare void @llvm.memmove.p0i8.p0i8.i32(i8*, i8*, i32, i1)
declare void @llvm.memset.p0i8.i32(i8*, i8, i32, i1)

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
  call void @println(i8* getelementptr inbounds ([14 x i8], [14 x i8]* @.str.0, i32 0, i32 0))
  call void @println(i8* getelementptr inbounds ([42 x i8], [42 x i8]* @.str.1, i32 0, i32 0))
  %0 = alloca i32*, align 4
  store i32 %0, i32* %0, align 4
  %1 = load i32, i32* %0, align 4
  call void @println(%1)
  ret i32 0
}

