pub use inner::Mmap;

use std::{io, ptr::NonNull};

#[cfg(not(test))]
mod inner {
    use libc::{
        MAP_FAILED, MAP_HUGETLB, MAP_POPULATE, MAP_SHARED, PROT_READ, PROT_WRITE,
    };
    use log::error;
    use std::{ffi::CString, os::fd::RawFd, ptr};

    use super::*;

    /// An anonymous memory mapped region.
    #[derive(Debug)]
    pub struct Mmap {
        addr: NonNull<libc::c_void>,
        len: usize,
        fd: RawFd,
    }

    unsafe impl Send for Mmap {}

    impl Mmap {
        pub fn new(len: usize, use_huge_pages: bool) -> io::Result<Self> {
            // A memfd gives the UMEM stable backing pages which can be mapped
            // at a different virtual address for each Wasm instance.
            let name = CString::new("xsk-umem").expect("static memfd name has no nul");
            let memfd_flags = if use_huge_pages {
                libc::MFD_CLOEXEC | libc::MFD_HUGETLB
            } else {
                libc::MFD_CLOEXEC
            };
            let fd = unsafe { libc::memfd_create(name.as_ptr(), memfd_flags) };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            let truncate_result = unsafe { libc::ftruncate(fd, len as libc::off_t) };
            if truncate_result != 0 {
                let error = io::Error::last_os_error();
                unsafe { libc::close(fd) };
                return Err(error);
            }

            let mut flags = MAP_SHARED | MAP_POPULATE;
            if use_huge_pages {
                flags |= MAP_HUGETLB;
            }

            let addr = unsafe {
                libc::mmap(
                    ptr::null_mut(),
                    len,
                    PROT_READ | PROT_WRITE,
                    flags,
                    fd,
                    0,
                )
            };

            if addr == MAP_FAILED {
                let error = io::Error::last_os_error();
                unsafe { libc::close(fd) };
                Err(error)
            } else {
                let addr =
                    NonNull::new(addr).expect("ptr non-null since we confirmed `mmap()` succeeded");

                Ok(Mmap { addr, len, fd })
            }
        }

        /// Returns a pointer to the start of the mmap'd region.
        #[inline]
        pub fn addr(&self) -> NonNull<libc::c_void> {
            self.addr
        }

        pub fn duplicate_fd(&self) -> io::Result<RawFd> {
            let fd = unsafe { libc::fcntl(self.fd, libc::F_DUPFD_CLOEXEC, 0) };
            if fd < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(fd)
            }
        }
    }

    impl Drop for Mmap {
        fn drop(&mut self) {
            let err = unsafe { libc::munmap(self.addr.as_ptr(), self.len) };

            if err != 0 {
                error!(
                    "`munmap()` failed with error: {}",
                    io::Error::last_os_error()
                );
            }
            unsafe { libc::close(self.fd) };
        }
    }
}

#[cfg(test)]
mod inner {
    use std::mem::ManuallyDrop;

    use super::*;

    #[derive(Debug)]
    struct VecParts<T> {
        ptr: NonNull<T>,
        len: usize,
        capacity: usize,
    }

    unsafe impl<T> Send for VecParts<T> {}

    impl<T> VecParts<T> {
        fn new(v: Vec<T>) -> Self {
            let mut v = ManuallyDrop::new(v);

            Self {
                ptr: NonNull::new(v.as_mut_ptr()).expect("obtained pointer from Vec"),
                len: v.len(),
                capacity: v.capacity(),
            }
        }
    }

    impl<T> Drop for VecParts<T> {
        fn drop(&mut self) {
            unsafe { Vec::from_raw_parts(self.ptr.as_ptr(), self.len, self.capacity) };
        }
    }

    /// A mocked [`Mmap`] that uses the heap for memory.
    #[derive(Debug)]
    pub struct Mmap(VecParts<u8>);

    impl Mmap {
        pub fn new(len: usize, _use_huge_pages: bool) -> io::Result<Self> {
            Ok(Self(VecParts::new(vec![0; len])))
        }

        /// Returns a pointer to the start of the mmap'd region.
        #[inline]
        pub fn addr(&self) -> NonNull<libc::c_void> {
            NonNull::new(self.0.ptr.as_ptr() as *mut libc::c_void).unwrap()
        }

        pub fn duplicate_fd(&self) -> io::Result<std::os::fd::RawFd> {
            Err(io::Error::new(io::ErrorKind::Unsupported, "test mmap has no fd"))
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn confirm_pointer_offset_is_a_single_byte() {
        assert_eq!(std::mem::size_of::<libc::c_void>(), 1);
    }
}
