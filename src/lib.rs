use nix::libc::c_int;
use nix::sys::socket::{AddressFamily, SockFlag, SockType, UnixAddr, connect, socket};
use std::ffi::{CStr, CString};
use std::io::Write;
use std::os::{fd::FromRawFd, fd::IntoRawFd, fd::RawFd, raw::c_char, unix::net::UnixStream};
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

static CONN_MAGIC: u32 = 0xb05acafe;

pub struct NotCatClient {
    stream: UnixStream,
}

impl NotCatClient {
    pub fn connect<P: AsRef<Path> + std::convert::AsRef<std::ffi::OsStr>>(
        path: P,
    ) -> io::Result<Self> {
        // TODO: add logging for I/O errors
        let owned_fd = socket(
            AddressFamily::Unix,
            SockType::SeqPacket,
            SockFlag::empty(),
            None,
        )
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let fd: RawFd = owned_fd.into_raw_fd();

        let addr =
            UnixAddr::new(Path::new(&path)).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        connect(fd, &addr).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let mut payload = Vec::with_capacity(10);
        payload.extend_from_slice(&CONN_MAGIC.to_be_bytes());
        payload.push(1); // version 1
        let mut stream = unsafe { UnixStream::from_raw_fd(fd) };
        let pid = unsafe { libc::getpid() } as u32;
        payload.extend_from_slice(&pid.to_be_bytes());
        let sink_type: u8 = 3; // LocalFileSink and AndroidNativeSink
        payload.push(sink_type);
        stream
            .write(&payload)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(NotCatClient { stream })
    }

    pub fn log(&mut self, msg: &[u8]) -> io::Result<()> {
        let mut payload = Vec::with_capacity(4 + msg.len());
        payload.extend_from_slice(&(msg.len() as u32).to_be_bytes());
        payload.extend_from_slice(msg);
        self.stream.write_all(&payload)
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

extern crate jni;

#[cfg(target_os = "android")]
mod notcat_jni {
    use super::NotCatClient;
    use super::jni::JNIEnv;
    use jni::objects::{JClass, JString};
    use jni::sys::{jint, jlong};

    fn jstring_to_string(env: &JNIEnv, js: JString) -> Option<String> {
        env.get_string(js).ok().map(|s| s.into())
    }

    // Methods signature correspond to top level Kotlin functions
    // Java signature suppport implies class name instead of file name
    #[no_mangle]
    pub extern "system" fn Java_com_notcat_NotCatClientKt_nativeConnect(
        env: JNIEnv,
        _class: JClass,
        jpath: JString,
    ) -> jlong {
        let path = match jstring_to_string(&env, jpath) {
            Some(s) => s,
            None => return 0,
        };
        match NotCatClient::connect(path) {
            Ok(client) => {
                let boxed = Box::new(client);
                Box::into_raw(boxed) as jlong
            }
            Err(_) => 0,
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_notcat_NotCatClientKt_nativeLog(
        env: JNIEnv,
        _class: JClass,
        handle: jlong,
        jmsg: JString,
    ) -> jint {
        if handle == 0 {
            return -1;
        }
        let client: &mut NotCatClient = unsafe { &mut *(handle as *mut NotCatClient) };
        let msg = match jstring_to_string(&env, jmsg) {
            Some(s) => s.into_bytes(),
            None => return -1,
        };
        match client.log(&msg) {
            Ok(()) => 0,
            Err(_) => -1,
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_notcat_NotCatClientKt_nativeClose(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
    ) -> jint {
        if handle == 0 {
            return -1;
        }
        let boxed: Box<NotCatClient> = unsafe { Box::from_raw(handle as *mut NotCatClient) };
        match boxed.close() {
            Ok(()) => 0,
            Err(_) => -1,
        }
    }
}
