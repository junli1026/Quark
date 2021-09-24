use core::sync::atomic::Ordering;

use super::super::qlib::mem::list_allocator::*;
use super::super::qlib::mutex::*;

impl OOMHandler for ListAllocator {
    fn handleError(&self, size:u64, alignment:u64) {
        super::super::Kernel::HostSpace::KernelOOM(size, alignment);
    }

    fn log(&self, a: u64, b: u64) {
        super::super::Kernel::HostSpace::KernelMsg(a, b);
    }
}

impl ListAllocator {
    pub fn initialize(&self)-> () {
        self.initialized.store(true, Ordering::Relaxed);
    }
}

impl<T: ?Sized> QMutex<T> {
    pub fn Log(&self, a: u64, b: u64) {
        super::super::Kernel::HostSpace::KernelMsg(a, b);
    }
}