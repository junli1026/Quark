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


mod idt;
use lazy_static::lazy_static;
use core::fmt;

use super::qlib::addr::*;
use super::task::*;
use super::qlib::common::*;
use super::qlib::linux_def::*;
use super::threadmgr::task_sched::*;
use super::SignalDef::*;
use super::MainRun;
use super::asm::*;
use super::qlib::perf_tunning::*;
use super::SHARESPACE;

#[derive(Clone, Copy, Debug)]
pub enum ExceptionStackVec {
    DivideByZero,
    Debug,
    NMI,
    Breakpoint,
    Overflow,
    BoundRangeExceeded,
    InvalidOpcode,
    DeviceNotAvailable,
    DoubleFault,
    CoprocessorSegmentOverrun,
    InvalidTSS,
    SegmentNotPresent,
    StackSegmentFault,
    GeneralProtectionFault,
    PageFault,
    X87FloatingPointException,
    AlignmentCheck,
    MachineCheck,
    SIMDFloatingPointException,
    VirtualizationException,
    SecurityException,
    SyscallInt80,
    NrInterrupts,
}

pub fn test_breakpoint_exception() {
    // invoke a breakpoint exception
    x86_64::instructions::interrupts::int3();
}

extern "C" {
    pub fn div_zero_handler();
    pub fn debug_handler();
    pub fn nm_handler();
    pub fn breakpoint_handler();
    pub fn overflow_handler();
    pub fn bound_range_handler();
    pub fn invalid_op_handler();
    pub fn device_not_available_handler();
    pub fn double_fault_handler();
    pub fn invalid_tss_handler();
    pub fn segment_not_present_handler();
    pub fn stack_segment_handler();
    pub fn gp_handler();
    pub fn page_fault_handler();
    pub fn x87_fp_handler();
    pub fn alignment_check_handler();
    pub fn machine_check_handler();
    pub fn simd_fp_handler();
    pub fn virtualization_handler();
    pub fn security_handler();
}

lazy_static! {
    static ref IDT: idt::Idt = {
        let mut idt = idt::Idt::new();

        idt.set_handler(0, div_zero_handler).set_stack_index(0);

        idt.set_handler(1, debug_handler).set_stack_index(0);
        idt.set_handler(2, nm_handler).set_stack_index(0);
        idt.set_handler(3, breakpoint_handler).set_stack_index(0).set_privilege_level(3);
        idt.set_handler(4, overflow_handler).set_stack_index(0);
        idt.set_handler(5, bound_range_handler).set_stack_index(0);
        idt.set_handler(6, invalid_op_handler).set_stack_index(0).set_privilege_level(3);
        idt.set_handler(7, device_not_available_handler).set_stack_index(0);
        idt.set_handler(8, double_fault_handler).set_stack_index(0);

        idt.set_handler(10, invalid_tss_handler).set_stack_index(0);
        idt.set_handler(11, segment_not_present_handler).set_stack_index(0);
        idt.set_handler(12, stack_segment_handler).set_stack_index(0);
        idt.set_handler(13, gp_handler).set_stack_index(0).set_privilege_level(3);

        idt.set_handler(14, page_fault_handler).set_stack_index(0).set_privilege_level(3);

        idt.set_handler(16, x87_fp_handler).set_stack_index(0);
        idt.set_handler(17, alignment_check_handler).set_stack_index(0);
        idt.set_handler(18, machine_check_handler).set_stack_index(0);
        idt.set_handler(19, simd_fp_handler).set_stack_index(0);
        idt.set_handler(20, virtualization_handler).set_stack_index(0);

        idt.set_handler(30, security_handler).set_stack_index(0);

        idt
    };
}

pub fn init() {
    IDT.load();
}

#[repr(C)]
pub struct ExceptionStackFrame {
    ip: u64,
    cs: u64,
    eflags: u64,
    sp: u64,
    ss: u64,
}

