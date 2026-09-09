use std::ffi::CString;
use std::sync::{Arc, Mutex, Once};

use lxs_core::LxsError;
use x11::xlib;

static INIT_THREADS: Once = Once::new();

pub struct Display {
    ptr: *mut xlib::Display,
}

impl Display {
    pub fn open(name: &str) -> Result<Self, LxsError> {
        INIT_THREADS.call_once(|| unsafe {
            xlib::XInitThreads();
        });

        let cname = CString::new(name)
            .map_err(|_| LxsError::InvalidArgument("invalid display name".into()))?;
        let ptr = unsafe { xlib::XOpenDisplay(cname.as_ptr()) };
        if ptr.is_null() {
            return Err(LxsError::InvalidArgument(format!(
                "cannot open display {}",
                name
            )));
        }
        Ok(Self { ptr })
    }

    pub fn ptr(&self) -> *mut xlib::Display {
        self.ptr
    }
}

unsafe impl Send for Display {}
unsafe impl Sync for Display {}

impl Drop for Display {
    fn drop(&mut self) {
        unsafe {
            xlib::XCloseDisplay(self.ptr);
        }
    }
}

pub type DisplayHandle = Arc<Mutex<Display>>;

pub fn open_display(name: &str) -> Result<DisplayHandle, LxsError> {
    Ok(Arc::new(Mutex::new(Display::open(name)?)))
}
