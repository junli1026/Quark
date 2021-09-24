use libc;
use core::ptr;

use super::qlib::mem::list_allocator::*;
use super::qlib::mutex::*;


impl OOMHandler for ListAllocator {
    fn handleError(&self, _a:u64, _b:u64) {
        panic!("qvisor OOM: Heap allocator fails to allocate memory block");
    }

    fn log(&self, a: u64, b: u64) {
        error!("ListAllocator::Log {:x}/{:x}", a, b);
    }
}

impl ListAllocator {
    pub fn initialize(&self) {
        let address: *mut libc::c_void;
        unsafe {
            address = libc::mmap(ptr::null_mut(), 1<<29 as usize, libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANON, -1, 0);
            if address == libc::MAP_FAILED {
                panic!("mmap: failed to get mapped memory area for heap");
            }
            self.Add(address as usize, 1<<29 as usize);
        }
    }
}

impl<T: ?Sized> QMutex<T> {
    pub fn Log(&self, a: u64, b: u64) {
        error!("ListAllocator::Log {:x}/{:x}", a, b);
    }
}