impl fmt::Debug for ExceptionStackFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExceptionStackFrame {{ ip: {:x}, cs: {:x}, eflags: {:x}, sp: {:x}, ss: {:x},}}",
               self.ip, self.cs, self.eflags, self.sp, self.ss)
    }
}

pub fn ExceptionHandler(ev: ExceptionStackVec, sf: &ExceptionStackFrame, errorCode: u64) {
    let PRINT_EXECPTION : bool = SHARESPACE.config.PrintException;

    let currTask = Task::Current();

    // is this call from user
    if sf.ss & 0x3 != 0 {
        SwapGs();
        //PerfGofrom(PerfType::User);
        currTask.AccountTaskLeave(SchedState::RunningApp);
    } else {
        let pt = currTask.GetPtRegs();
        print!("get non page fault exception from kernel ... {:#x?}/registers is {:#x?}", sf, pt);

        for i in 0..super::CPU_LOCAL.len() {
            print!("CPU#{} is {:#x?}", i, super::CPU_LOCAL[i]);
        }

        /*backtracer::trace1(sf.ip, sf.sp, pt.rbp, &mut |frame| {
            print!("pagefault frame is {:#x?}", frame);
            true
        });*/
        panic!("Get on page fault exception from kernel");
    };

    if PRINT_EXECPTION {
        let map =  currTask.mm.GetSnapshotLocked(currTask, false);
        error!("ExceptionHandler  .... ev is {:?}, sf is {:x?} errorcode is {:x}, map is {}",
            ev, sf, errorCode, &map);
    }

    let mut rflags = GetRflags();
    rflags &= !KERNEL_FLAGS_CLEAR;
    rflags |= KERNEL_FLAGS_SET;
    SetRflags(rflags);
    //currTask.SaveFp();

    let regs = currTask.GetPtRegs();

    defer!({
        let mut rflags = regs.eflags;
        rflags &= !USER_FLAGS_CLEAR;
        rflags |= USER_FLAGS_SET;
        SetRflags(rflags);
        Task::Current().RestoreFp();
    });

    match ev {
        ExceptionStackVec::DivideByZero => {
            let info = SignalInfo {
                Signo: Signal::SIGFPE,
                Code: 1,  // FPE_INTDIV (divide by zero).
                ..Default::default()
            };

            let sigfault = info.SigFault();
            sigfault.addr = sf.ip;

            let thread = currTask.Thread();
            // Synchronous signal. Send it to ourselves. Assume the signal is
            // legitimate and force it (work around the signal being ignored or
            // blocked) like Linux does. Conveniently, this is even the correct
            // behavior for SIGTRAP from single-stepping.
            thread.forceSignal(Signal(info.Signo), false);
            thread.SendSignal(&info).expect("DivByZeroHandler send signal fail");
        }
        ExceptionStackVec::Overflow => {
            let info = SignalInfo {
                Signo: Signal::SIGFPE,
                Code: 2,  // FPE_INTOVF (integer overflow).
                ..Default::default()
            };

            let sigfault = info.SigFault();
            sigfault.addr = sf.ip;

            let thread = currTask.Thread();
            // Synchronous signal. Send it to ourselves. Assume the signal is
            // legitimate and force it (work around the signal being ignored or
            // blocked) like Linux does. Conveniently, this is even the correct
            // behavior for SIGTRAP from single-stepping.
            thread.forceSignal(Signal(info.Signo), false);
            thread.SendSignal(&info).expect("DivByZeroHandler send signal fail");
        }
        ExceptionStackVec::X87FloatingPointException |
        ExceptionStackVec::SIMDFloatingPointException => {
            let info = SignalInfo {
                Signo: Signal::SIGFPE,
                Code: 7,  // FPE_FLTINV (invalid operation).
                ..Default::default()
            };

            let sigfault = info.SigFault();
            sigfault.addr = sf.ip;

            let thread = currTask.Thread();
            // Synchronous signal. Send it to ourselves. Assume the signal is
            // legitimate and force it (work around the signal being ignored or
            // blocked) like Linux does. Conveniently, this is even the correct
            // behavior for SIGTRAP from single-stepping.
            thread.forceSignal(Signal(info.Signo), false);
            thread.SendSignal(&info).expect("DivByZeroHandler send signal fail");
        }
        ExceptionStackVec::Debug |
        ExceptionStackVec::Breakpoint => {
            let info = SignalInfo {
                Signo: Signal::SIGTRAP,
                Code: 1,
                ..Default::default()
            };

            let sigfault = info.SigFault();
            sigfault.addr = sf.ip;
            let thread = currTask.Thread();
            thread.forceSignal(Signal(info.Signo), false);
            thread.SendSignal(&info).expect("DivByZeroHandler send signal fail");
        }
        ExceptionStackVec::GeneralProtectionFault |
        ExceptionStackVec::SegmentNotPresent |
        ExceptionStackVec::BoundRangeExceeded |
        ExceptionStackVec::InvalidTSS |
        ExceptionStackVec::StackSegmentFault
        => {
            let info = SignalInfo {
                Signo: Signal::SIGSEGV,
                Code: SignalInfo::SIGNAL_INFO_KERNEL,
                ..Default::default()
            };

            let sigfault = info.SigFault();
            sigfault.addr = sf.ip;
            let thread = currTask.Thread();
            thread.forceSignal(Signal(info.Signo), false);
            thread.SendSignal(&info).expect("DivByZeroHandler send signal fail");
        }
        ExceptionStackVec::InvalidOpcode => {
            let _ml = currTask.mm.MappingWriteLock();
            let map =  currTask.mm.GetSnapshotLocked(currTask, false);
            let data = unsafe {
                *(sf.ip as * const u64)
            };

            print!("InvalidOpcode: data is {:x}, phyAddr is {:x?}, the map is {}",
                   data, currTask.mm.VirtualToPhyLocked(sf.ip), &map);

            let info = SignalInfo {
                Signo: Signal::SIGILL,
                Code: 1,
                ..Default::default()
            };

            let sigfault = info.SigFault(); // ILL_ILLOPC (illegal opcode).
            sigfault.addr = sf.ip;
            let thread = currTask.Thread();
            thread.forceSignal(Signal(info.Signo), false);
            thread.SendSignal(&info).expect("InvalidOpcode send signal fail");
        }
        ExceptionStackVec::AlignmentCheck => {
            let info = SignalInfo {
                Signo: Signal::SIGBUS,
                Code: 2,  // BUS_ADRERR (physical address does not exist).
                ..Default::default()
            };

            let sigfault = info.SigFault(); // ILL_ILLOPC (illegal opcode).
            sigfault.addr = sf.ip;
            let thread = currTask.Thread();
            thread.forceSignal(Signal(info.Signo), false);
            thread.SendSignal(&info).expect("DivByZeroHandler send signal fail");
        }
        _ => {
            panic!("ExceptionHandler: get unhanded exception {:?}", ev)
        }
    }

    MainRun(currTask, TaskRunState::RunApp);

    ReturnToApp(currTask);
}

