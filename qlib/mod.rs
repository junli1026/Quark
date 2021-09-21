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

extern crate rusty_asm;
extern crate alloc;
extern crate spin;

//#[macro_use]
//pub mod macros;
pub mod common;
pub mod addr;
pub mod pagetable;
pub mod range;
pub mod linux_def;
pub mod buddyallocator;
pub mod auxv;
//pub mod Process;
pub mod bytestream;
pub mod lockfreebytestream;
pub mod config;
pub mod device;
pub mod cstring;
pub mod mem;
pub mod lrc_cache;
pub mod metric;
pub mod linux;
pub mod limits;
pub mod usage;
pub mod cpuid;
pub mod eventchannel;
pub mod qmsg;
pub mod task_mgr;
pub mod loader;
pub mod platform;
pub mod path;
pub mod auth;
pub mod control_msg;
pub mod perf_tunning;
pub mod uring;
pub mod mutex;

pub mod ringbuf;
pub mod vcpu_mgr;

use core::sync::atomic::AtomicU64;
use core::sync::atomic::AtomicI32;
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering;
use spin::Mutex;

use super::asm::*;
use self::task_mgr::*;
use self::qmsg::*;
use self::ringbuf::*;
use self::config::*;
use self::linux_def::*;
use self::bytestream::*;

pub const HYPERCALL_INIT: u16 = 1;
pub const HYPERCALL_PANIC: u16 = 2;
pub const HYPERCALL_OOM: u16 = 4;
pub const HYPERCALL_MSG: u16 = 5;
pub const HYPERCALL_U64: u16 = 6;
pub const HYPERCALL_PRINT: u16 = 8;
pub const HYPERCALL_EXIT: u16 = 9;
pub const HYPERCALL_WAKEUP: u16 = 10;
pub const HYPERCALL_GETTIME: u16 = 11;
pub const HYPERCALL_HLT: u16 = 13;
pub const HYPERCALL_URING_WAKE: u16 = 14;
pub const HYPERCALL_HCALL: u16 = 15;
pub const HYPERCALL_IOWAIT: u16 = 16;
pub const HYPERCALL_WAKEUP_VCPU: u16 = 17;
pub const HYPERCALL_EXIT_VM: u16 = 18;
pub const HYPERCALL_VCPU_FREQ: u16 = 19;
pub const HYPERCALL_VCPU_YIELD: u16 = 20;

pub const DUMMY_TASKID: TaskId = TaskId::New(0xffff_ffff);

pub const MAX_VCPU_COUNT: usize = 16;

