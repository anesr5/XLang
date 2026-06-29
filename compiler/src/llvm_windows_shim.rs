// The official LLVM Windows installer can ship LLVM-C.lib without the target
// initialization symbols referenced by Inkwell's target module. The MVP does
// not call these functions; clang owns native target selection. These shims let
// the Inkwell IR builder link against that installer layout.

#[unsafe(no_mangle)]
pub extern "C" fn LLVM_InitializeAllTargets() {}

#[unsafe(no_mangle)]
pub extern "C" fn LLVM_InitializeAllTargetInfos() {}

#[unsafe(no_mangle)]
pub extern "C" fn LLVM_InitializeAllAsmParsers() {}

#[unsafe(no_mangle)]
pub extern "C" fn LLVM_InitializeAllAsmPrinters() {}

#[unsafe(no_mangle)]
pub extern "C" fn LLVM_InitializeAllDisassemblers() {}

#[unsafe(no_mangle)]
pub extern "C" fn LLVM_InitializeAllTargetMCs() {}

#[unsafe(no_mangle)]
pub extern "C" fn LLVM_InitializeNativeTarget() -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn LLVM_InitializeNativeAsmPrinter() -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn LLVM_InitializeNativeAsmParser() -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn LLVM_InitializeNativeDisassembler() -> i32 {
    0
}