pub fn ReturnToApp(task: &Task) -> ! {
    let pt = task.GetPtRegs();
    let kernalRsp = pt as *const _ as u64;
    let mut rflags = pt.eflags;
    rflags &= !USER_FLAGS_CLEAR;
    rflags |= USER_FLAGS_SET;
    SetRflags(rflags);

    SyscallRet(kernalRsp);
}

#[no_mangle]
pub extern fn DivByZeroHandler(sf: &ExceptionStackFrame) {
    ExceptionHandler(ExceptionStackVec::DivideByZero, sf, 0);
}

#[no_mangle]
pub extern fn DebugHandler(sf: &ExceptionStackFrame) {
    ExceptionHandler(ExceptionStackVec::Debug, sf, 0);
}

#[no_mangle]
pub extern fn NonmaskableInterrupt(sf: &ExceptionStackFrame) {
    ExceptionHandler(ExceptionStackVec::NMI, sf, 0);
}

#[no_mangle]
pub extern fn BreakpointHandler(sf: &ExceptionStackFrame) {
    // breakpoint can only call from user;
    ExceptionHandler(ExceptionStackVec::Breakpoint, sf, 0);
}

#[no_mangle]
pub extern fn OverflowHandler(sf: &ExceptionStackFrame) {
    ExceptionHandler(ExceptionStackVec::Overflow, sf, 0);
}

