use chrono::Datelike;
use chrono::Timelike;
use chrono::Utc;
use nix::errno::Errno;
use nix::libc::c_int;
use nix::sys::socket::{
    AddressFamily, MsgFlags, SockFlag, SockType, UnixAddr, connect, send, socket,
};
use std::ffi::{CStr, CString, c_char};
use std::os::{fd::IntoRawFd, fd::RawFd};
use std::sync::{Condvar, Mutex, RwLock};
use std::thread;
use std::time::Duration;
use std::{
    io::{self},
    path::Path,
};
extern "C" {
    fn __android_log_write(prio: i32, tag: *const c_char, msg: *const c_char) -> i32;
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

static FD_SERVER: RwLock<RawFd> = RwLock::new(-1);

static SERVER_LOST: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

static CONN_MAGIC: u32 = 0xb05acafe;

static SERVER_SOCKET: &str = "/dev/socket/notcat_socket";

#[repr(i32)]
pub enum LogPriority {
    Verbose = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

pub static LOCAL_FILE_SINK: u8 = 1;
pub static ANDROID_LOGCAT_SINK: u8 = 2;

pub fn log_init(sink_type: u8) -> io::Result<()> {
    // TODO: add logging for I/O errors
    let owned_fd = socket(
        AddressFamily::Unix,
        SockType::SeqPacket,
        SockFlag::empty(),
        None,
    )
    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    {
        let mut fd_server = FD_SERVER.write().map_err(|_| {
            io::Error::new(
                io::ErrorKind::Other,
                "Failed to acquire write lock on FD_SERVER",
            )
        })?;
        if *fd_server != -1 {
            return Ok(()); // Already initialized
        } else {
            *fd_server = owned_fd.into_raw_fd();
        }
    }

    let addr = UnixAddr::new(Path::new(SERVER_SOCKET))
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    {
        let fd_server = FD_SERVER.read().map_err(|_| {
            io::Error::new(
                io::ErrorKind::Other,
                "Failed to acquire read lock on FD_SERVER",
            )
        })?;
        connect(*fd_server, &addr).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    }

    let mut payload = Vec::with_capacity(10);
    payload.extend_from_slice(&CONN_MAGIC.to_be_bytes());
    payload.push(1); // version 1
    let pid = unsafe { libc::getpid() } as u32;
    payload.extend_from_slice(&pid.to_be_bytes());
    payload.push(sink_type);
    {
        let fd_server = FD_SERVER.read().map_err(|_| {
            io::Error::new(
                io::ErrorKind::Other,
                "Failed to acquire read lock on FD_SERVER",
            )
        })?;
        send(
            *fd_server,
            &payload,
            MsgFlags::MSG_DONTWAIT | MsgFlags::MSG_NOSIGNAL,
        )?;
    }
    thread::spawn(move || {
        loop {
            let (lock, cvar) = &SERVER_LOST;
            let mut server_lost = lock.lock().unwrap();
            if *server_lost {
                let owned_fd = socket(
                    AddressFamily::Unix,
                    SockType::SeqPacket,
                    SockFlag::empty(),
                    None,
                )
                .unwrap();
                loop {
                    match FD_SERVER.try_write() {
                        Ok(mut fd_server) => {
                            *fd_server = owned_fd.into_raw_fd();
                            break;
                        }
                        Err(_) => {
                            thread::sleep(Duration::from_millis(10));
                        }
                    }
                }
                let addr = UnixAddr::new(Path::new(SERVER_SOCKET)).unwrap();
                loop {
                    match FD_SERVER.try_read() {
                        Ok(fd_server) => {
                            match connect(*fd_server, &addr) {
                                Ok(_) => {
                                    *server_lost = false;
                                    let mut payload = Vec::with_capacity(10);
                                    payload.extend_from_slice(&CONN_MAGIC.to_be_bytes());
                                    payload.push(1); // version 1
                                    let pid = unsafe { libc::getpid() } as u32;
                                    payload.extend_from_slice(&pid.to_be_bytes());
                                    payload.push(sink_type);
                                    send(
                                        *fd_server,
                                        &payload,
                                        MsgFlags::MSG_DONTWAIT | MsgFlags::MSG_NOSIGNAL,
                                    )
                                    .unwrap();
                                    break;
                                }
                                Err(_) => {
                                    thread::sleep(Duration::from_millis(100));
                                }
                            }
                        }
                        Err(_) => {
                            thread::sleep(Duration::from_millis(10));
                        }
                    }
                }
                drop(server_lost);
                continue;
            }
            drop(cvar.wait(server_lost).unwrap());
        }
    });
    Ok(())
}

pub fn log(priority: LogPriority, tag: &[u8], msg: &[u8]) {
    let payload_len = tag.len() + 1 + msg.len();
    let mut payload = Vec::with_capacity(14 + payload_len);
    payload.extend_from_slice(&(payload_len as u32).to_be_bytes());
    payload.extend_from_slice(&(priority as u8).to_be_bytes());
    payload.extend_from_slice(&get_timestamp_bytes());
    payload.extend_from_slice(tag);
    payload.push(0x20 as u8); // space between tag and message
    payload.extend_from_slice(msg);
    {
        //TODO: add error handling for locking to add stability
        let fd_server = FD_SERVER.read().unwrap();
        if let Err(e) = send(
            *fd_server,
            &payload,
            MsgFlags::MSG_DONTWAIT | MsgFlags::MSG_NOSIGNAL,
        ) {
            match e {
                Errno::EPIPE | Errno::ENOTCONN => {
                    log_android_native(
                        AndroidLogPriority::Error,
                        "NotCatClient",
                        "Connection to NotCat server lost",
                    );
                    let (lock, cvar) = &SERVER_LOST;
                    let mut server_lost = lock.lock().unwrap();
                    *server_lost = true;
                    cvar.notify_all();
                }
                _ => {
                    log_android_native(
                        AndroidLogPriority::Verbose,
                        "NotCatClient",
                        &format!("Failed to send log message: {}", e),
                    );
                }
            }
        }
    }
}

pub fn close() -> io::Result<()> {
    Ok(())
}

fn get_timestamp_bytes() -> [u8; 9] {
    let now = Utc::now();
    let year = now.year() as u16;
    let month = now.month() as u8;
    let day = now.day() as u8;
    let hour = now.hour() as u8;
    let minute = now.minute() as u8;
    let second = now.second() as u8;
    let millisecond = now.timestamp_subsec_millis() as u16;

    let mut buf = [0u8; 9];
    buf[0..2].copy_from_slice(&year.to_be_bytes());
    buf[2] = month;
    buf[3] = day;
    buf[4] = hour;
    buf[5] = minute;
    buf[6] = second;
    buf[7..9].copy_from_slice(&millisecond.to_be_bytes());
    buf
}

#[no_mangle]
pub unsafe extern "C" fn notcat_init(sink_type: u8) -> c_int {
    match log_init(sink_type) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

#[no_mangle]
pub unsafe extern "C" fn notcat_log(priority: c_int, tag: *const c_char, message: *const c_char) {
    if tag.is_null() || message.is_null() {
        return;
    }
    let tag_str = unsafe { CStr::from_ptr(tag) };
    let tag_bytes = tag_str.to_bytes();
    let c_msg = unsafe { CStr::from_ptr(message) };
    let bytes = c_msg.to_bytes();
    let priority = match priority {
        0 => LogPriority::Verbose,
        1 => LogPriority::Debug,
        2 => LogPriority::Info,
        3 => LogPriority::Warn,
        4 => LogPriority::Error,
        _ => LogPriority::Verbose,
    };
    log(priority, tag_bytes, bytes);
}

#[no_mangle]
pub unsafe extern "C" fn notcat_close() -> c_int {
    match close() {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

extern crate jni;

#[cfg(target_os = "android")]
mod notcat_jni {
    use super::jni::JNIEnv;
    use crate::LogPriority;
    use crate::close;
    use crate::log;
    use crate::log_init;
    use jni::objects::{JClass, JString};
    use jni::sys::jint;

    fn jstring_to_string(env: &JNIEnv, js: JString) -> Option<String> {
        env.get_string(js).ok().map(|s| s.into())
    }

    // Methods signature correspond to top level Kotlin functions
    // Java signature suppport implies class name instead of file name
    #[no_mangle]
    pub extern "system" fn Java_com_notcat_NotCatClientKt_nativeInit(
        _env: JNIEnv,
        _class: JClass,
        sink_type: jint,
    ) -> jint {
        match log_init(sink_type as u8) {
            Ok(_) => 0,
            Err(_) => -1,
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_notcat_NotCatClientKt_nativeLog(
        env: JNIEnv,
        _class: JClass,
        priority: jint,
        jtag: JString,
        jmsg: JString,
    ) {
        let tag = match jstring_to_string(&env, jtag) {
            Some(s) => s.into_bytes(),
            None => return,
        };
        let msg = match jstring_to_string(&env, jmsg) {
            Some(s) => s.into_bytes(),
            None => return,
        };
        let priority = match priority {
            0 => LogPriority::Verbose,
            1 => LogPriority::Debug,
            2 => LogPriority::Info,
            3 => LogPriority::Warn,
            4 => LogPriority::Error,
            _ => LogPriority::Verbose,
        };
        log(priority, &tag, &msg);
    }

    #[no_mangle]
    pub extern "system" fn Java_com_notcat_NotCatClientKt_nativeClose(
        _env: JNIEnv,
        _class: JClass,
    ) -> jint {
        match close() {
            Ok(()) => 0,
            Err(_) => -1,
        }
    }
}