#[allow(non_camel_case_types)]
#[repr(u64)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SysCallID {
    sys_read = 0 as u64,
    sys_write,
    sys_open,
    sys_close,
    sys_stat,
    sys_fstat,
    sys_lstat,
    sys_poll,
    sys_lseek,
    sys_mmap,
    sys_mprotect,
    //10
    sys_munmap,
    sys_brk,
    sys_rt_sigaction,
    sys_rt_sigprocmask,
    sys_rt_sigreturn,
    sys_ioctl,
    sys_pread64,
    sys_pwrite64,
    sys_readv,
    sys_writev,
    //20
    sys_access,
    sys_pipe,
    sys_select,
    sys_sched_yield,
    sys_mremap,
    sys_msync,
    sys_mincore,
    sys_madvise,
    sys_shmget,
    sys_shmat,
    //30
    sys_shmctl,
    sys_dup,
    sys_dup2,
    sys_pause,
    sys_nanosleep,
    sys_getitimer,
    sys_alarm,
    sys_setitimer,
    sys_getpid,
    sys_sendfile,
    //40
    sys_socket,
    sys_connect,
    sys_accept,
    sys_sendto,
    sys_recvfrom,
    sys_sendmsg,
    sys_recvmsg,
    sys_shutdown,
    sys_bind,
    sys_listen,
    //50
    sys_getsockname,
    sys_getpeername,
    sys_socketpair,
    sys_setsockopt,
    sys_getsockopt,
    sys_clone,
    sys_fork,
    sys_vfork,
    sys_execve,
    sys_exit,
    //60
    sys_wait4,
    sys_kill,
    sys_uname,
    sys_semget,
    sys_semop,
    sys_semctl,
    sys_shmdt,
    sys_msgget,
    sys_msgsnd,
    sys_msgrcv,
    //70
    sys_msgctl,
    sys_fcntl,
    sys_flock,
    sys_fsync,
    sys_fdatasync,
    sys_truncate,
    sys_ftruncate,
    sys_getdents,
    sys_getcwd,
    sys_chdir,
    //80
    sys_fchdir,
    sys_rename,
    sys_mkdir,
    sys_rmdir,
    sys_creat,
    sys_link,
    sys_unlink,
    sys_symlink,
    sys_readlink,
    sys_chmod,
    //90
    sys_fchmod,
    sys_chown,
    sys_fchown,
    sys_lchown,
    sys_umask,
    sys_gettimeofday,
    sys_getrlimit,
    sys_getrusage,
    sys_sysinfo,
    sys_times,
    //100
    sys_ptrace,
    sys_getuid,
    sys_syslog,
    sys_getgid,
    sys_setuid,
    sys_setgid,
    sys_geteuid,
    sys_getegid,
    sys_setpgid,
    sys_getppid,
    //110
    sys_getpgrp,
    sys_setsid,
    sys_setreuid,
    sys_setregid,
    sys_getgroups,
    sys_setgroups,
    sys_setresuid,
    sys_getresuid,
    sys_setresgid,
    sys_getresgid,
    //120
    sys_getpgid,
    sys_setfsuid,
    sys_setfsgid,
    sys_getsid,
    sys_capget,
    sys_capset,
    sys_rt_sigpending,
    sys_rt_sigtimedwait,
    sys_rt_sigqueueinfo,
    sys_rt_sigsuspend,
    //130
    sys_sigaltstack,
    sys_utime,
    sys_mknod,
    sys_uselib,
    sys_personality,
    sys_ustat,
    sys_statfs,
    sys_fstatfs,
    sys_sysfs,
    sys_getpriority,
    //140
    sys_setpriority,
    sys_sched_setparam,
    sys_sched_getparam,
    sys_sched_setscheduler,
    sys_sched_getscheduler,
    sys_sched_get_priority_max,
    sys_sched_get_priority_min,
    sys_sched_rr_get_interval,
    sys_mlock,
    sys_munlock,
    //150
    sys_mlockall,
    sys_munlockall,
    sys_vhangup,
    sys_modify_ldt,
    sys_pivot_root,
    sys__sysctl,
    sys_prctl,
    sys_arch_prctl,
    sys_adjtimex,
    sys_setrlimit,
    sys_chroot,
    sys_sync,
    sys_acct,
    sys_settimeofday,
    sys_mount,
    sys_umount2,
    sys_swapon,
    sys_swapoff,
    sys_reboot,
    sys_sethostname,
    //160
    sys_setdomainname,
    sys_iopl,
    sys_ioperm,
    sys_create_module,
    sys_init_module,
    sys_delete_module,
    sys_get_kernel_syms,
    sys_query_module,
    sys_quotactl,
    sys_nfsservctl,
    //180
    sys_getpmsg,
    sys_putpmsg,
    sys_afs_syscall,
    sys_tuxcall,
    sys_security,
    sys_gettid,
    sys_readahead,
    sys_setxattr,
    sys_lsetxattr,
    sys_fsetxattr,
    //190
    sys_getxattr,
    sys_lgetxattr,
    sys_fgetxattr,
    sys_listxattr,
    sys_llistxattr,
    sys_flistxattr,
    sys_removexattr,
    sys_lremovexattr,
    sys_fremovexattr,
    sys_tkill,
    //200
    sys_time,
    sys_futex,
    sys_sched_setaffinity,
    sys_sched_getaffinity,
    sys_set_thread_area,
    sys_io_setup,
    sys_io_destroy,
    sys_io_getevents,
    sys_io_submit,
    sys_io_cancel,
    //210
    sys_get_thread_area,
    sys_lookup_dcookie,
    sys_epoll_create,
    sys_epoll_ctl_old,
    sys_epoll_wait_old,
    sys_remap_file_pages,
    sys_getdents64,
    sys_set_tid_address,
    sys_restart_syscall,
    sys_semtimedop,
    //220
    sys_fadvise64,
    sys_timer_create,
    sys_timer_settime,
    sys_timer_gettime,
    sys_timer_getoverrun,
    sys_timer_delete,
    sys_clock_settime,
    sys_clock_gettime,
    sys_clock_getres,
    sys_clock_nanosleep,
    //230
    sys_exit_group,
    sys_epoll_wait,
    sys_epoll_ctl,
    sys_tgkill,
    sys_utimes,
    sys_vserver,
    sys_mbind,
    sys_set_mempolicy,
    sys_get_mempolicy,
    sys_mq_open,
    //240
    sys_mq_unlink,
    sys_mq_timedsend,
    sys_mq_timedreceive,
    sys_mq_notify,
    sys_mq_getsetattr,
    sys_kexec_load,
    sys_waitid,
    sys_add_key,
    sys_request_key,
    sys_keyctl,
    //250
    sys_ioprio_set,
    sys_ioprio_get,
    sys_inotify_init,
    sys_inotify_add_watch,
    sys_inotify_rm_watch,
    sys_migrate_pages,
    sys_openat,
    sys_mkdirat,
    sys_mknodat,
    sys_fchownat,
    //260
    sys_futimesat,
    sys_newfstatat,
    sys_unlinkat,

    sys_renameat,
    sys_linkat,
    sys_symlinkat,
    sys_readlinkat,
    sys_fchmodat,
    sys_faccessat,
    sys_pselect6,
    //270
    sys_ppoll,
    sys_unshare,
    sys_set_robust_list,
    sys_get_robust_list,
    sys_splice,
    sys_tee,
    sys_sync_file_range,
    sys_vmsplice,
    sys_move_pages,
    sys_utimensat,
    //280
    sys_epoll_pwait,
    sys_signalfd,
    sys_timerfd_create,
    sys_eventfd,
    sys_fallocate,
    sys_timerfd_settime,
    sys_timerfd_gettime,
    sys_accept4,
    sys_signalfd4,
    sys_eventfd2,
    //290
    sys_epoll_create1,
    sys_dup3,
    sys_pipe2,
    sys_inotify_init1,
    sys_preadv,
    sys_pwritev,
    sys_rt_tgsigqueueinfo,
    sys_perf_event_open,
    sys_recvmmsg,
    sys_fanotify_init,
    //300
    sys_fanotify_mark,
    sys_prlimit64,
    sys_name_to_handle_at,
    sys_open_by_handle_at,
    sys_clock_adjtime,
    sys_syncfs,
    sys_sendmmsg,
    sys_setns,
    sys_getcpu,
    sys_process_vm_readv,
    //310
    sys_process_vm_writev,
    sys_kcmp,
    sys_finit_module,
    sys_sched_setattr,
    sys_sched_getattr,
    sys_renameat2,
    sys_seccomp,
    sys_getrandom,
    sys_memfd_create,
    sys_kexec_file_load,
    //320
    sys_bpf,
    sys_stub_execveat,
    sys_userfaultfd,
    sys_membarrier,
    sys_mlock2,
    sys_copy_file_range,
    sys_preadv2,
    sys_pwritev2,
    sys_pkey_mprotect,
    sys_pkey_alloc,
    // 330
    sys_pkey_free,
    sys_statx,

    maxsupport,
}

