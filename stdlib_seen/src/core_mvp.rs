use std::ffi::CStr;
use std::os::raw::{c_char, c_int};

/// FFI bindings to the C standard library's printf function
extern "C" {
    fn printf(format: *const c_char, ...) -> c_int;
}

/// Seen language println function (English API name)
/// 
/// Prints a message to the standard output followed by a newline.
/// 
/// # Safety
///
/// This function is unsafe because it calls the C printf function.
#[no_mangle]
pub unsafe extern "C" fn println(text: *const c_char) -> c_int {
    // Create a format string with a newline
    let format = b"%s\n\0" as *const u8 as *const c_char;
    printf(format, text)
}

/// Seen language println function (Arabic API name)
/// 
/// Prints a message to the standard output followed by a newline.
/// This is the Arabic alias for the println function.
/// 
/// # Safety
///
/// This function is unsafe because it calls the C printf function.
#[no_mangle]
pub unsafe extern "C" fn اطبع(text: *const c_char) -> c_int {
    // Simply call the English implementation
    println(text)
}

/// Print an integer to standard output (English API name)
/// 
/// # Safety
///
/// This function is unsafe because it calls the C printf function.
#[no_mangle]
pub unsafe extern "C" fn println_int(value: i64) -> c_int {
    let format = b"%lld\n\0" as *const u8 as *const c_char;
    printf(format, value)
}

/// Print an integer to standard output (Arabic API name)
/// 
/// # Safety
///
/// This function is unsafe because it calls the C printf function.
#[no_mangle]
pub unsafe extern "C" fn اطبع_رقم(value: i64) -> c_int {
    println_int(value)
}

/// Print a floating-point number to standard output (English API name)
/// 
/// # Safety
///
/// This function is unsafe because it calls the C printf function.
#[no_mangle]
pub unsafe extern "C" fn println_float(value: f64) -> c_int {
    let format = b"%f\n\0" as *const u8 as *const c_char;
    printf(format, value)
}

/// Print a floating-point number to standard output (Arabic API name)
/// 
/// # Safety
///
/// This function is unsafe because it calls the C printf function.
#[no_mangle]
pub unsafe extern "C" fn اطبع_عدد(value: f64) -> c_int {
    println_float(value)
}

/// Print a boolean value to standard output (English API name)
/// 
/// # Safety
///
/// This function is unsafe because it calls the C printf function.
#[no_mangle]
pub unsafe extern "C" fn println_bool(value: bool) -> c_int {
    let text = if value {
        b"true\n\0" as *const u8 as *const c_char
    } else {
        b"false\n\0" as *const u8 as *const c_char
    };
    printf(text)
}

/// Print a boolean value to standard output (Arabic API name)
/// 
/// # Safety
///
/// This function is unsafe because it calls the C printf function.
#[no_mangle]
pub unsafe extern "C" fn اطبع_منطقي(value: bool) -> c_int {
    println_bool(value)
}
