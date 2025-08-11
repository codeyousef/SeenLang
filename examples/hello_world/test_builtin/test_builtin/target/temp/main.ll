target triple = "x86_64-unknown-linux-gnu"
; Module: cli_build

define i32 @main() fastcc {
entry:
  %0 = call i32 @print(@.str)
  %1 = call i32 @debug(@.str)
  %2 = call i32 @assert(1)
  %3 = add i32 1, 1
  %4 = icmp eq i32 %3, 2
  %5 = call i32 @assert(%4)
  ret i32 0
}

define i32 @main() fastcc {
entry:
  %0 = call i32 @print(@.str)
  %1 = call i32 @debug(@.str)
  %2 = call i32 @assert(1)
  %3 = add i32 1, 1
  %4 = icmp eq i32 %3, 2
  %5 = call i32 @assert(%4)
  ret i32 0
}

