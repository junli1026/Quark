// Copyright (c) 2021 Quark Container Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicBool, AtomicUsize};
use core::sync::atomic::Ordering;
use core::cmp::max;
use core::mem::size_of;
use core::ptr::NonNull;
//use spin::Mutex;
use buddy_system_allocator::Heap;

use super::super::range::Range;
use super::super::mutex::QMutex;

pub const CLASS_CNT : usize = 16;
pub const FREE_THRESHOLD: usize = 30; // when free size less than 30%, need to free buffer
pub const BUFF_THRESHOLD: usize = 50; // when buff size takes more than 50% of free size, needs to free
pub const FREE_BATCH: usize = 10; // free 10 blocks each time.
pub const ORDER : usize = 33;

pub struct ListAllocator {
    pub bufs: [QMutex<FreeMemBlockMgr>; CLASS_CNT],
    pub heap: QMutex<Heap<ORDER>>,
    pub total: AtomicUsize,
    pub free: AtomicUsize,
    pub bufSize: AtomicUsize,
    pub range: QMutex<Range>,
    //pub errorHandler: Arc<OOMHandler>
    pub initialized: AtomicBool
}

pub trait OOMHandler {
    fn handleError(&self, a:u64, b:u64);
    fn log(&self, a: u64, b: u64);
}

impl ListAllocator {
    pub const fn Empty() -> Self {
        let bufs : [QMutex<FreeMemBlockMgr>; CLASS_CNT] = [
            QMutex::new(FreeMemBlockMgr::New(0, 0)),
            QMutex::new(FreeMemBlockMgr::New(0, 1)),
            QMutex::new(FreeMemBlockMgr::New(0, 2)),
            QMutex::new(FreeMemBlockMgr::New(128, 3)),
            QMutex::new(FreeMemBlockMgr::New(128, 4)),
            QMutex::new(FreeMemBlockMgr::New(128, 5)),
            QMutex::new(FreeMemBlockMgr::New(64, 6)),
            QMutex::new(FreeMemBlockMgr::New(64, 7)),
            QMutex::new(FreeMemBlockMgr::New(64, 8)),
            QMutex::new(FreeMemBlockMgr::New(32, 9)),
            QMutex::new(FreeMemBlockMgr::New(32, 10)),
            QMutex::new(FreeMemBlockMgr::New(16, 11)),
            QMutex::new(FreeMemBlockMgr::New(1024, 12)),
            QMutex::new(FreeMemBlockMgr::New(16, 13)),
            QMutex::new(FreeMemBlockMgr::New(8, 14)),
            QMutex::new(FreeMemBlockMgr::New(8, 15))
        ];

        return Self {
            bufs: bufs,
            heap: QMutex::new(Heap::empty()),
            total: AtomicUsize::new(0),
            free: AtomicUsize::new(0),
            bufSize: AtomicUsize::new(0),
            range: QMutex::new(Range::New(0, 0)),
            initialized: AtomicBool::new(false)
        }
    }

    pub fn PrintAddr(&self) {
        error!("ListAllocator self {:x}, bufs {:x}", self as * const _ as u64, &self.bufs[0] as * const _ as u64);
    }

    pub fn AddToHead(&self, start: usize, end: usize) {
        unsafe {
            self.heap.lock().add_to_heap(start, end);
        }

        let size = end - start;
        self.total.fetch_add(size, Ordering::Release);
        self.free.fetch_add(size, Ordering::Release);
    }

    /// add the chunk of memory (start, start+size) to heap for allocating dynamic memory
    pub fn Add(&self, start: usize, size: usize) {
        *self.range.lock() = Range::New(start as u64, size as u64);

        let mut start = start;
        let end = start + size;
        let size = 1 << 30; // 1GB
        // note: we can't add full range (>4GB) to the buddyallocator
        while start + size < end {
            self.AddToHead(start, start + size);
            start  += size;
        }

        if start < end {
            self.AddToHead(start, end)
        }

        self.initialized.store(true, Ordering::Relaxed);
    }

    pub fn NeedFree(&self) -> bool {
        let total = self.total.load(Ordering::Acquire);
        let free = self.free.load(Ordering::Acquire);
        let bufSize = self.bufSize.load(Ordering::Acquire);

        if free > core::usize::MAX / 100 || total > core::usize::MAX / 100 {
            error!("total is {:x}, free is {:x}, buffsize is {:x}", total, free, bufSize);
        }

        if total * FREE_THRESHOLD / 100 > free && // there are too little free memory
            free * BUFF_THRESHOLD /100 < bufSize { // there are too much bufferred memory
            return true
        }

        return false
    }

    // ret: true: free some memory, false: no memory freed
    pub fn Free(&self) -> bool {
        let mut count = 0;
        for i in 0..self.bufs.len() {
            if !self.NeedFree() || count == FREE_BATCH {
                return count > 0
            }

            let idx = self.bufs.len() - i - 1; // free from larger size
            let cnt = self.bufs[idx].lock().FreeMultiple(&self.heap, FREE_BATCH - count);
            self.bufSize.fetch_sub(cnt * self.bufs[idx].lock().size, Ordering::Release);
            count += cnt;
        }

        return count > 0;
    }
}

