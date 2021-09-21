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

use core::sync::atomic::Ordering;

use qlib::*;
use super::qlib::common::*;
use super::qlib::qmsg;
use super::qlib::mutex::*;
use super::qlib::config::*;
use super::qlib::qmsg::*;
use super::qlib::task_mgr::*;
use super::qlib::linux_def::*;
//use super::qlib::perf_tunning::*;
use super::task::*;
use super::asm::*;
use super::IOURING;
use taskMgr;

extern "C" {
    pub fn rdtsc() -> i64;
}

pub struct HostSpace {}

impl HostSpace {
    pub fn Wakeup() {
        HyperCall64(HYPERCALL_WAKEUP, 0, 0);
    }

    pub fn WakeupVcpu(vcpuId: u64) {
        HyperCall64(HYPERCALL_WAKEUP_VCPU, vcpuId, 0);
    }

    pub fn IOWait() {
        HyperCall64(HYPERCALL_IOWAIT, 0, 0);
    }

    pub fn Hlt() {
        HyperCall64(HYPERCALL_HLT, 0, 0);
    }

    pub fn UringWake() {
        HyperCall64(HYPERCALL_URING_WAKE, 0, 0);
    }

    pub fn LoadProcessKernel(processAddr: u64, len: usize) -> i64 {
        let mut msg = Msg::LoadProcessKernel(LoadProcessKernel {
            processAddr: processAddr,
            len: len,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn CreateMemfd(len: i64) -> i64 {
        let mut msg = Msg::CreateMemfd(CreateMemfd {
            len: len,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn ControlMsgCall(addr: u64, len: usize) -> i64 {
        let mut msg = Msg::ControlMsgCall(ControlMsgCall {
            addr: addr,
            len: len,
        });

        return HostSpace::Call(&mut msg, true) as i64;
    }

    pub fn ControlMsgRet(msgId: u64, addr: u64, len: usize) -> i64 {
        let mut msg = Msg::ControlMsgRet(ControlMsgRet {
            msgId: msgId,
            addr: addr,
            len: len,
        });

        return HostSpace::Call(&mut msg, true) as i64;
    }

    pub fn LoadExecProcess(processAddr: u64, len: usize) -> i64 {
        let mut msg = Msg::LoadExecProcess(LoadExecProcess {
            processAddr: processAddr,
            len: len,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn Fallocate(fd: i32, mode: i32, offset: i64, len: i64) -> i64 {
        let mut msg = Msg::Fallocate(Fallocate {
            fd,
            mode,
            offset,
            len,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn RenameAt(olddirfd: i32, oldpath: u64, newdirfd: i32, newpath: u64) -> i64 {
        let mut msg = Msg::RenameAt(RenameAt {
            olddirfd,
            oldpath,
            newdirfd,
            newpath,
        });

        return HostSpace::HCall(&mut msg) as i64;
    }

    pub fn Ftruncate(fd: i32, len: i64) -> i64 {
        let mut msg = Msg::Ftruncate(Ftruncate {
            fd,
            len,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn IORead(fd: i32, iovs: u64, iovcnt: i32) -> i64 {
        let mut msg = Msg::IORead(IORead {
            fd,
            iovs,
            iovcnt,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn IOTTYRead(fd: i32, iovs: u64, iovcnt: i32) -> i64 {
        let mut msg = Msg::IOTTYRead(IOTTYRead {
            fd,
            iovs,
            iovcnt,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn IOWrite(fd: i32, iovs: u64, iovcnt: i32) -> i64 {
        let mut msg = Msg::IOWrite(IOWrite {
            fd,
            iovs,
            iovcnt,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn IOReadAt(fd: i32, iovs: u64, iovcnt: i32, offset: u64) -> i64 {
        let mut msg = Msg::IOReadAt(IOReadAt {
            fd,
            iovs,
            iovcnt,
            offset,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn IOWriteAt(fd: i32, iovs: u64, iovcnt: i32, offset: u64) -> i64 {
        let mut msg = Msg::IOWriteAt(IOWriteAt {
            fd,
            iovs,
            iovcnt,
            offset,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn IOAppend(fd: i32, iovs: u64, iovcnt: i32) -> (i64, i64) {
        let mut fileLen : i64 = 0;
        let mut msg = Msg::IOAppend(IOAppend {
            fd,
            iovs,
            iovcnt,
            fileLenAddr : &mut fileLen as * mut _ as u64,
        });

        let ret = HostSpace::Call(&mut msg, false) as i64;
        if ret < 0 {
            return (ret, 0)
        }

        return (ret, fileLen)
    }

    pub fn IOAccept(fd: i32, addr: u64, addrlen: u64, flags: i32, blocking: bool) -> i64 {
        let mut msg = Msg::IOAccept(IOAccept {
            fd,
            addr,
            addrlen,
            flags,
            blocking,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn IOConnect(fd: i32, addr: u64, addrlen: u32, blocking: bool) -> i64 {
        let mut msg = Msg::IOConnect(IOConnect {
            fd,
            addr,
            addrlen,
            blocking,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn IORecvMsg(fd: i32, msghdr: u64, flags: i32, blocking: bool) -> i64 {
        let mut msg = Msg::IORecvMsg(IORecvMsg {
            fd,
            msghdr,
            flags,
            blocking,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn IOSendMsg(fd: i32, msghdr: u64, flags: i32, blocking: bool) -> i64 {
        let mut msg = Msg::IOSendMsg(IOSendMsg {
            fd,
            msghdr,
            flags,
            blocking,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn GetTimeOfDay(tv: u64, tz: u64) -> i64 {
        let mut msg = Msg::GetTimeOfDay(GetTimeOfDay {
            tv,
            tz,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn ReadLinkAt(dirfd: i32, path: u64, buf: u64, bufsize: u64) -> i64 {
        let mut msg = Msg::ReadLinkAt(ReadLinkAt {
            dirfd,
            path,
            buf,
            bufsize,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn Close(fd: i32) -> i64 {
        let mut msg = Msg::Close(qcall::Close {
            fd
        });

        return HostSpace::HCall(&mut msg) as i64;

    }

    pub fn AsyncClose(fd: i32) {
        Self::AQCall(&qmsg::HostOutputMsg::Close(qmsg::output::Close {
            fd: fd,
        }));
    }

    pub fn Fcntl(fd: i32, cmd: i32, arg: u64) -> i64 {
        let mut msg = Msg::Fcntl(Fcntl {
            fd,
            cmd,
            arg,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn IoCtl(fd: i32, cmd: u64, argp: u64) -> i64 {
        let mut msg = Msg::IoCtl(IoCtl {
            fd,
            cmd,
            argp,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn Fstatfs(fd: i32, buf: u64) -> i64 {
        let mut msg = Msg::Fstatfs(Fstatfs {
            fd,
            buf,
        });

        return HostSpace::Call(&mut msg, false) as i64
    }

    pub fn FAccessAt(dirfd: i32, pathname: u64, mode: i32, flags: i32) -> i64 {
        let mut msg = Msg::FAccessAt(FAccessAt {
            dirfd,
            pathname,
            mode,
            flags
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn Fstat(fd: i32, buff: u64) -> i64 {
        let mut msg = Msg::Fstat(Fstat {
            fd,
            buff,
        });

        return Self::HCall(&mut msg) as i64;
        //return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn BatchFstatat(filetypes: &mut [FileType]) -> i64 {
        let addr = &filetypes[0] as * const _ as u64;
        let count = filetypes.len();
        let mut msg = Msg::BatchFstatat(BatchFstatat {
            addr,
            count
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn Fstatat(dirfd: i32, pathname: u64, buff: u64, flags: i32) -> i64 {
        let mut msg = Msg::Fstatat(Fstatat {
            dirfd,
            pathname,
            buff,
            flags,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn Getxattr(path: u64, name: u64, value: u64, size: u64) -> i64 {
        let mut msg = Msg::Getxattr(Getxattr {
            path,
            name,
            value,
            size,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn Lgetxattr(path: u64, name: u64, value: u64, size: u64) -> i64 {
        let mut msg = Msg::Lgetxattr(Lgetxattr {
            path,
            name,
            value,
            size,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn Fgetxattr(fd: i32, name: u64, value: u64, size: u64) -> i64 {
        let mut msg = Msg::Fgetxattr(Fgetxattr {
            fd,
            name,
            value,
            size,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn Unlinkat(dirfd: i32, pathname: u64, flags: i32) -> i64 {
        let mut msg = Msg::Unlinkat(Unlinkat {
            dirfd,
            pathname,
            flags
        });

        return HostSpace::HCall(&mut msg) as i64;
    }

    pub fn Mkdirat(dirfd: i32, pathname: u64, mode_: u32, uid: u32, gid: u32) -> i64 {
        let mut msg = Msg::Mkdirat(Mkdirat {
            dirfd,
            pathname,
            mode_,
            uid,
            gid,
        });

        return HostSpace::HCall(&mut msg) as i64;
    }

    pub fn SysSync() -> i64 {
        let mut msg = Msg::SysSync(SysSync {});

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn SyncFs(fd: i32) -> i64 {
        let mut msg = Msg::SyncFs(SyncFs {
            fd,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn SyncFileRange(fd: i32, offset: i64, nbytes: i64, flags: u32) -> i64 {
        let mut msg = Msg::SyncFileRange(SyncFileRange {
            fd,
            offset,
            nbytes,
            flags,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn FSync(fd: i32) -> i64 {
        let mut msg = Msg::FSync(FSync {
            fd,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn MSync(addr: u64, len: usize, flags: i32) -> i64 {
        let mut msg = Msg::MSync(MSync {
            addr,
            len,
            flags,
        });

        return HostSpace::HCall(&mut msg) as i64;
    }

    pub fn Madvise(addr: u64, len: usize, advise: i32) -> i64 {
        let mut msg = Msg::MAdvise(MAdvise {
            addr,
            len,
            advise,
        });

        return HostSpace::HCall(&mut msg) as i64;
    }

    pub fn FDataSync(fd: i32) -> i64 {
        let mut msg = Msg::FDataSync(FDataSync {
            fd,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn Seek(fd: i32, offset: i64, whence: i32) -> i64 {
        let mut msg = Msg::Seek(Seek {
            fd,
            offset,
            whence,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn GetDents64(fd: i32, dirp: u64, count: u32) -> i64 {
        let mut msg = Msg::GetDents64(GetDents64 {
            fd,
            dirp,
            count,
        });

        return HostSpace::HCall(&mut msg) as i64;
    }

    pub fn GetRandom(buf: u64, len: u64, flags: u32) -> i64 {
        let mut msg = Msg::GetRandom(GetRandom {
            buf,
            len,
            flags,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn Statm(statm: &mut StatmInfo) -> i64 {
        let mut msg = Msg::Statm(Statm {
            buf: statm as * const _ as u64
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn Socket(domain: i32, type_: i32, protocol: i32) -> i64 {
        let mut msg = Msg::Socket(Socket {
            domain,
            type_,
            protocol,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn SocketPair(domain: i32, type_: i32, protocol: i32, socketVect: u64) -> i64 {
        let mut msg = Msg::SocketPair(SocketPair {
            domain,
            type_,
            protocol,
            socketVect,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn GetSockName(sockfd: i32, addr: u64, addrlen: u64) -> i64 {
        let mut msg = Msg::GetSockName(GetSockName {
            sockfd,
            addr,
            addrlen,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn GetPeerName(sockfd: i32, addr: u64, addrlen: u64) -> i64 {
        let mut msg = Msg::GetPeerName(GetPeerName {
            sockfd,
            addr,
            addrlen,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn GetSockOpt(sockfd: i32, level: i32, optname: i32, optval: u64, optlen: u64) -> i64 {
        let mut msg = Msg::GetSockOpt(GetSockOpt {
            sockfd,
            level,
            optname,
            optval,
            optlen,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn SetSockOpt(sockfd: i32, level: i32, optname: i32, optval: u64, optlen: u32) -> i64 {
        let mut msg = Msg::SetSockOpt(SetSockOpt {
            sockfd,
            level,
            optname,
            optval,
            optlen,
        });

        //return Self::HCall(&mut msg) as i64;
        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn Bind(sockfd: i32, addr: u64, addrlen: u32, umask: u32) -> i64 {
        let mut msg = Msg::Bind(Bind {
            sockfd,
            addr,
            addrlen,
            umask,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn Listen(sockfd: i32, backlog: i32) -> i64 {
        let mut msg = Msg::Listen(Listen {
            sockfd,
            backlog,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn Shutdown(sockfd: i32, how: i32) -> i64 {
        let mut msg = Msg::Shutdown(Shutdown {
            sockfd,
            how,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn ExitVM(exitCode: i32) {
        HyperCall64(HYPERCALL_EXIT_VM, exitCode as u64, 0);
        //Self::AQCall(qmsg::HostOutputMsg::ExitVM(exitCode));
    }

    pub fn Panic(str: &str) {
        let msg = Print {
            level: DebugLevel::Error,
            str: str,
        };

        HyperCall64(HYPERCALL_PANIC, &msg as *const _ as u64, 0);
    }

    pub fn TryOpenAt(dirfd: i32, name: u64, addr: u64) -> i64 {
        let mut msg = Msg::TryOpenAt(TryOpenAt {
            dirfd: dirfd,
            name: name,
            addr: addr,
        });

        let ret = Self::HCall(&mut msg) as i64;
        return ret;
        //return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn CreateAt(dirfd: i32, pathName: u64, flags: i32, mode: i32, uid: u32, gid: u32, fstatAddr: u64) -> i64 {
        let mut msg = Msg::CreateAt(CreateAt {
            dirfd,
            pathName,
            flags,
            mode,
            uid,
            gid,
            fstatAddr
        });

        return HostSpace::HCall(&mut msg) as i64;
    }

    pub fn SchedGetAffinity(pid: i32, cpuSetSize: u64, mask: u64) -> i64 {
        let mut msg = Msg::SchedGetAffinity(SchedGetAffinity {
            pid,
            cpuSetSize,
            mask,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn Fchdir(fd: i32) -> i64 {
        let mut msg = Msg::Fchdir(Fchdir {
            fd,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn Fadvise(fd: i32, offset: u64, len: u64, advice: i32) -> i64 {
        let mut msg = Msg::Fadvise(Fadvise {
            fd,
            offset,
            len,
            advice,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn Mlock2(addr: u64, len: u64, flags: u32) -> i64 {
        let mut msg = Msg::Mlock2(Mlock2 {
            addr,
            len,
            flags,
        });

        return HostSpace::HCall(&mut msg) as i64;
    }

    pub fn MUnlock(addr: u64, len: u64) -> i64 {
        let mut msg = Msg::MUnlock(MUnlock {
            addr,
            len,
        });

        return HostSpace::HCall(&mut msg) as i64;
    }

    pub fn NonBlockingPoll(fd: i32, mask: EventMask) -> i64 {
        let mut msg = Msg::NonBlockingPoll(NonBlockingPoll {
            fd,
            mask,
        });

        //return HostSpace::Call(&mut msg, false) as i64;
        let ret = Self::HCall(&mut msg) as i64;
        //error!("NonBlockingPoll2 fd is {} ret is {}", fd, ret);

        return ret;
    }

    pub fn NewTmpfsFile(typ: TmpfsFileType, addr: u64) -> i64 {
        let mut msg = Msg::NewTmpfsFile(NewTmpfsFile {
            typ,
            addr,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn IoUringSetup(submission: u64, completion: u64) -> i64 {
        let mut msg = Msg::IoUringSetup(IoUringSetup {
            submission,
            completion
        });

        //return HostSpace::Call(&mut msg, false) as i64;
        return Self::HCall(&mut msg) as i64
    }

    pub fn IoUringRegister(fd: i32, Opcode: u32, arg: u64, nrArgs: u32) -> i64 {
        let mut msg = Msg::IoUringRegister(IoUringRegister {
            fd,
            Opcode,
            arg,
            nrArgs,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn IoUringEnter(fd: i32, toSubmit: u32, minComplete: u32, flags: u32) -> i64 {
        /*let msg = qmsg::HostOutputMsg::IoUringEnter(qmsg::IoUringEnter {
            fd,
            toSubmit,
            minComplete,
            flags,
        });

        HostSpace::AQCall(msg);*/

        let mut msg = Msg::IoUringEnter(IoUringEnter {
            fd,
            toSubmit,
            minComplete,
            flags,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn Chown(pathname: u64, owner: u32, group: u32) -> i64 {
        let mut msg = Msg::Chown(Chown {
            pathname,
            owner,
            group,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn FChown(fd: i32, owner: u32, group: u32) -> i64 {
        let mut msg = Msg::FChown(FChown {
            fd,
            owner,
            group,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn Chmod(pathname: u64, mode: u32) -> i64 {
        let mut msg = Msg::Chmod(Chmod {
            pathname,
            mode,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn Fchmod(fd: i32, mode: u32) -> i64 {
        let mut msg = Msg::Fchmod(Fchmod {
            fd,
            mode,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn SymLinkAt(oldpath: u64, newdirfd: i32, newpath: u64) -> i64 {
        let mut msg = Msg::SymLinkAt(SymLinkAt {
            oldpath,
            newdirfd,
            newpath
        });

        return HostSpace::HCall(&mut msg) as i64;
    }

    pub fn Futimens(fd: i32, times: u64) -> i64 {
        let mut msg = Msg::Futimens(Futimens {
            fd,
            times,
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    pub fn GetStdfds(addr: u64) -> i64 {
        let mut msg = Msg::GetStdfds(GetStdfds {
            addr
        });

        return HostSpace::Call(&mut msg, false) as i64;
    }

    //unblock wait
    pub fn Wait() {
        info!("start send qlib::Msg::Wait");
        let mut msg = Msg::Wait;
        HostSpace::Call(&mut msg, true);
    }

    pub fn PrintStr() {
        let msg = qmsg::HostOutputMsg::PrintStr(qmsg::PrintStr{});

        HostSpace::AQCall(&msg);
    }

    pub fn WakeVCPU(vcpuId: usize) {
        // quick path
        if super::SHARESPACE.IOThreadState() == IOThreadState::WAITING {
            HostSpace::WakeupVcpu(vcpuId as u64);
            return
        }

        let msg = qmsg::HostOutputMsg::WakeVCPU(qmsg::WakeVCPU {
            vcpuId,
        });

        HostSpace::AQCall(&msg);
    }

    pub fn MMapFile(len: u64, fd: i32, offset: u64, prot: i32) -> i64 {
        assert!(len % MemoryDef::PMD_SIZE == 0, "offset is {:x}, len is {:x}", offset, len);
        assert!(offset % MemoryDef::PMD_SIZE == 0, "offset is {:x}, len is {:x}", offset, len);
        let mut msg = Msg::MMapFile(MMapFile {
            len,
            fd,
            offset,
            prot,
        });

        let res = HostSpace::HCall(&mut msg) as i64;
        assert!(res as u64 % MemoryDef::PMD_SIZE == 0, "res {:x}", res);
        return res;
    }

    pub fn MUnmap(addr: u64, len: u64) {
        assert!(addr % MemoryDef::PMD_SIZE == 0, "addr is {:x}, len is {:x}", addr, len);
        assert!(len % MemoryDef::PMD_SIZE == 0, "addr is {:x}, len is {:x}", addr, len);
        let msg = qmsg::HostOutputMsg::MUnmap(qmsg::MUnmap {
            addr,
            len,
        });

        HostSpace::AQCall(&msg);
    }

    fn Call(msg: &mut Msg, mustAsync: bool) -> u64 {
        super::SHARESPACE.hostMsgCount.fetch_add(1, Ordering::SeqCst);
        if super::SHARESPACE.Notify() && !mustAsync  {
            super::SHARESPACE.hostMsgCount.fetch_sub(1, Ordering::SeqCst);
            return Self::HCall(msg) as u64
        }

        let current = Task::Current().GetTaskIdQ();

        //error!("Qcall msg is {:?}, super::SHARESPACE.hostMsgCount is {}", msg, super::SHARESPACE.hostMsgCount.load(Ordering::SeqCst));
        let mut event = Event {
            taskId: current,
            interrupted: false,
            ret: 0,
            msg: msg
        };

        //PerfGoto(PerfType::QCall);
        //error!("Qcall event is {:x?}", event);
        super::SHARESPACE.QCall(&mut event);
        //PerfGofrom(PerfType::QCall);

        taskMgr::Wait();
        return event.ret;
    }

    fn HCall(msg: &mut Msg) -> u64 {
        let current = Task::Current().GetTaskIdQ();

        let mut event = Event {
            taskId: current,
            interrupted: false,
            ret: 0,
            msg: msg
        };

        HyperCall64(HYPERCALL_HCALL, &mut event as * const _ as u64, 0);

        return event.ret;
    }

    fn AQCall(msg: &qmsg::HostOutputMsg) {
        super::SHARESPACE.AQHostOutputCall(msg);
    }

    pub fn WaitFD(fd: i32, mask: EventMask) {
        Self::AQCall(&qmsg::HostOutputMsg::WaitFD(qmsg::WaitFD {
            fd,
            mask,
        }))
    }

    pub fn SlowPrint(level: DebugLevel, str: &str) {
        let msg = Print {
            level,
            str,
        };

        HyperCall64(HYPERCALL_PRINT, &msg as *const _ as u64, 0);
    }

    pub fn Kprint(str: &str) {
        let bytes = str.as_bytes();
        let trigger = super::SHARESPACE.Log(bytes);
        let uringLog = super::SHARESPACE.config.UringLog;
        if uringLog {
            if trigger {
                super::IOURING.LogFlush();
            }
        } else {
            Self::PrintStr();
        }
    }

    pub fn KernelMsg(id: u64, val: u64) {
        HyperCall64(HYPERCALL_MSG, id, val)
    }

    pub fn KernelOOM(size: u64, alignment: u64) {
        HyperCall64(HYPERCALL_OOM, size, alignment)
    }

    pub fn KernelGetTime(clockId: i32) -> Result<i64> {
        let call = GetTimeCall {
            clockId,
            ..Default::default()
        };

        let addr = &call as *const _ as u64;
        HyperCall64(HYPERCALL_GETTIME, addr, 0);

        use self::common::*;

        if call.res < 0 {
            return Err(Error::SysError(-call.res as i32))
        }

        return Ok(call.res);
    }

    pub fn KernelVcpuFreq() -> i64 {
        let call = VcpuFeq::default();

        let addr = &call as *const _ as u64;
        HyperCall64(HYPERCALL_VCPU_FREQ, addr, 0);

        return call.res;
    }

    pub fn VcpuYield() {
        HyperCall64(HYPERCALL_VCPU_YIELD, 0, 0);
    }
}

pub fn GetSockOptI32(sockfd: i32, level: i32, optname: i32) -> Result<i32> {
    let mut val: i32 = 0;
    let len: i32 = 4;
    let res = HostSpace::GetSockOpt(sockfd,
                                    level,
                                    optname,
                                    &mut val as *mut i32 as u64,
                                    &len as *const i32 as u64) as i32;

    if res < 0 {
        return Err(Error::SysError(-res as i32));
    }

    return Ok(val);
}

impl<'a> ShareSpace {
    pub fn QCall(&self, item: &mut Event) {
        let addr = item as *const _ as u64;
        let msg = HostOutputMsg::QCall(addr);
        loop {
           match self.QOutput.TryPush(&msg) {
                Ok(()) => {
                    break;
                }
                Err(_) => (),
            };
        }
    }

    pub fn AQHostOutputCall(&self, item: &HostOutputMsg) {
        self.hostMsgCount.fetch_add(1, Ordering::SeqCst);
        self.Notify();

        let item = *item;
        loop {
            match self.QOutput.TryPush(&item) {
                Ok(()) => break,
                Err(_) => (),
            };
        }
    }

    // return whether it is sleeping
    pub fn Notify(&self) -> bool {
        let old = self.SwapIOThreadState(IOThreadState::RUNNING);
        if old == IOThreadState::WAITING {
            IOURING.EventfdWrite(self.hostIOThreadEventfd, &self.hostIOThreadTriggerData as * const _ as u64);
            //error!("ShareSpace::Notify wake up...");
            return true
        }

        return false;
    }

    pub fn Schedule(&self, taskId: u64) {
        self.scheduler.Schedule(TaskId::New(taskId));
    }
}

impl ShareSpace {
    pub fn Yield() {
        HostSpace::VcpuYield();
    }
}

impl <T: ?Sized> QMutex<T> {
    pub fn GetID() -> u64 {
        return Task::Current().taskId;
    }
}