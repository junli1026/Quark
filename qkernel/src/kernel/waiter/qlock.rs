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

//use spin::Mutex;
use core::ops::Deref;
use core::ops::DerefMut;
use core::cell::UnsafeCell;

use super::super::super::qlib::common::*;
use super::super::super::qlib::mutex::*;
use super::queue::*;
use super::*;

#[derive(Default)]
pub struct QLock <T: ?Sized> {
    pub locked: QMutex<bool>,
    pub queue: Queue,
    pub data: UnsafeCell<T>,
}

pub struct QLockGuard <'a, T: ?Sized + 'a> {
    pub lock: &'a QLock<T>
}

// Same unsafe impls as `std::sync::QMutex`
unsafe impl<T: ?Sized + Send> Sync for QLock<T> {}
unsafe impl<T: ?Sized + Send> Send for QLock<T> {}

impl <T> QLock <T> {
    pub fn New(data: T) -> Self {
        return Self {
            locked: QMutex::new(false),
            queue: Queue::default(),
            data: UnsafeCell::new(data),
        }
    }
}

impl <T: ?Sized> QLock <T> {
    pub fn Unlock(&self) {
        let mut l = self.locked.lock();
        assert!(*l == true, "QLock::Unlock misrun");
        *l = false;
        self.queue.Notify(!0);

        //self.queue.write().RemoveAll();
    }

    pub fn Lock(&self, task: &Task) -> Result<QLockGuard<T>> {
        let blocker = task.blocker.clone();

        loop {
            let block = {
                let mut l = self.locked.lock();
                if *l == false {
                    //fast path, got lock
                    *l = true;
                    false
                } else {
                    blocker.generalEntry.Clear();
                    self.queue.EventRegister(task, &blocker.generalEntry, 1);
                    true
                }
            };

            if !block {
                //fast path
                return Ok(QLockGuard {
                    lock: self,
                })
            }

            match blocker.BlockGeneral() {
                Err(e) => {
                    self.queue.EventUnregister(task, &blocker.generalEntry);
                    return Err(e)
                }
                Ok(()) => ()
            }

            self.queue.EventUnregister(task, &blocker.generalEntry);
        }
    }
}

impl <'a, T: ?Sized + 'a> Deref for QLockGuard <'a, T> {
    type Target = T;

    fn deref (&self) -> & T {
        let data = unsafe {
            &mut *self.lock.data.get()
        };
        & *data
    }
}

impl <'a, T: ?Sized + 'a> DerefMut for QLockGuard <'a, T> {
    fn deref_mut (&mut self) -> &mut T {
        let data = unsafe {
            &mut *self.lock.data.get()
        };
        &mut *data
    }
}

impl <'a, T: ?Sized + 'a> Drop for QLockGuard <'a, T> {
    fn drop(&mut self) {
        self.lock.Unlock();
    }
}