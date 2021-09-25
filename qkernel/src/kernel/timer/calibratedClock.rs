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
//use spin::RwLock;
use lazy_static::lazy_static;
use core::ops::Deref;

use super::super::super::asm::muldiv64;
use super::super::super::qlib::linux::time::*;
use super::super::super::qlib::metric::*;
use super::super::super::qlib::common::*;
use super::super::super::qlib::mutex::*;
use super::super::super::qlib::linux_def::*;
use super::super::super::Kernel::HostSpace;
use super::super::super::asm::*;
use super::sampler::*;
use super::parameters::*;
use super::*;

lazy_static! {
    // fallbackMetric tracks failed updates. It is not sync, as it is not critical
    // that all occurrences are captured and CalibratedClock may fallback many
    // times.
    pub static ref FALLBACK_METRIC : Arc<U64Metric> = NewU64Metric("/time/fallback", false,
            "Incremented when a clock falls back to system calls due to a failed update");
}

// CalibratedClock implements a clock that tracks a reference clock.
//
// Users should call Update at regular intervals of around approxUpdateInterval
// to ensure that the clock does not drift significantly from the reference
// clock.
pub struct CalibratedClockInternal {
    // ref sample the reference clock that this clock is calibrated
    // against.
    pub sampler: Sampler,

    // ready indicates that the fields below are ready for use calculating
    // time.
    pub ready: bool,

    // params are the current timekeeping parameters.
    pub params: Parameters,

    // errorNS is the estimated clock error in nanoseconds.
    pub errorNS: ReferenceNS,
}

impl CalibratedClockInternal {
    pub fn resetLocked(&mut self, str: &str) {
        info!("{}", str);
        self.ready = false;
        self.sampler.Reset();
        FALLBACK_METRIC.Incr();
    }

    pub fn updateParams(&mut self, actual: &Parameters) {
        if !self.ready {
            // At initial calibration there is nothing to correct.
            self.params = *actual;
            self.ready = true;
            return;
        }

        let (newParams, errorNS) = match ErrorAdjust(&self.params, actual, actual.BaseCycles) {
            Ok((n, e)) => (n, e),
            Err(err) => {
                // Something is very wrong. Reset and try again from the
                // beginning.
                self.resetLocked(format!("Unable to update params: {:?}.", err).as_str());
                return;
            }
        };

        let clockId = self.sampler.clockID;
        logErrorAdjustement(clockId, errorNS, &self.params, &newParams);

        if Magnitude(errorNS) > MAX_CLOCK_ERROR {
            // We should never get such extreme error, something is very
            // wrong. Reset everything and start again.
            self.resetLocked("Extreme clock error.");
            return;
        }

        self.params = newParams;
        self.errorNS = errorNS;
    }
}

#[derive(Clone)]
pub struct CalibratedClock(Arc<QRwLock<CalibratedClockInternal>>);

impl Deref for CalibratedClock {
    type Target = Arc<QRwLock<CalibratedClockInternal>>;

    fn deref(&self) -> &Arc<QRwLock<CalibratedClockInternal>> {
        &self.0
    }
}

impl CalibratedClock {
    pub fn New(c: ClockID) -> Self {
        let internal = CalibratedClockInternal {
            sampler: Sampler::New(c),
            ready: false,
            params: Parameters::default(),
            errorNS: 0,
        };
        return Self(Arc::new(QRwLock::new(internal)))
    }

    // reset forces the clock to restart the calibration process, logging the
    // passed message.
    fn reset(&self, str: &str) {
        self.write().resetLocked(str);
    }