#[no_mangle]
pub extern fn BoundRangeHandler(sf: &ExceptionStackFrame) {
    ExceptionHandler(ExceptionStackVec::BoundRangeExceeded, sf, 0);
}

#[no_mangle]
pub extern fn InvalidOpcodeHandler(sf: &ExceptionStackFrame) {
    ExceptionHandler(ExceptionStackVec::InvalidOpcode, sf, 0);
}

#[no_mangle]
pub extern fn DeviceNotAvailableHandler(sf: &ExceptionStackFrame) {
    ExceptionHandler(ExceptionStackVec::DeviceNotAvailable, sf, 0);
}

#[no_mangle]
pub extern fn DoubleFaultHandler(sf: &mut ExceptionStackFrame, errorCode: u64) {
    ExceptionHandler(ExceptionStackVec::DoubleFault, sf, errorCode);
}

//Coprocessor Segment Overrun? skip?

#[no_mangle]
pub extern fn InvalidTSSHandler(sf: &mut ExceptionStackFrame, errorCode: u64) {
    ExceptionHandler(ExceptionStackVec::InvalidTSS, sf, errorCode);
}

#[no_mangle]
pub extern fn SegmentNotPresentHandler(sf: &mut ExceptionStackFrame, errorCode: u64) {
    ExceptionHandler(ExceptionStackVec::SegmentNotPresent, sf, errorCode);
}

#[no_mangle]
pub extern fn StackSegmentHandler(sf: &mut ExceptionStackFrame, errorCode: u64) {
    ExceptionHandler(ExceptionStackVec::StackSegmentFault, sf, errorCode);
}

// General Protection Fault
#[no_mangle]
pub extern fn GPHandler(sf: &mut ExceptionStackFrame, errorCode: u64) {
    ExceptionHandler(ExceptionStackVec::GeneralProtectionFault, sf, errorCode);
}

bitflags! {
    #[repr(transparent)]
    struct PageFaultErrorCode: u64 {
        const PROTECTION_VIOLATION = 1 << 0;
        const CAUSED_BY_WRITE = 1 << 1;
        const USER_MODE = 1 << 2;
        const MALFORMED_TABLE = 1 << 3;
        const INSTRUCTION_FETCH = 1 << 4;
    }
}