#[derive(Clone, Default, Debug, Copy)]
pub struct GetTimeCall {
    pub res: i64,
    pub clockId: i32,
}

#[derive(Clone, Default, Debug, Copy)]
pub struct VcpuFeq {
    pub res: i64,
}

pub enum IOState {
    Wait,
    Processing,
}

#[derive(Clone, Default, Debug, Copy)]
pub struct LoadAddr {
    pub phyAddr: u64,
    pub virtualAddr: u64,
    pub len: u64,
}

#[derive(Clone, Default, Debug)]
pub struct Str {
    pub addr: u64,
    pub len: u32
}

#[derive(Clone, Debug, PartialEq, Copy)]
#[repr(u64)]
pub enum IOThreadState {
    //IOThread is in waiting epoll_wait state
    WAITING = 0,
    //IOThread is running
    RUNNING = 1,
}

#[repr(align(128))]
pub struct ShareSpace {
    pub QInput: QRingBuf<HostInputMsg>, //Mutex<VecDeque<HostInputMsg>>,
    pub QOutput: QRingBuf<HostOutputMsg>,  //Mutex<VecDeque<HostOutputMsg>>,

    pub hostIOThreadEventfd: i32,
    pub hostIOThreadTriggerData: u64,

    pub scheduler: task_mgr::Scheduler,
    pub ioThreadState: AtomicU64,
    pub hostMsgCount : AtomicU64,
    pub guestMsgCount: AtomicU64,

    pub kernelIOThreadWaiting: AtomicBool,
    pub config: Config,

    pub logBuf: Mutex<Option<ByteStream>>,
    pub logfd: AtomicI32,

    pub values: [[AtomicU64; 2]; 16],
}

