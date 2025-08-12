target triple = "x86_64-unknown-linux-gnu"
; Module: cli_build


; String constants
@.str.0 = private unnamed_addr constant [14 x i8] c"Hello, World!\00", align 1
@.str.1 = private unnamed_addr constant [42 x i8] c"Welcome to the Seen programming language!\00", align 1
@.str.2 = private unnamed_addr constant [7 x i8] c"Hello \00", align 1
@.str.3 = private unnamed_addr constant [33 x i8] c"Testing basic syntax features...\00", align 1
@.str.4 = private unnamed_addr constant [27 x i8] c"Function result calculated\00", align 1
@.str.5 = private unnamed_addr constant [5 x i8] c"Seen\00", align 1
@.str.6 = private unnamed_addr constant [20 x i8] c"Boolean logic works\00", align 1
@.str.7 = private unnamed_addr constant [27 x i8] c"Variable declarations work\00", align 1
@.str.8 = private unnamed_addr constant [29 x i8] c"Basic syntax test completed!\00", align 1

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

define internal i32 @simpleFunction(i32 %x) fastcc {
entry:
  %0 = add i32 0, 1
  ret i32 %0
}

define internal i32 @testStringConcatenation(i32 %name) fastcc {
entry:
  %0 = add i32 @.str.2, 0
  ret i32 %0
}

define internal i32 @testBooleanLogic(i32 %x, i32 %y) fastcc {
entry:
  ret i32 0
}

define i32 @main() nounwind {
entry:
  call void @println(i8* getelementptr inbounds ([33 x i8], [33 x i8]* @.str.3, i32 0, i32 0))
  %1 = call i32 @simpleFunction(42)
  %0 = alloca i32, align 4
  store i32 %1, i32* %0, align 4
  call void @println(i8* getelementptr inbounds ([27 x i8], [27 x i8]* @.str.4, i32 0, i32 0))
  %3 = call i32 @testStringConcatenation(@.str.5)
  %2 = alloca i32, align 4
  store i32 %3, i32* %2, align 4
  %4 = load i32, i32* %2, align 4
  call void @println(%4)
  %6 = call i32 @testBooleanLogic(1, 0)
  %5 = alloca i32, align 4
  store i32 %6, i32* %5, align 4
  call void @println(i8* getelementptr inbounds ([20 x i8], [20 x i8]* @.str.6, i32 0, i32 0))
  %7 = alloca i32, align 4
  %8 = add i32 123, 0
  store i32 %8, i32* %7, align 4
  %9 = alloca i32*, align 4
  store i32 %9, i32* %9, align 4
  %10 = alloca i1, align 4
  store i32 %10, i32* %10, align 4
  call void @println(i8* getelementptr inbounds ([27 x i8], [27 x i8]* @.str.7, i32 0, i32 0))
  call void @println(i8* getelementptr inbounds ([29 x i8], [29 x i8]* @.str.8, i32 0, i32 0))
  ret i32 0
}