#[no_mangle]
pub extern fn PageFaultHandler(sf: &mut ExceptionStackFrame, errorCode: u64) {
    let cr2: u64;
    unsafe { llvm_asm!("mov %cr2, $0" : "=r" (cr2) ) };
    let cr3: u64;
    unsafe { llvm_asm!("mov %cr3, $0" : "=r" (cr3) ) };

    let ss: u16 = 16;
    unsafe{ llvm_asm!("movw $0, %ss" :: "r" (ss) : "memory");}

    let currTask = Task::Current();
    currTask.Check();

    // is this call from user
    let fromUser = if sf.ss & 0x3 != 0 {
        SwapGs();
        //PerfGofrom(PerfType::User);
        currTask.AccountTaskLeave(SchedState::RunningApp);
        if SHARESPACE.config.KernelPagetable {
            Task::SetKernelPageTable();
        }
        true
    } else {
        false
    };

    if !fromUser {
        let mut str = format!("Get pagefault from kernel ...cr2 {:x}, cr3 {:x} sf {:#x?} \n registers is {:#x?} \n", cr2, cr3, sf, currTask.GetPtRegs());
        for i in 0..super::CPU_LOCAL.len() {
            str += &format!("CPU#{} is {:#x?}\n", i, super::CPU_LOCAL[i]);
        }

        print!("{}", str);
        /*backtracer::trace1(sf.ip, sf.sp, pt.rbp, &mut |frame| {
            print!("pagefault frame is {:#x?}", frame);
            true
        });*/
        panic!("Get pagefault from kernel .");
    }

    currTask.PerfGoto(PerfType::PageFault);
    defer!(Task::Current().PerfGofrom(PerfType::PageFault));

    if fromUser {
        let mut rflags = GetRflags();
        rflags &= !KERNEL_FLAGS_CLEAR;
        rflags |= KERNEL_FLAGS_SET;
        SetRflags(rflags);
        //currTask.SaveFp();
    }

    let regs = currTask.GetPtRegs();
    defer!({
        if fromUser {
            let mut eflags = regs.eflags;
            eflags &= !USER_FLAGS_CLEAR;
            eflags |= USER_FLAGS_SET;
            SetRflags(eflags);
            //Task::Current().RestoreFp();
        }
    });

    let PRINT_EXECPTION : bool = SHARESPACE.config.PrintException;
    if PRINT_EXECPTION {
        error!("in PageFaultHandler, cr2: {:x}, cr3: {:x}, isuser = {}, error is {:b}, ss is {:x}, cs == {:x}, eflags = {:x}, new ss is {}",
            cr2,
            cr3,
            PageFaultErrorCode::from_bits(errorCode).unwrap() & PageFaultErrorCode::USER_MODE == PageFaultErrorCode::USER_MODE,
            PageFaultErrorCode::from_bits(errorCode).unwrap(),
            sf.ss,
            sf.cs,
            sf.eflags,
            ss
        );
    }

    let signal;
    // no need loop, just need to enable break
    loop {
        let _ml = currTask.mm.MappingWriteLock();

        let (vma, range) = match currTask.mm.GetVmaAndRangeLocked(cr2) {
            //vmas.lock().Get(cr2) {
            None => {
                if cr2 > 0x1000 {
                    let map =  currTask.mm.GetSnapshotLocked(currTask, false);
                    print!("the map is {}", &map);
                }

                //todo: when to send sigbus/SIGSEGV
                signal = Signal::SIGSEGV;
                break;
            }
            Some(vma) => vma.clone(),
        };

        let errbits = PageFaultErrorCode::from_bits(errorCode).unwrap();
        if vma.kernel == true {
            let map =  currTask.mm.GetSnapshotLocked(currTask, false);
            error!("the map2 is {}", &map);

            signal = Signal::SIGSEGV;
            break;
        }

        if !vma.effectivePerms.Read() { // has no read permission
            signal = Signal::SIGSEGV;
            break;
        }

        let pageAddr = Addr(cr2).RoundDown().unwrap().0;
        assert!(range.Contains(pageAddr), "PageFaultHandler vaddr is not in the Vma range");

        // triggered because pagetable not mapping
        if errbits & PageFaultErrorCode::PROTECTION_VIOLATION !=  PageFaultErrorCode::PROTECTION_VIOLATION {
            //error!("InstallPage 1, range is {:x?}, address is {:x}, vma.growsDown is {}",
            //    &range, pageAddr, vma.growsDown);
            match currTask.mm.InstallPageLocked(currTask, &vma, pageAddr, &range) {
                Err(Error::FileMapError) => {
                    signal = Signal::SIGBUS;
                    break;
                }
                Err(e) => {
                    panic!("PageFaultHandler error is {:?}", e)
                }
                _ => ()
            };

            for i in 1..8 {
                let addr = if vma.growsDown {
                    pageAddr - i * PAGE_SIZE
                } else {
                    pageAddr + i * PAGE_SIZE
                };
                if range.Contains(addr) {
                    match currTask.mm.InstallPageLocked(currTask, &vma, pageAddr, &range) {
                        Err(_) => {
                            break;
                        }
                        _ => ()
                    };
                } else {
                    break;
                }
            }

            if fromUser {
                //PerfGoto(PerfType::User);
                currTask.AccountTaskEnter(SchedState::RunningApp);
                if SHARESPACE.config.KernelPagetable {
                    currTask.SwitchPageTable();
                }
                SwapGs();
            }

            return
        }

        if vma.private == false {
            signal = Signal::SIGSEGV;
            break;
        }

        if (errbits & PageFaultErrorCode::CAUSED_BY_WRITE) == PageFaultErrorCode::CAUSED_BY_WRITE {
            if !vma.effectivePerms.Write() && fromUser {
                signal = Signal::SIGSEGV;
                break;
            }

            currTask.mm.CopyOnWriteLocked(pageAddr, &vma);
            if fromUser {
                //PerfGoto(PerfType::User);
                currTask.AccountTaskEnter(SchedState::RunningApp);

                if SHARESPACE.config.KernelPagetable {
                    currTask.SwitchPageTable();
                }
                SwapGs();
            }
        } else {
            signal = Signal::SIGSEGV;
            break;
        }

        return
    }

    HandleFault(currTask, fromUser, errorCode, cr2, sf, signal);
}