unsafe impl GlobalAlloc for ListAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let initialized = self.initialized.load(Ordering::Relaxed);
        if !initialized {
            self.initialize();      
        }

        let size = max(
            layout.size().next_power_of_two(),
            max(layout.align(), size_of::<usize>()),
        );

        let class = size.trailing_zeros() as usize;

        if 3 <= class && class < self.bufs.len() {
            let ret = self.bufs[class].lock().Alloc();
            if ret.is_some() {
                self.bufSize.fetch_sub(size, Ordering::Release);
                return ret.unwrap();
            }
        }

        let ret = self
            .heap
            .lock()
            .alloc(layout)
            .ok()
            .map_or(0 as *mut u8, |allocation| allocation.as_ptr()) as u64;

        if ret == 0 {
            self.handleError(size as u64, layout.align() as u64);
            loop {}
        }

        // Subtract when ret != 0 to avoid overflow
        self.free.fetch_sub(size, Ordering::Release);

        return ret as *mut u8;
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let size = max(
            layout.size().next_power_of_two(),
            max(layout.align(), size_of::<usize>()),
        );
        let class = size.trailing_zeros() as usize;

        let addr = ptr as u64;
        let range = self.range.lock().clone();
        if !range.Contains(addr) || !range.Contains(addr + size as u64) {
            self.log(range.Start(), range.End());
            self.log(addr, size as u64);
            return
            //self.handleError(addr, size as u64);
        }

        self.free.fetch_add(size, Ordering::Release);
        self.bufSize.fetch_add(size, Ordering::Release);
        if class < self.bufs.len() {
            return self.bufs[class].lock().Dealloc(ptr, &self.heap);
        }

        self.heap.lock().dealloc(NonNull::new_unchecked(ptr), layout)
    }
}

/// FreeMemoryBlockMgr is used to manage heap memory block allocated by allocator
pub struct FreeMemBlockMgr {
    pub size: usize,
    pub count: usize,
    pub reserve: usize,
    pub list: MemList,
}

impl FreeMemBlockMgr {
    /// Return a newly created FreeMemBlockMgr
    /// # Arguments
    ///
    /// * `reserve` - number of clocks the Block Manager keeps for itself when free multiple is called.
    /// * `class` - denotes the block size this manager is in charge of. class i means the block is of size 2^i bytes
    pub const fn New(reserve: usize, class: usize) -> Self {
        return Self {
            size: 1<<class,
            reserve: reserve,
            count: 0,
            list: MemList::New(1<<class),
        }
    }

    pub fn Layout(&self) -> Layout {
        return Layout::from_size_align(self.size, self.size).unwrap();
    }

    pub fn Alloc(&mut self) -> Option<*mut u8> {
        if self.count > 0 {
            self.count -= 1;
            let ret = self.list.Pop();

            let ptr = ret as * mut MemBlock;
            unsafe {
                ptr.write(0)
            }
            return Some(ret as * mut u8)
        } else {
            return None
        }
    }

    pub fn Dealloc(&mut self, ptr: *mut u8, _heap: &QMutex<Heap<ORDER>>) {
        /*let size = self.size / 8;
        unsafe {
            let toArr = slice::from_raw_parts(ptr as *mut u64, size);
            for i in 0..size {
                assert!(toArr[i] == 0);
            }
        }*/

        self.count += 1;
        self.list.Push(ptr as u64);
    }

    fn Free(&mut self, heap: &QMutex<Heap<ORDER>>) {
        assert!(self.count > 0);
        self.count -= 1;
        let addr = self.list.Pop();

        unsafe {
            heap.lock().dealloc(NonNull::new_unchecked(addr as * mut u8), self.Layout());
        }
    }

    pub fn FreeMultiple(&mut self, heap: &QMutex<Heap<ORDER>>, count: usize) -> usize {
        for i in 0..count {
            if self.count <= self.reserve {
                return i;
            }

            self.Free(heap)
        }

        return count;
    }
}


type MemBlock = u64;


pub struct MemList {
    size: u64,
    head: MemBlock,
    tail: MemBlock,
}

impl MemList {
    pub const fn New(size: usize) -> Self {
        return Self {
            size: size as u64,
            head: 0,
            tail: 0,
        }
    }

    pub fn Push(&mut self, addr: u64) {
        assert!(addr % self.size == 0, "Push addr is {:x}/size is {:x}", addr, self.size);

        let newB = addr as * mut MemBlock;
        unsafe {
            *newB = 0;
        }

        if self.head == 0 {
            self.head = addr;
            self.tail = addr;
            return
        }

        let tail = self.tail;

        let ptr = tail as * mut MemBlock;
        unsafe {
            *ptr = addr;
        }
        self.tail = addr;
    }

    pub fn Pop(&mut self) -> u64 {
        if self.head == 0 {
            return 0
        }

        let next = self.head;

        if self.head == self.tail {
            self.head = 0;
            self.tail = 0;
            return next;
        }

        let ptr = unsafe {
            &mut *(next as * mut MemBlock)
        };

        self.head = *ptr;
        assert!(next % self.size == 0, "Pop next is {:x}/size is {:x}", next, self.size);
        return next;
    }
}