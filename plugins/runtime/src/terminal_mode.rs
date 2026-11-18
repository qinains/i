//! System terminal APIs only. No stty subprocess or additional crate is required.
use std::io;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod platform {
    use super::*;
    use std::ffi::{c_int, c_ulong, c_void};
    // Opaque, initialized, suitably aligned storage: Darwin termios is 72 bytes,
    // Linux termios is <= 128 bytes on supported ABIs. libc owns its layout.
    #[derive(Clone, Copy)]
    #[repr(C, align(16))]
    struct Attributes([u8; 256]);
    #[repr(C)]
    #[derive(Default)]
    struct Size {
        rows: u16,
        columns: u16,
        x: u16,
        y: u16,
    }
    unsafe extern "C" {
        fn tcgetattr(fd: c_int, termios: *mut c_void) -> c_int;
        fn tcsetattr(fd: c_int, action: c_int, termios: *const c_void) -> c_int;
        fn cfmakeraw(termios: *mut c_void);
        fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    }
    pub struct Mode(Attributes);
    impl Mode {
        pub fn enter() -> io::Result<Self> {
            let mut saved = Attributes([0; 256]);
            // SAFETY: storage exceeds/aligned for the platform termios and stays alive for each call.
            if unsafe { tcgetattr(0, (&mut saved as *mut Attributes).cast()) } != 0 {
                return Err(io::Error::last_os_error());
            }
            let guard = Self(saved);
            let mut raw = saved;
            unsafe {
                cfmakeraw((&mut raw as *mut Attributes).cast());
            }
            if unsafe { tcsetattr(0, 0, (&raw as *const Attributes).cast()) } != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(guard)
        }
    }
    impl Drop for Mode {
        fn drop(&mut self) {
            unsafe {
                tcsetattr(0, 0, (&self.0 as *const Attributes).cast());
            }
        }
    }
    pub fn size() -> (usize, usize) {
        let mut size = Size::default();
        let request = if cfg!(target_os = "macos") { 0x40087468 } else { 0x5413 };
        // SAFETY: TIOCGWINSZ writes the four u16 fields of winsize.
        let ok = unsafe { ioctl(2, request, &mut size as *mut Size) } == 0;
        if ok && size.columns > 0 && size.rows > 0 { (size.columns.into(), size.rows.into()) } else { (80, 24) }
    }
}
#[cfg(windows)]
mod platform {
    use super::*;
    use std::ffi::c_void;
    type Handle = *mut c_void;
    #[derive(Default)]
    #[repr(C)]
    struct Coord {
        x: i16,
        y: i16,
    }
    #[derive(Default)]
    #[repr(C)]
    struct Rect {
        left: i16,
        top: i16,
        right: i16,
        bottom: i16,
    }
    #[derive(Default)]
    #[repr(C)]
    struct Info {
        size: Coord,
        cursor: Coord,
        attributes: u16,
        window: Rect,
        maximum: Coord,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(id: u32) -> Handle;
        fn GetConsoleMode(handle: Handle, mode: *mut u32) -> i32;
        fn SetConsoleMode(handle: Handle, mode: u32) -> i32;
        fn GetConsoleScreenBufferInfo(handle: Handle, info: *mut Info) -> i32;
    }
    pub struct Mode {
        input: Handle,
        output: Handle,
        input_mode: u32,
        output_mode: u32,
    }
    impl Mode {
        pub fn enter() -> io::Result<Self> {
            // SAFETY: standard handles are borrowed, never closed; out-pointers have the Win32 ABI layout.
            let (input, output) = unsafe { (GetStdHandle(-10i32 as u32), GetStdHandle(-12i32 as u32)) };
            let (mut input_mode, mut output_mode) = (0, 0);
            if unsafe { GetConsoleMode(input, &mut input_mode) == 0 || GetConsoleMode(output, &mut output_mode) == 0 } {
                return Err(io::Error::last_os_error());
            }
            let guard = Self { input, output, input_mode, output_mode };
            // VT input; disable line editing/echo/Ctrl-C processing/QuickEdit. Keep other original modes.
            if unsafe {
                SetConsoleMode(input, (input_mode & !(1 | 2 | 4 | 0x40)) | 0x200 | 0x80) == 0
                    || SetConsoleMode(output, output_mode | 1 | 4) == 0
            } {
                return Err(io::Error::last_os_error());
            }
            Ok(guard)
        }
    }
    impl Drop for Mode {
        fn drop(&mut self) {
            unsafe {
                SetConsoleMode(self.input, self.input_mode);
                SetConsoleMode(self.output, self.output_mode);
            }
        }
    }
    pub fn size() -> (usize, usize) {
        let mut info = Info::default();
        let ok = unsafe { GetConsoleScreenBufferInfo(GetStdHandle(-12i32 as u32), &mut info) } != 0;
        if ok {
            (
                (i32::from(info.window.right) - i32::from(info.window.left) + 1).max(1) as usize,
                (i32::from(info.window.bottom) - i32::from(info.window.top) + 1).max(1) as usize,
            )
        } else {
            (80, 24)
        }
    }
}
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
mod platform {
    use super::*;
    pub struct Mode;
    impl Mode {
        pub fn enter() -> io::Result<Self> {
            Err(io::Error::new(io::ErrorKind::Unsupported, "terminal editor unsupported on this platform"))
        }
    }
    pub fn size() -> (usize, usize) {
        (80, 24)
    }
}
pub(crate) use platform::{Mode, size};
