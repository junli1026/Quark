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

use alloc::string::String;
use lazy_static::lazy_static;

use super::task::*;
use super::qlib::vcpu_mgr::*;
use super::qlib::config::DebugLevel;
use super::asm::*;

pub const SCALE : i64 = 2_000;
pub const ERROR : DebugLevel = DebugLevel::Error;
pub const INFO : DebugLevel = DebugLevel::Info;
pub const DEBUG : DebugLevel = DebugLevel::Debug;

lazy_static! {
    pub static ref DEBUG_LEVEL: DebugLevel = super::SHARESPACE.config.DebugLevel;
}

pub fn PrintPrefix() -> String {
    //super::PrintData1(0x8888);
    let now = if super::SHARESPACE.config.PerfDebug {
        Rdtsc()/SCALE
    } else {
        0
    };

    //super::task::Task::Current().Check();

    return format!("[{}/{:x}|{}]", CPULocal::CpuId() , Task::TaskId().Addr(), now);
}

pub fn DebugLevel() -> DebugLevel {
    //super::PrintData1(0x8884);
    let level = super::SHARESPACE.config.DebugLevel;
    //super::PrintData1(0x8885);
    return level;
}

#[macro_export]
macro_rules! raw_print {
    ($($arg:tt)*) => ({
        if $crate::SHARESPACE.config.DebugLevel >= $crate::qlib::config::DebugLevel::Error {
            //$crate::qlib::perf_tunning::PerfGoto($crate::qlib::perf_tunning::PerfType::Print);
            let s = &format!($($arg)*);
            let str = format!("[Print] {}", s);

            $crate::Kernel::HostSpace::SlowPrint($crate::qlib::config::DebugLevel::Error, &str);
            //$crate::qlib::perf_tunning::PerfGofrom($crate::qlib::perf_tunning::PerfType::Print);
        }
    });
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ({
        let current = $crate::print::ERROR;
        let level = *$crate::print::DEBUG_LEVEL;
        let cmp = level >= current;

        if cmp {
            //$crate::qlib::perf_tunning::PerfGoto($crate::qlib::perf_tunning::PerfType::Print);
            let prefix = $crate::print::PrintPrefix();
            let s = &format!($($arg)*);
            let str = format!("[Print] {} {}", prefix, s);

            $crate::Kernel::HostSpace::SlowPrint($crate::qlib::config::DebugLevel::Error, &str);
            //$crate::qlib::perf_tunning::PerfGofrom($crate::qlib::perf_tunning::PerfType::Print);
        }
    });
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => ({
        //$crate::PrintData(0x8889);

        // the repro will change the value of the address 0x43c41efd78 from 0 to 40000ba879
        // the log repro piece will be as below

        /*
            [ERROR] [1/43c41f0000|0] Lookup 4 name dev/HostInodeOp
            [INFO] [1] get kernel msg [rsp 43c41f24b8]: 221, 43c41efd78, 0
            [INFO] [1] get kernel msg [rsp 43c41f1978]: 222, 43c41efd78, 0
            [INFO] [1] get kernel msg [rsp 43c41f1978]: 8889, 43c41efd78, 0
            [INFO] [1] get kernel msg [rsp 43c41f1978]: 8886, 43c41efd78, 40000bbba9
            [INFO] [1] get kernel msg [rsp 43c41f1968]: 8887, 43c41efd78, 40000bbba9
        */

        let repro = true;
        let cmp;

        if !repro {
            let current = $crate::print::ERROR;
            let level = *$crate::print::DEBUG_LEVEL;
            cmp = level >= current;
        } else {
            cmp = $crate::SHARESPACE.config.DebugLevel >= $crate::qlib::config::DebugLevel::Error;
        }

        //$crate::PrintData(0x8886);

        if cmp {
        //if $crate::SHARESPACE.config.DebugLevel >= $crate::qlib::config::DebugLevel::Error {
            //$crate::qlib::perf_tunning::PerfGoto($crate::qlib::perf_tunning::PerfType::Print);
            //$crate::PrintData1(0x8887);
            let prefix = $crate::print::PrintPrefix();
            let s = &format!($($arg)*);

            if $crate::SHARESPACE.config.SlowPrint {
                let str = format!("[ERROR] {} {}", prefix, s);
                $crate::Kernel::HostSpace::SlowPrint($crate::qlib::config::DebugLevel::Error, &str);
            } else {
                let str = format!("[ERROR] {} {}\n", prefix, s);
                $crate::Kernel::HostSpace::Kprint(&str);
            }

            //$crate::qlib::perf_tunning::PerfGofrom($crate::qlib::perf_tunning::PerfType::Print);
        }
    });
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => ({
        let current = $crate::print::INFO;
        let level = *$crate::print::DEBUG_LEVEL;
        let cmp = level >= current;

        if cmp  {
            //$crate::qlib::perf_tunning::PerfGoto($crate::qlib::perf_tunning::PerfType::Print);
            let prefix = $crate::print::PrintPrefix();
            let s = &format!($($arg)*);

            if $crate::SHARESPACE.config.SlowPrint {
                let str = format!("[INFO] {} {}", prefix, s);
                $crate::Kernel::HostSpace::SlowPrint($crate::qlib::config::DebugLevel::Error, &str);
            } else {
                 let str = format!("[INFO] {} {}\n", prefix, s);
                 $crate::Kernel::HostSpace::Kprint(&str);
            }
            //$crate::qlib::perf_tunning::PerfGofrom($crate::qlib::perf_tunning::PerfType::Print);
        }
    });
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => ({
        let current = $crate::print::DEBUG;
        let level = *$crate::print::DEBUG_LEVEL;
        let cmp = level >= current;

        if cmp {
            //$crate::qlib::perf_tunning::PerfGoto($crate::qlib::perf_tunning::PerfType::Print);
            let prefix = $crate::print::PrintPrefix();
            let s = &format!($($arg)*);

            if $crate::SHARESPACE.config.SlowPrint {
                let str = format!("[DEBUG] {} {}", prefix, s);
                $crate::Kernel::HostSpace::SlowPrint($crate::qlib::config::DebugLevel::Error, &str);
            } else {
                let str = format!("[DEBUG] {} {}\n", prefix, s);
                $crate::Kernel::HostSpace::Kprint(&str);
            }
            //$crate::qlib::perf_tunning::PerfGofrom($crate::qlib::perf_tunning::PerfType::Print);
        }
    });
}

