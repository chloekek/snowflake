//! Extra items for talking to the operating system.
//!
//! This crate provides a safe, low-level interface to the operating system.
//! The functions in this crate are named after their underlying system calls,
//! and their behavior is identical except for the differences listed below.
//! The exact semantics of each function can be found in their man pages.
//! This is in contrast with the std crate, which gives no such guarantees.
//! This is a trade-off against cross-platform compatibility.
//!
//! # Differences with underlying system calls
//!
//! Errors are reported using [`Result`] rather than
//! through `errno` and an arbitrary return value.
//!
//! Regular string arguments are accepted instead of NUL-terminated strings.
//! They are automatically made NUL-terminated by the wrapper functions.
//!
//! When a new file descriptor is created by one of the functions,
//! it is created with the `FD_CLOEXEC` bit set (atomically).
//! That is, the `*_CLOEXEC` flag is set implicitly by the wrapper functions.
//! This ensures no resources are leaked in a threaded program that forks.
//!
//! If the system call fails with `EINTR` (interrupted),
//! the wrapper function automatically retries it.
//!
//! [`Result`]: `std::io::Result`

#![feature(io_safety)]
#![warn(missing_docs)]

pub use {
    self::{fcntl::*, stdlib::*, sys_stat::*},
    libc::{O_CREAT, O_DIRECTORY, O_PATH, O_WRONLY},
};

use std::io::{self, ErrorKind::Interrupted};

mod fcntl;
mod stdlib;
mod sys_stat;

/// Call `f` until it no longer fails with `EINTR`.
fn retry_on_eintr<F, T>(mut f: F) -> io::Result<T>
    where F: FnMut() -> io::Result<T>
{
    loop {
        match f() {
            Err(err) if err.kind() == Interrupted => continue,
            result                                => return result,
        }
    }
}
