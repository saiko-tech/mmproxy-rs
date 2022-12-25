use std::io;

pub const PIPE_BUF_SIZE: usize = 1 << 20;

#[derive(Debug)]
pub struct Pipe {
    pub r: i32,
    pub w: i32,
}

impl Pipe {
    pub fn new() -> io::Result<Self> {
        let pipes = unsafe {
            let mut pipes = std::mem::MaybeUninit::<[libc::c_int; 2]>::uninit();
            if libc::pipe2(
                pipes.as_mut_ptr().cast(),
                libc::O_NONBLOCK | libc::O_CLOEXEC,
            ) < 0
            {
                return Err(io::Error::last_os_error());
            }
            pipes.assume_init()
        };

        unsafe {
            if libc::fcntl(pipes[0], libc::F_SETPIPE_SZ, 1 << 20) < 0 {
                return Err(io::Error::last_os_error());
            }
        }

        Ok(Self {
            r: pipes[0],
            w: pipes[1],
        })
    }
}

impl Drop for Pipe {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.r);
            libc::close(self.w);
        }
    }
}

pub fn splice(r: i32, w: i32, n: usize) -> isize {
    unsafe {
        libc::splice(
            r,
            std::ptr::null_mut::<libc::loff_t>(),
            w,
            std::ptr::null_mut::<libc::loff_t>(),
            n,
            libc::SPLICE_F_MOVE | libc::SPLICE_F_NONBLOCK,
        )
    }
}

pub fn wouldblock() -> bool {
    let errno = unsafe { *libc::__errno_location() };
    errno == libc::EWOULDBLOCK || errno == libc::EAGAIN
}
