use std::{
    ffi::{CStr, CString},
    io,
    mem::forget,
    os::unix::io::{AsRawFd, OwnedFd},
    ptr::NonNull,
};

/// DIR(3) structure.
pub struct DIR
{
    inner: NonNull<libc::DIR>,
}

// NOTE: Our DIR wrapper provides no interior mutability.
//       All wrapper functions must take `&mut DIR`!
unsafe impl Send for DIR { }
unsafe impl Sync for DIR { }

impl Drop for DIR
{
    fn drop(&mut self)
    {
        // SAFETY: self.inner is owning and not dangling.
        unsafe { libc::closedir(self.inner.as_ptr()); }
    }
}

/// dirent(3) structure.
#[allow(missing_docs, non_camel_case_types)]
pub struct dirent
{
    pub d_name: CString,
}

/// Call fdopendir(3) with the given arguments.
pub fn fdopendir(fd: OwnedFd) -> io::Result<DIR>
{
    // SAFETY: This is always safe.
    let dir = unsafe { libc::fdopendir(fd.as_raw_fd()) };

    if let Some(dir) = NonNull::new(dir) {
        forget(fd);  // fd is now owned by dir.
        Ok(DIR{inner: dir})
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Call readdir(3) with the given arguments.
pub fn readdir(dirp: &mut DIR) -> io::Result<Option<dirent>>
{
    // SAFETY: This is always safe.
    unsafe { *libc::__errno_location() = 0; }

    // SAFETY: dirp points to a valid DIR.
    let dirent = unsafe { libc::readdir(dirp.inner.as_ptr()) };

    // SAFETY: This is always safe.
    let errno = unsafe { *libc::__errno_location() };

    if dirent.is_null() && errno != 0 {
        return Err(io::Error::from_raw_os_error(errno));
    }

    if dirent.is_null() {
        return Ok(None);
    }

    // SAFETY: d_name is a NUL-terminated string.
    let d_name = unsafe { CStr::from_ptr((*dirent).d_name.as_ptr()) };
    let d_name = d_name.to_owned();

    Ok(Some(dirent{d_name}))
}
