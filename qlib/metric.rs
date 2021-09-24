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

use core::sync::atomic::AtomicU64;
use core::sync::atomic::Ordering;
//use spin::Mutex;
use alloc::sync::Arc;
use alloc::string::String;
use alloc::string::ToString;
use alloc::collections::btree_map::BTreeMap;

use super::mutex::*;

use lazy_static::lazy_static;

lazy_static! {
    pub static ref ALL_METRICS: QMutex<MetricSet> = QMutex::new(MetricSet::New());
}

pub fn NewU64Metric(name: &str, sync: bool, description: &str) -> Arc<U64Metric> {
    return ALL_METRICS.lock().RegisterU64Metric(name.to_string(), sync, description.to_string())
}

pub trait Metric: Send + Sync {
    fn Value(&self) -> u64;
}

pub struct U64Metric {
    val: AtomicU64,
}

impl Metric for U64Metric {
    fn Value(&self) -> u64 {
        return self.val.load(Ordering::SeqCst)
    }
}

impl U64Metric {
    pub fn New() -> Self {
        return Self {
            val: AtomicU64::new(0),
        }
    }

    pub fn Incr(&self) {
        self.val.fetch_add(1, Ordering::SeqCst);
    }

    pub fn IncrBy(&self, v: u64) {
        self.val.fetch_add(v, Ordering::SeqCst);
    }
}

pub struct MetricData {
    pub description: String,
    pub sync: bool,
    pub metric: Arc<Metric>,
}

pub struct MetricSet {
    pub m: BTreeMap<String, MetricData>
}

impl MetricSet {
    pub fn New() -> Self {
        return Self {
            m: BTreeMap::new(),
        }
    }

    pub fn RegisterU64Metric(&mut self, name: String, sync: bool, description: String) -> Arc<U64Metric> {
        if self.m.contains_key(&name) {
            panic!("Unable to create metric: {}", name);
        }

        let metric = Arc::new(U64Metric::New());
        let data = MetricData {
            description: description,
            sync: sync,
            metric: metric.clone(),
        };

        self.m.insert(name, data);
        return metric;
    }
}

