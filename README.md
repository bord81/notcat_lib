# notcat_lib — NotCat Logging Client Library

`notcat_lib` is a cross-language client library for interacting with the [`notcatd`](https://github.com/your-org/notcatd) logging daemon. It enables structured logging to a Unix domain socket from **Rust**, **C**, and **Kotlin (via JNI)**, acting as a flexible drop-in alternative to traditional `logcat` APIs.

---

## 📦 Overview

This library provides:

- ✅ **Rust library**: Native async and sync logging interface.
- ✅ **C API**: Lightweight FFI bindings for use in native system components.
- ✅ **Kotlin/JNI bridge**: For use in Android apps or services needing native log streaming.

All variants communicate with `notcatd` over a SEQPACKET Unix domain socket (`/dev/socket/notcat_socket`).

---

## 🔧 Features

- 🔐 Log messages with priority levels (`Verbose`, `Debug`, `Info`, `Warn`, `Error`)
- ⏱️ Timestamped messages with priority and sink selection
- 📡 Lightweight protocol compatible with `notcatd`
- 🔀 Thread-safe design with optional async support in Rust

---

## 🧩 Components

| Component                | Description                           |
|--------------------------|---------------------------------------|
| `notcat_lib`             | Rust core client logic                |
| `notcat_lib_ffi_headers` | C FFI wrapper with simple logging API |
| `notcat_lib_ffi`         | C/JNI (Kotlin) bindings               |

---

## 📂 Examples

You can find usage examples for all supported languages in the [notcat_client_test](https://github.com/bord81/notcat_client_test).

---

## 🛡️ License

This project is licensed under the [MIT License](LICENSE).

© 2025 Borys Zakaliuk

---