    // Update runs the update step of the clock, updating its synchronization with
    // the reference clock.
    //
    // Update returns timekeeping and true with the new timekeeping parameters if
    // the clock is calibrated. Update should be called regularly to prevent the
    // clock from getting significantly out of sync from the reference clock.
    //
    // The returned timekeeping parameters are invalidated on the next call to
    // Update.
    pub fn Update(&self) -> (Parameters, bool) {
        let mut c = self.write();

        let sample = c.sampler.Sample();
        match sample {
            Err(err) => {
                c.resetLocked(format!("Unable to update calibrated clock: {:?}.", err).as_str());
                return (Parameters::default(), false)
            }
            Ok(()) => (),
        }

        let (oldest, newest) = match c.sampler.Range() {
            None => return (Parameters::default(), false),
            Some((o, n)) => (o, n),
        };

        let minCount = (newest.Before - oldest.After) as u64;
        let maxCount = (newest.After - oldest.Before) as u64;
        let refInterval = (newest.Ref - oldest.Ref) as u64;

        // freq hz = count / (interval ns) * (nsPerS ns) / (1 s)
        let nsPerS = SECOND as u64;

        let (minHz, ok) = muldiv64(minCount, nsPerS, refInterval);
        if !ok {
            c.resetLocked(format!("Unable to update calibrated clock: ({} - {}) * {} / {} overflows.",
                                  newest.Before, oldest.After, nsPerS, refInterval).as_str());
            return (Parameters::default(), false)
        }

        let (maxHz, ok) = muldiv64(maxCount, nsPerS, refInterval);
        if !ok {
            c.resetLocked(format!("Unable to update calibrated clock: ({} - {}) * {} / {} overflows.",
                                  newest.After, oldest.Before, nsPerS, refInterval).as_str());
            return (Parameters::default(), false)
        }

        c.updateParams(&Parameters {
            Frequency: (minHz + maxHz) / 2,
            BaseRef: newest.Ref,
            BaseCycles: newest.After,
        });

        return (c.params, true)
    }

    // GetTime returns the current time based on the clock calibration.
    pub fn GetTime(&self) -> Result<i64> {
        let now = {
            let c = self.read();

            if !c.ready {
                let ret = c.sampler.Syscall();
                return ret;
            }

            let now = c.sampler.Cycles();
            let (v, ok) = c.params.ComputeTime(now);
            if ok {
                return Ok(v)
            }
            now
        };

        let mut c = self.write();
        // Something is seriously wrong with the clock. Try
        // again with syscalls.
        let parameters = c.params;
        c.resetLocked(format!("Time computation overflowed. params ={:?}, now = {}.",
                              &parameters, now).as_str());
        return c.sampler.Syscall();
    }
}

#[derive(Clone)]
pub struct CalibratedClocks {
    pub monotonic: CalibratedClock,
    pub realtime: CalibratedClock,
}

impl CalibratedClocks {
    pub fn New() -> Self {
        return Self {
            monotonic: CalibratedClock::New(MONOTONIC),
            realtime: CalibratedClock::New(REALTIME),
        }
    }

    pub fn Update_withSample(&mut self) -> (Parameters, bool, Parameters, bool) {
        let (monotonicParams, monotonicOk) = self.monotonic.Update();
        let (realtimeParams, realtimeOk) = self.realtime.Update();

        return (monotonicParams, monotonicOk, realtimeParams, realtimeOk)
    }

    pub fn Update(&mut self) -> (Parameters, bool, Parameters, bool) {
        let freq = HostSpace::KernelVcpuFreq() as u64;

        let tsc1 = Rdtsc();
        let monotime = HostSpace::KernelGetTime(MONOTONIC).unwrap();
        let tsc2 = Rdtsc();

        let tsc = (tsc1 + tsc2) / 2;

        let monotonicParams = Parameters {
            Frequency: freq,
            BaseRef: monotime,
            BaseCycles: tsc,
        };

        let tsc1 = Rdtsc();
        let realtime = HostSpace::KernelGetTime(REALTIME).unwrap();
        let tsc2 = Rdtsc();

        let tsc = (tsc1 + tsc2) / 2;

        let realtimeParams = Parameters {
            Frequency: freq,
            BaseRef: realtime,
            BaseCycles: tsc,
        };

        return (monotonicParams, true, realtimeParams, true)
    }

    pub fn GetTime(&self, id: ClockID) -> Result<i64> {
        match id {
            MONOTONIC => self.monotonic.GetTime(),
            REALTIME => self.realtime.GetTime(),
            _ => return Err(Error::SysError(SysErr::EINVAL))
        }
    }
}
