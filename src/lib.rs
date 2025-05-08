use nix::libc::c_int;
use std::ffi::{CStr, CString};
use std::io::Write;
use std::os::{raw::c_char, unix::net::UnixStream};
use std::{
    io::{self},
    net::Shutdown,
    path::Path,
    ptr,
};
extern "C" {
    fn __android_log_write(prio: i32, tag: *const i8, msg: *const i8) -> i32;
}

#[allow(dead_code)]
#[repr(i32)]
pub enum AndroidLogPriority {
    Unknown = 0,
    Default = 1,
    Verbose = 2,
    Debug = 3,
    Info = 4,
    Warn = 5,
    Error = 6,
    Fatal = 7,
    Silent = 8,
}
#[allow(dead_code)]
fn log_android_native(prio: AndroidLogPriority, tag: &str, msg: &str) {
    let tag_c = match CString::new(tag) {
        Ok(c) => c,
        Err(_) => return,
    };
    let msg_c = match CString::new(msg) {
        Ok(c) => c,
        Err(n) => {
            let nul_position = n.nul_position();
            if nul_position == 0 {
                return;
            }
            let mut valid_part = n.into_vec();
            valid_part.truncate(nul_position);
            unsafe {
                let valid_string = CString::from_vec_unchecked(valid_part);
                valid_string
            }
        }
    };

    unsafe {
        __android_log_write(prio as i32, tag_c.as_ptr(), msg_c.as_ptr());
    }
}

pub struct NotCatClient {
    stream: UnixStream,
}

impl NotCatClient {
    pub fn connect<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let stream = UnixStream::connect(path)?;
        Ok(NotCatClient { stream })
    }

    pub fn log(&mut self, msg: &[u8]) -> io::Result<()> {
        self.stream.write_all(msg)
    }

    pub fn close(self) -> io::Result<()> {
        self.stream.shutdown(Shutdown::Both)
    }
}

#[repr(C)]
pub struct NotCatClientHandle {
    inner: NotCatClient,
}

#[no_mangle]
pub unsafe extern "C" fn notcat_connect(path: *const c_char) -> *mut NotCatClientHandle {
    if path.is_null() {
        return ptr::null_mut();
    }
    let c_str = unsafe { CStr::from_ptr(path) };
    let sock_path = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    match NotCatClient::connect(sock_path) {
        Ok(client) => {
            let handle = Box::new(NotCatClientHandle { inner: client });
            Box::into_raw(handle)
        }
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn notcat_log(
    handle: *mut NotCatClientHandle,
    message: *const c_char,
) -> c_int {
    if handle.is_null() || message.is_null() {
        return -1;
    }
    let client = unsafe { &mut (*handle).inner };
    let c_msg = unsafe { CStr::from_ptr(message) };
    let bytes = c_msg.to_bytes();
    match client.log(bytes) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

#[no_mangle]
pub unsafe extern "C" fn notcat_close(handle: *mut NotCatClientHandle) -> c_int {
    if handle.is_null() {
        return -1;
    }
    let boxed = unsafe { Box::from_raw(handle) };
    match boxed.inner.close() {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

//TODO: Add JNI bindings for the above functions
