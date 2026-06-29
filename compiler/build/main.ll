; ModuleID = 'xlang'
source_filename = "xlang"
target triple = "x86_64-pc-windows-msvc"

define i32 @add(i32 %a, i32 %b) {
entry:
  %a.addr = alloca i32, align 4
  store i32 %a, ptr %a.addr, align 4
  %b.addr = alloca i32, align 4
  store i32 %b, ptr %b.addr, align 4
  %a.load = load i32, ptr %a.addr, align 4
  %b.load = load i32, ptr %b.addr, align 4
  %addtmp = add i32 %a.load, %b.load
  ret i32 %addtmp
}

define i32 @main() {
entry:
  %calltmp = call i32 @add(i32 40, i32 2)
  %x = alloca i32, align 4
  store i32 %calltmp, ptr %x, align 4
  %x.load = load i32, ptr %x, align 4
  ret i32 %x.load
}
