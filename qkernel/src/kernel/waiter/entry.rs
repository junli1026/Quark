// Copyright (c) 2021 Quark Container Authors / 2018 The gVisor Authors.
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

use alloc::sync::Arc;
//use spin::Mutex;
use core::ops::Deref;
use core::cell::*;

use super::super::futex::*;
use super::super::epoll::epoll_entry::*;
use super::super::fasync::*;
use super::super::super::qlib::mutex::*;
use super::waiter::*;
use super::*;

pub enum WaitContext {
    None,
    ThreadContext(RefCell<ThreadContext>),
    EpollContext(PollEntry),
    // use Arc instead of Weak as the Unregister will be called in the File Drop
    FileAsync(FileAsync),
}

impl Default for WaitContext {
    fn default() -> Self {
        return Self::None
    }
}

impl WaitContext {
    pub fn ThreadContext(&self) -> RefMut<ThreadContext> {
        match self {
            WaitContext::ThreadContext(c) => c.borrow_mut(),
            _ => panic!("WaitContext is not ThreadContext"),
        }
    }

    pub fn EpollContext(&self) -> PollEntry {
        match self {
            WaitContext::EpollContext(p) => p.clone(),

            _ => panic!("WaitContext is not EpollContext"),
        }
    }

    pub fn CallBack(&self) {
        match self {
            WaitContext::EpollContext(p) => {
                p.CallBack();
            },
            WaitContext::ThreadContext(t) => {
                let context = t.borrow_mut();
                context.waiter.Trigger(context.waiterID);
            }
            WaitContext::FileAsync(a) => {
                a.Callback();
            }
            _ => (),
        }
    }
}

pub struct ThreadContext {
    pub waiterID: WaiterID,
    pub waiter: Waiter,
    // Just for futex, tid is the thread ID for the waiter in case this is a PI mutex.
    pub tid: u32,
    pub key: Key,
}

#[derive(Default)]
pub struct EntryInternal {
    pub next: Option<WaitEntry>,
    pub prev: Option<WaitEntry>,
    pub mask: EventMask,

    pub context: WaitContext,
}

#[derive(Default, Clone)]
pub struct WaitEntry(Arc<QMutex<EntryInternal>>);

impl Deref for WaitEntry {
    type Target = Arc<QMutex<EntryInternal>>;

    fn deref(&self) -> &Arc<QMutex<EntryInternal>> {
        &self.0
    }
}

impl PartialEq for WaitEntry {
    fn eq(&self, other: &Self) -> bool {
        return Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for WaitEntry {}

impl WaitEntry {
    pub fn New() -> Self {
        let internal = EntryInternal {
            next: None,
            prev: None,
            mask: 0,
            context: WaitContext::None,
        };

        return Self(Arc::new(QMutex::new(internal)))
    }

    pub fn Timeout(&self) {
        self.Notify(1);
    }

    pub fn NewThreadContext(waiter: &Waiter, waiterId: WaiterID, mask: EventMask) -> Self {
        let context = ThreadContext {
            waiterID: waiterId,
            waiter: waiter.clone(),
            tid: 0,
            key: Key::default(),
        };

        let internal = EntryInternal {
            next: None,
            prev: None,
            mask: mask,
            context: WaitContext::ThreadContext(RefCell::new(context)),
        };

        return Self(Arc::new(QMutex::new(internal)))
    }

    pub fn ID(&self) -> WaiterID {
        return self.lock().context.ThreadContext().waiterID;
    }

    pub fn SetMask(&self, mask: EventMask) {
        self.lock().mask = mask;
    }

    pub fn SetTid(&self, tid: u32) {
        self.lock().context.ThreadContext().tid = tid;
    }

    pub fn SetKey(&self, key: &Key) {
        self.lock().context.ThreadContext().key = *key;
    }

    pub fn Notify(&self, mask: EventMask) -> bool {
        let e = self.lock();
        if mask & e.mask != 0 {
            e.context.CallBack();
            return true
        }

        return false;
    }

    //clear the related bit of the entry in the waiter
    pub fn Clear(&self) {
        let e = self.lock();
        let context = e.context.ThreadContext();
        context.waiter.Clear(context.waiterID);
    }

    pub fn Reset(&self) {
        self.lock().prev = None;
        self.lock().next = None;
    }

    pub fn InitState(&self) -> bool {
        let s = self.lock();
        return s.prev == None && s.next == None;
    }

    pub fn Mask(&self) -> EventMask {
        return self.lock().mask;
    }
}