impl ShareSpace {
    pub fn New() -> Self {
        return ShareSpace {
            QInput: QRingBuf::New(MemoryDef::MSG_QLEN), //Mutex::new(VecDeque::with_capacity(MSG_QLEN)),
            QOutput: QRingBuf::New(MemoryDef::MSG_QLEN), //Mutex::new(VecDeque::with_capacity(MSG_QLEN)),

            hostIOThreadEventfd: 0,
            hostIOThreadTriggerData: 1,

            scheduler: task_mgr::Scheduler::default(),
            ioThreadState: AtomicU64::new(IOThreadState::WAITING as u64),
            hostMsgCount: AtomicU64::new(0),
            guestMsgCount: AtomicU64::new(0),
            kernelIOThreadWaiting: AtomicBool::new(false),
            config: Config::default(),
            logBuf: Mutex::new(None),
            logfd: AtomicI32::new(-1),
            values: [
                [AtomicU64::new(0), AtomicU64::new(0)], [AtomicU64::new(0), AtomicU64::new(0)], [AtomicU64::new(0), AtomicU64::new(0)], [AtomicU64::new(0), AtomicU64::new(0)],
                [AtomicU64::new(0), AtomicU64::new(0)], [AtomicU64::new(0), AtomicU64::new(0)], [AtomicU64::new(0), AtomicU64::new(0)], [AtomicU64::new(0), AtomicU64::new(0)],
                [AtomicU64::new(0), AtomicU64::new(0)], [AtomicU64::new(0), AtomicU64::new(0)], [AtomicU64::new(0), AtomicU64::new(0)], [AtomicU64::new(0), AtomicU64::new(0)],
                [AtomicU64::new(0), AtomicU64::new(0)], [AtomicU64::new(0), AtomicU64::new(0)], [AtomicU64::new(0), AtomicU64::new(0)], [AtomicU64::new(0), AtomicU64::new(0)],
            ],
        }
    }

    pub fn SetValue(&self, cpuId: usize, idx: usize, val: u64) {
        self.values[cpuId][idx].store(val, Ordering::Relaxed);
    }

    pub fn GetValue(&self, cpuId: usize, idx: usize) -> u64 {
        return self.values[cpuId][idx].load(Ordering::Relaxed);
    }

    #[inline]
    pub fn AQHostInputPop(&self) -> Option<HostInputMsg> {
        let res = self.QInput.Pop();
        return res;
    }

    #[inline]
    pub fn AQHostInputTryPop(&self) -> Option<HostInputMsg> {
        return self.QInput.TryPop();
    }

    #[inline]
    pub fn AQHostOutputPop(&self) -> Option<HostOutputMsg> {
        let res = self.QOutput.Pop();

        if res.is_some() {
            self.hostMsgCount.fetch_sub(1, Ordering::SeqCst);
        }

        return res;
    }

    pub fn SetLogfd(&self, fd: i32) {
        self.logfd.store(fd, Ordering::SeqCst);
    }

    pub fn Logfd(&self) -> i32 {
        return self.logfd.load(Ordering::SeqCst);
    }

    pub fn Log(&self, buf: &[u8]) -> bool {
        for i in 0..3 {
            match self.logBuf.lock().as_mut().unwrap().writeFull(buf) {
                Err(_) => {
                    print!("log is full ... retry {}", i+1);
                    Self::Yield();
                }
                Ok((trigger, _)) => {
                    return trigger
                }
            }
        }

        panic!("Log is full...")
    }

    pub fn ConsumeAndGetAvailableWriteBuf(&self, cnt: usize) -> (u64, usize) {
        let mut lock = self.logBuf.lock();
        lock.as_mut().unwrap().Consume(cnt);
        let (addr, len) = lock.as_mut().unwrap().GetDataBuf();
        return (addr, len)
    }

    pub fn GetDataBuf(&self) -> (u64, usize) {
        let mut lock = self.logBuf.lock();
        let (addr, len) = lock.as_mut().unwrap().GetDataBuf();
        return (addr, len)
    }

    pub fn ReadLog(&self, buf: &mut [u8]) -> usize {
        let (_trigger, cnt) = self.logBuf.lock().as_mut().unwrap().read(buf).unwrap();
        return cnt;
    }

    #[inline]
    pub fn ReadyTaskCnt(&self, vcpuId: usize) -> u64 {
        return self.scheduler.ReadyTaskCnt(vcpuId) as u64;
    }

    #[inline]
    pub fn ReadyAsyncMsgCnt(&self) -> u64 {
        return self.QInput.Count() as u64;
    }

    #[inline]
    pub fn ReadyOutputMsgCnt(&self) -> u64 {
        //return self.QOutput.CountLockless() as u64;
        self.hostMsgCount.load(Ordering::SeqCst)
    }

    pub fn SetIOThreadState(&self, state: IOThreadState) {
        self.ioThreadState.store(state as u64, Ordering::Release);
    }

    pub fn SwapIOThreadState(&self, state: IOThreadState) -> IOThreadState {
        let old = self.ioThreadState.swap(state as u64, Ordering::SeqCst);
        return unsafe { core::mem::transmute(old) };
    }

    pub fn WakeInHost(&self) {
        self.SetIOThreadState(IOThreadState::RUNNING);
    }

    pub fn WaitInHost(&self) {
        self.SetIOThreadState(IOThreadState::WAITING);
    }

    pub fn IOThreadState(&self) -> IOThreadState {
        let state = self.ioThreadState.load(Ordering::Acquire);
        return unsafe { core::mem::transmute(state) };
    }
}