pub fn HandleFault(task: &mut Task, user: bool, errorCode: u64, cr2: u64, sf: &mut ExceptionStackFrame, signal: i32) -> ! {
    if !user {
        let map =  task.mm.GetSnapshotLocked(task, false);
        print!("unhandle EXCEPTION: page_fault FAULT\n{:#?}, error code is {:?}, cr2 is {:x}, registers is {:#x?}",
               sf, errorCode, cr2, task.GetPtRegs());
        print!("the map 3 is {}", &map);
        panic!();
    }

    let mut info = SignalInfo {
        Signo: signal, //Signal::SIGBUS,
        ..Default::default()
    };

    let sigfault = info.SigFault();
    sigfault.addr = cr2;
    //let read = errorCode & (1<<1) == 0;
    let write = errorCode & (1<<1) != 0;
    let execute = errorCode & (1<<4) != 0;

    if !write && !execute {
        info.Code = 1; // SEGV_MAPERR.
    } else {
        info.Code = 2; // SEGV_ACCERR.
    }

    let thread = task.Thread();
    // Synchronous signal. Send it to ourselves. Assume the signal is
    // legitimate and force it (work around the signal being ignored or
    // blocked) like Linux does. Conveniently, this is even the correct
    // behavior for SIGTRAP from single-stepping.
    thread.forceSignal(Signal(Signal::SIGSEGV), false);
    thread.SendSignal(&info).expect("PageFaultHandler send signal fail");
    MainRun(task, TaskRunState::RunApp);

    ReturnToApp(task);
}

// x87 Floating-Point Exception
#[no_mangle]
pub extern fn X87FPHandler(sf: &ExceptionStackFrame) {
    ExceptionHandler(ExceptionStackVec::X87FloatingPointException, sf, 0);
}

#[no_mangle]
pub extern fn AlignmentCheckHandler(sf: &mut ExceptionStackFrame, errorCode: u64) {
    ExceptionHandler(ExceptionStackVec::AlignmentCheck, sf, errorCode);
}

#[no_mangle]
pub extern fn MachineCheckHandler(sf: &ExceptionStackFrame) {
    ExceptionHandler(ExceptionStackVec::MachineCheck, sf, 0);
}

#[no_mangle]
pub extern fn SIMDFPHandler(sf: &ExceptionStackFrame) {
    ExceptionHandler(ExceptionStackVec::SIMDFloatingPointException, sf, 0);
}

#[no_mangle]
pub extern fn VirtualizationHandler(sf: &ExceptionStackFrame) {
    ExceptionHandler(ExceptionStackVec::VirtualizationException, sf, 0);
}

#[no_mangle]
pub extern fn SecurityHandler(sf: &mut ExceptionStackFrame, errorCode: u64) {
    ExceptionHandler(ExceptionStackVec::SecurityException, sf, errorCode);
}

#[no_mangle]
pub extern fn TripleFaultHandler(sf: &ExceptionStackFrame) {
    info!("\nTripleFaultHandler: at {:#x}\n{:#?}",
    sf.ip,
    sf);
    loop {}
}
