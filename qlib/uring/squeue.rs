//! Submission Queue

use core::sync::atomic;

use super::sys;
use super::util::{unsync_load, Mmap};
use super::super::linux_def::QOrdering;

use bitflags::bitflags;

pub struct SubmissionQueue {
    pub(crate) head: *const atomic::AtomicU32,
    pub(crate) tail: *const atomic::AtomicU32,
    pub(crate) ring_mask: *const u32,
    pub(crate) ring_entries: *const u32,
    pub(crate) flags: *const atomic::AtomicU32,
    dropped: *const atomic::AtomicU32,

    pub(crate) sqes: *mut sys::io_uring_sqe,
}

impl Default for SubmissionQueue {
    fn default() -> Self {
        return Self {
            head: 0 as *const atomic::AtomicU32,
            tail: 0 as *const atomic::AtomicU32,
            ring_mask: 0 as *const u32,
            ring_entries: 0 as *const u32,
            sqes: 0 as *mut sys::io_uring_sqe,
            dropped: 0 as *const atomic::AtomicU32,
            flags: 0 as *const atomic::AtomicU32,
        }
    }
}

impl SubmissionQueue {
    pub fn CopyTo(&self, to: &mut Self) {
        to.head = self.head;
        to.tail = self.tail;
        to.ring_mask = self.ring_mask;
        to.ring_entries = self.ring_entries;
        to.flags = self.flags;
        to.dropped = self.dropped;
        to.sqes = self.sqes;
    }
}

pub struct AvailableQueue<'a> {
    head: u32,
    tail: u32,
    ring_mask: u32,
    ring_entries: u32,
    queue: &'a mut SubmissionQueue,
}

/// Submission Entry
#[repr(transparent)]
#[derive(Clone)]
pub struct Entry(pub(crate) sys::io_uring_sqe);

bitflags! {
    /// Submission flags
    pub struct Flags: u8 {
        /// When this flag is specified,
        /// `fd` is an index into the files array registered with the io_uring instance.
        #[doc(hidden)]
        const FIXED_FILE = 1 << sys::IOSQE_FIXED_FILE_BIT;

        /// When this flag is specified,
        /// the SQE will not be started before previously submitted SQEs have completed,
        /// and new SQEs will not be started before this one completes.
        const IO_DRAIN = 1 << sys::IOSQE_IO_DRAIN_BIT;

        /// When this flag is specified,
        /// it forms a link with the next SQE in the submission ring.
        /// That next SQE will not be started before this one completes.
        const IO_LINK = 1 << sys::IOSQE_IO_LINK_BIT;

        /// Like [IO_LINK], but it doesn’t sever regardless of the completion result.
        const IO_HARDLINK = 1 << sys::IOSQE_IO_HARDLINK_BIT;

        /// Normal operation for io_uring is to try and issue an sqe as non-blocking first,
        /// and if that fails, execute it in an async manner.
        ///
        /// To support more efficient overlapped operation of requests
        /// that the application knows/assumes will always (or most of the time) block,
        /// the application can ask for an sqe to be issued async from the start.
        const ASYNC = 1 << sys::IOSQE_ASYNC_BIT;

        #[cfg(feature = "unstable")]
        const BUFFER_SELECT = 1 << sys::IOSQE_BUFFER_SELECT_BIT;
    }
}

impl SubmissionQueue {
    pub(crate) unsafe fn new(
        sq_mmap: &Mmap,
        sqe_mmap: &Mmap,
        p: &sys::io_uring_params,
    ) -> SubmissionQueue {
        mmap_offset! {
            let head            = sq_mmap + p.sq_off.head           => *const atomic::AtomicU32;
            let tail            = sq_mmap + p.sq_off.tail           => *const atomic::AtomicU32;
            let ring_mask       = sq_mmap + p.sq_off.ring_mask      => *const u32;
            let ring_entries    = sq_mmap + p.sq_off.ring_entries   => *const u32;
            let flags           = sq_mmap + p.sq_off.flags          => *const atomic::AtomicU32;
            let dropped         = sq_mmap + p.sq_off.dropped        => *const atomic::AtomicU32;
            let array           = sq_mmap + p.sq_off.array          => *mut u32;

            let sqes            = sqe_mmap + 0                      => *mut sys::io_uring_sqe;
        }

        // To keep it simple, map it directly to `sqes`.
        for i in 0..*ring_entries {
            array.add(i as usize).write_volatile(i);
        }

        SubmissionQueue {
            head,
            tail,
            ring_mask,
            ring_entries,
            flags,
            dropped,
            sqes,
        }
    }

    pub fn need_wakeup(&self) -> bool {
        unsafe { (*self.flags).load(QOrdering::ACQUIRE) & sys::IORING_SQ_NEED_WAKEUP != 0 }
    }

    pub fn dropped(&self) -> u32 {
        unsafe { (*self.dropped).load(QOrdering::ACQUIRE) }
    }

    #[cfg(feature = "unstable")]
    pub fn cq_overflow(&self) -> bool {
        unsafe { (*self.flags).load(QOrdering::ACQUIRE) & sys::IORING_SQ_CQ_OVERFLOW != 0 }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        unsafe { self.ring_entries.read() as usize }
    }

    #[inline]
    pub fn len(&self) -> usize {
        unsafe {
            let head = (*self.head).load(QOrdering::ACQUIRE);
            let tail = unsync_load(self.tail);

            tail.wrapping_sub(head) as usize
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }

    /// Get currently available submission queue
    pub fn available(&mut self) -> AvailableQueue<'_> {
        unsafe {
            AvailableQueue {
                head: (*self.head).load(QOrdering::ACQUIRE),
                tail: unsync_load(self.tail),
                ring_mask: self.ring_mask.read(),
                ring_entries: self.ring_entries.read(),
                queue: self,
            }
        }
    }
}

impl AvailableQueue<'_> {
    /// Sync queue
    pub fn sync(&mut self) {
        unsafe {
            (*self.queue.tail).store(self.tail, QOrdering::RELEASE);
            self.head = (*self.queue.head).load(QOrdering::ACQUIRE);
        }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.ring_entries as usize
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.tail.wrapping_sub(self.head) as usize
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.head == self.tail
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        self.tail.wrapping_sub(self.head) == self.ring_entries
    }

    /// Attempts to push an [Entry] into the queue.
    /// If the queue is full, the element is returned back as an error.
    ///
    /// # Safety
    ///
    /// Developers must ensure that parameters of the [Entry] (such as buffer) are valid,
    /// otherwise it may cause memory problems.
    pub unsafe fn push(&mut self, Entry(entry): Entry) -> Result<(), Entry> {
        if !self.is_full() {
            let idx = (self.tail & self.ring_mask) as usize;
            *self.queue.sqes.add(idx) = entry;
            self.tail = self.tail.wrapping_add(1);
            Ok(())
        } else {
            Err(Entry(entry))
        }
    }
}

impl Drop for AvailableQueue<'_> {
    fn drop(&mut self) {
        unsafe {
            (*self.queue.tail).store(self.tail, QOrdering::RELEASE);
        }
    }
}

impl Entry {
    /// Set [Submission flags](Flags)
    pub fn flags(mut self, flags: Flags) -> Entry {
        self.0.flags |= flags.bits();
        self
    }

    /// The `user_data` is an application-supplied value that will be copied into the completion queue
    /// entry (see below).
    pub fn user_data(mut self, user_data: u64) -> Entry {
        self.0.user_data = user_data;
        self
    }
}
