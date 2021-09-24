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
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::Any;
use core::sync::atomic::AtomicI64;
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering;
use core::ptr;
use core::ops::Deref;
//use spin::Mutex;

use crate::socket::control::ControlMessage;

//use super::super::*;
use super::super::super::guestfdnotifier::*;
use super::super::socket::*;
use super::super::unix::transport::unix::*;
use super::super::super::fs::file::*;
use super::super::super::fs::flags::*;
use super::super::super::fs::dentry::*;
use super::super::super::fs::dirent::*;
use super::super::super::fs::attr::*;
use super::super::super::fs::host::hostinodeop::*;
use super::super::super::kernel::fd_table::*;
use super::super::super::kernel::waiter::*;
use super::super::super::kernel::time::*;
use super::super::super::qlib::common::*;
use super::super::super::task::*;
use super::super::super::qlib::mem::block::*;
use super::super::super::qlib::mutex::*;
use super::super::super::qlib::linux::netdevice::*;
use super::super::super::Kernel;
use super::super::super::IOURING;
use super::super::super::Kernel::HostSpace;
use super::super::super::qlib::linux_def::*;
use super::super::super::fd::*;
use super::super::super::tcpip::tcpip::*;
use super::super::super::SHARESPACE;
use super::socket_buf::*;
use super::super::super::qlib::linux::time::Timeval;
use super::super::control::ControlMessageTCPInq;

fn newSocketFile(task: &Task, family: i32, fd: i32, stype: i32, nonblock: bool, enableBuf: bool, addr: Option<Vec<u8>>) -> Result<File> {
    let dirent = NewSocketDirent(task, SOCKET_DEVICE.clone(), fd)?;
    let inode = dirent.Inode();
    let iops = inode.lock().InodeOp.clone();
    let hostiops = iops.as_any().downcast_ref::<HostInodeOp>().unwrap();
    let s = SocketOperations::New(family, fd, stype, hostiops.Queue(), enableBuf, addr)?;

    Ok(File::New(&dirent,
              &FileFlags { NonBlocking: nonblock, Read: true, Write: true, ..Default::default() },
              s))
}

#[derive(Default)]
pub struct SocketOperationsIntern {
    pub send: AtomicI64,
    pub recv: AtomicI64,
    pub family: i32,
    pub stype: i32,
    pub fd: i32,
    pub queue: Queue,
    pub remoteAddr: QMutex<Option<SockAddr>>,
    pub socketBuf: QMutex<Option<Arc<SocketBuff>>>,
    pub enableSocketBuf: AtomicBool,
    passInq: AtomicBool,
}

#[derive(Clone)]
pub struct SocketOperations(Arc<SocketOperationsIntern>);

pub const DEFAULT_BUF_PAGE_COUNT : u64 = 16;

impl SocketOperations {
    pub fn New(family: i32, fd: i32, stype: i32, queue: Queue, enableSocketBuf: bool, addr: Option<Vec<u8>>) -> Result<Self> {
        let addr = match addr {
            None => None,
            Some(v) => {
                if v.len() == 0 {
                    None
                } else {
                    Some(GetAddr(v[0] as i16, &v[0..v.len()]).unwrap())
                }
            }
        };

        let ret = SocketOperationsIntern {
            send: AtomicI64::new(0),
            recv: AtomicI64::new(0),
            family,
            stype,
            fd,
            queue,
            remoteAddr: QMutex::new(addr),
            socketBuf: QMutex::new(None),
            enableSocketBuf: AtomicBool::new(false),
            passInq: AtomicBool::new(false)
        };

        let ret = Self(Arc::new(ret));

        if enableSocketBuf {
            ret.EnableSocketBuf();
        }

       //AddFD(fd, &ret.queue);
        return Ok(ret)
    }

    fn prepareControlMessage(&self, controlDataLen: usize) -> (i32, Vec<u8>) {
        // shortcut for no controldata wanted
        if controlDataLen == 0 {
            return (0, Vec::new())
        }


        let mut controlData: Vec<u8> = vec![0; controlDataLen];
        if self.passInq.load(Ordering::Relaxed) {
            let inqMessage = ControlMessageTCPInq {
                Size: self.SocketBuf().readBuf.lock().available as u32};
                let (remaining, updated_flags) = inqMessage.EncodeInto(&mut controlData[..], 0);
                let remainSize = remaining.len();
                controlData.resize(controlDataLen - remainSize, 0);
                return (updated_flags, controlData)
        } else {
            return (0, Vec::new())
        }
    }

    pub fn SocketBufEnabled(&self) -> bool {
        return self.enableSocketBuf.load(Ordering::Relaxed);
    }

    pub fn EnableSocketBuf(&self) {
        assert!(self.SocketBufEnabled() == false);

        let socketBuf = Arc::new(SocketBuff::Init(DEFAULT_BUF_PAGE_COUNT));
        *self.socketBuf.lock() = Some(socketBuf);
        self.enableSocketBuf.store(true, Ordering::Relaxed);
        IOURING.BufSockInit(self).unwrap();
    }

    pub fn Notify(&self, mask: EventMask) {
        self.queue.Notify(EventMaskFromLinux(mask as u32));
    }
}

impl Deref for SocketOperations {
    type Target = Arc<SocketOperationsIntern>;

    fn deref(&self) -> &Arc<SocketOperationsIntern> {
        &self.0
    }
}

impl SocketOperations {
    pub fn SocketBuf(&self) -> Arc<SocketBuff> {
        return self.socketBuf.lock().as_ref().unwrap().clone();
    }

    pub fn SetRemoteAddr(&self, addr: Vec<u8>) {
        let addr = GetAddr(addr[0] as i16, &addr[0..addr.len()]).unwrap();

        *self.remoteAddr.lock() = Some(addr);
    }

    pub fn GetRemoteAddr(&self) -> Option<Vec<u8>> {
        return match *self.remoteAddr.lock() {
            None => None,
            Some(ref v) => Some(v.ToVec().unwrap()),
        }
    }
}

pub const SIZEOF_SOCKADDR: usize = SocketSize::SIZEOF_SOCKADDR_INET6;

impl Waitable for SocketOperations {
    fn Readiness(&self, _task: &Task, mask: EventMask) -> EventMask {
        if self.SocketBufEnabled() {
            return self.SocketBuf().Events() & mask
        };

        let fd = self.fd;
        return NonBlockingPoll(fd, mask);
    }

    fn EventRegister(&self, task: &Task, e: &WaitEntry, mask: EventMask) {
        let queue = self.queue.clone();
        queue.EventRegister(task, e, mask);
        let fd = self.fd;
        if !self.SocketBufEnabled() {
            UpdateFD(fd).unwrap();
        };
    }

    fn EventUnregister(&self, task: &Task, e: &WaitEntry) {
        let queue = self.queue.clone();
        queue.EventUnregister(task, e);
        let fd = self.fd;
        if !self.SocketBufEnabled() {
            UpdateFD(fd).unwrap();
        };
    }
}

// pass the ioctl to the shadow hostfd
pub fn HostIoctlIFReq(task: &Task, hostfd: i32, request: u64, addr: u64) -> Result<()> {
    let mut ifr : IFReq = task.CopyInObj(addr)?;
    let res = HostSpace::IoCtl(hostfd, request, &mut ifr as *const _ as u64);
    if res < 0 {
        return Err(Error::SysError(-res as i32))
    }

    task.CopyOutObj(&ifr, addr)?;
    return Ok(())
}

pub fn HostIoctlIFConf(task: &Task, hostfd: i32, request: u64, addr: u64) -> Result<()> {
    let mut ifc : IFConf = task.CopyInObj(addr)?;

    const MAX_LEN : usize = 64 * 0x1000; // 256 KB

    // todo: how to handle very large ifconf?
    let len = if MAX_LEN > ifc.Len as usize {
        ifc.Len as usize
    } else {
        MAX_LEN
    };

    let buf = DataBuff::New(len);

    let mut ifr = IFConf {
        Len: len as i32,
        ..Default::default()
    };

    if ifc.Ptr != 0 {
        ifr.Ptr = buf.Ptr();
    }

    let res = HostSpace::IoCtl(hostfd, request, &mut ifr as *const _ as u64);
    if res < 0 {
        return Err(Error::SysError(-res as i32))
    }

    task.mm.CopyDataOut(task, ifr.Ptr, ifc.Ptr, ifr.Len as usize)?;

    ifc.Len = ifr.Len;

    task.CopyOutObj(&ifc, addr)?;
    return Ok(())
}

impl SpliceOperations for SocketOperations {}

impl FileOperations for SocketOperations {
    fn as_any(&self) -> &Any {
        return self;
    }

    fn FopsType(&self) -> FileOpsType {
        return FileOpsType::SocketOperations
    }

    fn Seekable(&self) -> bool {
        return false;
    }

    fn Seek(&self, _task: &Task, _f: &File, _whence: i32, _current: i64, _offset: i64) -> Result<i64> {
        return Err(Error::SysError(SysErr::ESPIPE))
    }

    fn ReadDir(&self, _task: &Task, _f: &File, _offset: i64, _serializer: &mut DentrySerializer) -> Result<i64> {
        return Err(Error::SysError(SysErr::ENOTDIR))
    }

    fn ReadAt(&self, task: &Task, _f: &File, dsts: &mut [IoVec], _offset: i64, _blocking: bool) -> Result<i64> {
        if self.SocketBufEnabled() {
            //todo: optimize to avoid extra memory copy
            let size = IoVec::NumBytes(dsts);
            let buf = DataBuff::New(size);
            let mut iovs = buf.Iovs();
            let ret = IOURING.BufSockRead(task, self, &mut iovs)?;
            if ret > 0 {
                task.CopyDataOutToIovs(&buf.buf[0..ret as usize], dsts)?;
            }
            return Ok(ret);

            //return IOURING.BufSockRead(task, self, dsts)
        }

        //defer!(task.GetMut().iovs.clear());
        //task.V2PIovs(dsts, true, &mut task.GetMut().iovs)?;

        let size = IoVec::NumBytes(dsts);
        let buf = DataBuff::New(size);
        let iovs = buf.Iovs();
        let ret = IORead(self.fd, &iovs)?;
        task.CopyDataOutToIovs(&buf.buf[0..ret as usize], dsts)?;
        return Ok(ret);
    }

    fn WriteAt(&self, task: &Task, _f: &File, srcs: &[IoVec], _offset: i64, _blocking: bool) -> Result<i64> {
        if self.SocketBufEnabled() {
            //todo: optimize to avoid extra memory copy
            let size = IoVec::NumBytes(srcs);
            let mut buf = DataBuff::New(size);
            task.CopyDataInFromIovs(&mut buf.buf, &srcs)?;
            let iovs = buf.Iovs();
            return IOURING.BufSockWrite(task, self, &iovs)

            //return IOURING.BufSockWrite(task, self, srcs)
        }

        //defer!(task.GetMut().iovs.clear());
        //task.V2PIovs(srcs, false, &mut task.GetMut().iovs)?;

        let size = IoVec::NumBytes(srcs);
        let mut buf = DataBuff::New(size);
        let iovs = buf.Iovs();
        task.CopyDataInFromIovs(&mut buf.buf, srcs)?;
        return IOWrite(self.fd, &iovs);
    }

    fn Append(&self, task: &Task, f: &File, srcs: &[IoVec]) -> Result<(i64, i64)> {
        let n = self.WriteAt(task, f, srcs, 0, false)?;
        return Ok((n, 0))
    }

    fn Fsync(&self, _task: &Task, _f: &File, _start: i64, _end: i64, _syncType: SyncType) -> Result<()> {
        return Err(Error::SysError(SysErr::EINVAL))
    }

    fn Flush(&self, _task: &Task, _f: &File) -> Result<()> {
        /*error!("host::socket flush... 1");
        if self.SocketBufEnabled() {
            error!("host::socket flush... data size is {}", self.socketBuf.as_ref().unwrap().WriteBufAvailableDataSize());
        }*/
        return Ok(())
    }

    fn UnstableAttr(&self, task: &Task, f: &File) -> Result<UnstableAttr> {
        let inode = f.Dirent.Inode();
        return inode.UnstableAttr(task);

    }

    fn Ioctl(&self, task: &Task, _f: &File, _fd: i32, request: u64, val: u64) -> Result<()> {
        let flags = request as i32;

        let hostfd = self.fd;
        match flags as u64 {
            LibcConst::SIOCGIFFLAGS |
            LibcConst::SIOCGIFBRDADDR |
            LibcConst::SIOCGIFDSTADDR |
            LibcConst::SIOCGIFHWADDR |
            LibcConst::SIOCGIFINDEX |
            LibcConst::SIOCGIFMAP |
            LibcConst::SIOCGIFMETRIC |
            LibcConst::SIOCGIFMTU |
            LibcConst::SIOCGIFNAME |
            LibcConst::SIOCGIFNETMASK |
            LibcConst::SIOCGIFTXQLEN => {
                let addr = val;
                HostIoctlIFReq(task, hostfd, request, addr)?;

                return Ok(())
            }
            LibcConst::SIOCGIFCONF => {
                let addr = val;
                HostIoctlIFConf(task, hostfd, request, addr)?;

                return Ok(())
            }
            LibcConst::TIOCINQ => {
                if self.SocketBufEnabled() {
                    let v =  self.SocketBuf().readBuf.lock().available as i32;
                    task.CopyOutObj(&v, val)?;
                    return Ok(())
                } else {
                    let tmp: i32 = 0;
                    let res = Kernel::HostSpace::IoCtl(self.fd, request, &tmp as *const _ as u64);
                    if res < 0 {
                        return Err(Error::SysError(-res as i32))
                    }
                    task.CopyOutObj(&tmp, val)?;
                    return Ok(())
                }
            }
            _ => {
                let tmp: i32 = 0;
                let res = Kernel::HostSpace::IoCtl(self.fd, request, &tmp as *const _ as u64);
                if res < 0 {
                    return Err(Error::SysError(-res as i32))
                }
                task.CopyOutObj(&tmp, val)?;
                return Ok(())
            }
        }
    }

    fn IterateDir(&self, _task: &Task, _d: &Dirent, _dirCtx: &mut DirCtx, _offset: i32) -> (i32, Result<i64>) {
        return (0, Err(Error::SysError(SysErr::ENOTDIR)))
    }

    fn Mappable(&self) -> Result<HostInodeOp> {
        return Err(Error::SysError(SysErr::ENODEV))
    }
}

impl SockOperations for SocketOperations {
    fn Connect(&self, task: &Task, sockaddr: &[u8], blocking: bool) -> Result<i64> {
        let mut socketaddr = sockaddr;

        if (self.family == AFType::AF_INET || self.family == AFType::AF_INET6)
            && socketaddr.len() > SIZEOF_SOCKADDR {
            socketaddr = &socketaddr[..SIZEOF_SOCKADDR]
        }

        let res = Kernel::HostSpace::IOConnect(self.fd, &socketaddr[0] as *const _ as u64, socketaddr.len() as u32, blocking) as i32;
        if res == 0 {
            return Ok(0)
        }

        if -res != SysErr::EINPROGRESS || !blocking {
            return Err(Error::SysError(-res))
        }


        //todo: which one is more efficent?
        let general = task.blocker.generalEntry.clone();
        self.EventRegister(task, &general, EVENT_OUT);
        defer!(self.EventUnregister(task, &general));

        if self.Readiness(task, EVENT_OUT) == 0 {
            match task.blocker.BlockWithMonoTimer(true, None) {
                Err(e) => {
                    return Err(e);
                }
                _ => ()
            }
        }

        let mut val: i32 = 0;
        let len: i32 = 4;
        let res = HostSpace::GetSockOpt(self.fd, LibcConst::SOL_SOCKET as i32, LibcConst::SO_ERROR as i32, &mut val as *mut i32 as u64, &len as *const i32 as u64) as i32;

        if res < 0 {
            return Err(Error::SysError(-res))
        }

        if val != 0 {
            return Err(Error::SysError(val as i32))
        }

        self.SetRemoteAddr(socketaddr.to_vec());

        if SHARESPACE.config.TcpBuffIO {
            self.EnableSocketBuf();
        }

        return Ok(0)
    }

    fn Accept(&self, task: &Task, addr: &mut [u8], addrlen: &mut u32, flags: i32, blocking: bool) -> Result<i64> {
        //todo: use blocking parameter
        let addrLoc = if addr.len() == 0 {
            0
        } else {
            &addr[0] as *const _ as u64
        };

        info!("host socket accept #1");
        *addrlen = addr.len() as u32;
        let mut res = Kernel::HostSpace::IOAccept(self.fd, addrLoc, addrlen as *const _ as u64, flags, blocking) as i32;
        //info!("host socket accept #2 blocking = {}", blocking);
        if blocking {
            let general = task.blocker.generalEntry.clone();
            self.EventRegister(task, &general, EVENT_IN);
            while res == -SysErr::EAGAIN {
                match task.blocker.BlockWithMonoTimer(true, None) {
                    Err(e) => {
                        self.EventUnregister(task, &general);
                        return Err(e);
                    }
                    _ => ()
                }
                res = Kernel::HostSpace::IOAccept(self.fd, addrLoc, addrlen as *const _ as u64, flags, blocking) as i32;
            }
            self.EventUnregister(task, &general);
        }

        if res < 0 {
            return Err(Error::SysError(-res as i32))
        }

        let enableBuf = SHARESPACE.config.TcpBuffIO &&
            (self.family == AFType::AF_INET || self.family == AFType::AF_INET6) &&
            self.stype == SockType::SOCK_STREAM;

        let remoteAddr = &addr[0..*addrlen as usize];
        let file = newSocketFile(task,
                                 self.family,
                                 res as i32,
                                 self.stype,
                                 flags & SocketFlags::SOCK_NONBLOCK != 0,
                                 enableBuf, Some(remoteAddr.to_vec()))?;

        let fdFlags = FDFlags {
            CloseOnExec: flags & SocketFlags::SOCK_CLOEXEC != 0
        };

        let fd = task.NewFDFrom(0, &Arc::new(file), &fdFlags)?;
        return Ok(fd as i64)
    }

    fn Bind(&self, task: &Task, sockaddr: &[u8]) -> Result<i64> {
        let mut socketaddr = sockaddr;

        info!("hostinet socket bind {:?}, addr is {:?}", self.family, socketaddr);
        if (self.family == AFType::AF_INET || self.family == AFType::AF_INET6) &&
            socketaddr.len() > SIZEOF_SOCKADDR {
            socketaddr = &socketaddr[..SIZEOF_SOCKADDR]
        } /*else if self.family == AFType::AF_UNIX {
            use super::super::unix::hostsocket::*;
            let path = ExtractPath(sockaddr)?;
            info!("unix socket bind ... path is {:?}", alloc::string::String::from_utf8(path));
        }*/

        let res = Kernel::HostSpace::Bind(self.fd, &socketaddr[0] as *const _ as u64, socketaddr.len() as u32, task.Umask());
        if res < 0 {
            return Err(Error::SysError(-res as i32))
        }

        return Ok(res)
    }

    fn Listen(&self, _task: &Task, backlog: i32) -> Result<i64> {
        let res = Kernel::HostSpace::Listen(self.fd, backlog);
        if res < 0 {
            return Err(Error::SysError(-res as i32))
        }

        return Ok(res)
    }

    fn Shutdown(&self, _task: &Task, how: i32) -> Result<i64> {
        let how = how as u64;

        if how == LibcConst::SHUT_RD || how == LibcConst::SHUT_WR || how == LibcConst::SHUT_RDWR {
            let res = Kernel::HostSpace::Shutdown(self.fd, how as i32);
            if res < 0 {
                return Err(Error::SysError(-res as i32))
            }

            return Ok(res)
        }

        return Err(Error::SysError(SysErr::EINVAL))
    }

    fn GetSockOpt(&self, _task: &Task, level: i32, name: i32, opt: &mut [u8]) -> Result<i64> {
        /*
        let optlen = match level as u64 {
            LibcConst::SOL_IPV6 => {
                match name as u64 {
                    LibcConst::IPV6_V6ONLY => SocketSize::SIZEOF_INT32,
                    LibcConst::IPV6_TCLASS => SocketSize::SIZEOF_INT32,
                    _ => 0,
                }
            }
            LibcConst::SOL_SOCKET => {
                match name as u64 {
                    LibcConst::SO_ERROR
                    | LibcConst::SO_KEEPALIVE
                    | LibcConst::SO_SNDBUF
                    | LibcConst::SO_RCVBUF
                    | LibcConst::SO_REUSEADDR
                    | LibcConst::SO_TYPE => SocketSize::SIZEOF_INT32,
                    LibcConst::SO_LINGER => SocketSize::SIZEOF_LINGER,
                    _ => 0,
                }
            }
            LibcConst::SOL_TCP => {
                match name as u64 {
                    LibcConst::TCP_NODELAY => SocketSize::SIZEOF_INT32,
                    LibcConst::TCP_INFO => SocketSize::SIZEOF_TCPINFO,
                    _ => 0,
                }
            }
            LibcConst::SOL_IP => {
                match name as u64 {
                    LibcConst::IP_TTL => SocketSize::SIZEOF_INT32,
                    LibcConst::IP_TOS => SocketSize::SIZEOF_INT32,
                    _ => 0,
                }
            }
            _ => 0,
        };

        if optlen == 0 {
            return Err(Error::SysError(SysErr::ENOPROTOOPT))
        }

        let bufferSize = opt.len();

        if bufferSize < optlen {
            // provide special handling for options like IP_TOS, which allow inadequate buffer for optval
            match name as u64 {
                LibcConst::IP_TOS => {
                    let res = if bufferSize == 0 {
                        // dirty, any better way?
                        Kernel::HostSpace::GetSockOpt(self.fd, level, name, &bufferSize as *const _ as u64, &bufferSize as *const _ as u64)
                    } else {
                        Kernel::HostSpace::GetSockOpt(self.fd, level, name, &opt[0] as *const _ as u64, &bufferSize as *const _ as u64)
                    };
                    if res < 0 {
                        return Err(Error::SysError(-res as i32))
                    }
                    // if optlen < sizeof(i32), the return of getsockopt will be of sizeof(i8)
                    return Ok(bufferSize as i64)
                },
                _ => return Err(Error::SysError(SysErr::EINVAL))
            };
        };

        let opt = &opt[..optlen];
        let res = Kernel::HostSpace::GetSockOpt(self.fd, level, name, &opt[0] as *const _ as u64, &optlen as *const _ as u64);
        if res < 0 {
            return Err(Error::SysError(-res as i32))
        }

        return Ok(optlen as i64)
        */

        let mut optLen = opt.len();
        let res = if optLen == 0 {
            Kernel::HostSpace::GetSockOpt(self.fd, level, name, ptr::null::<u8>() as u64, &mut optLen as *mut _ as u64)
        } else {
            Kernel::HostSpace::GetSockOpt(self.fd, level, name, &mut opt[0] as *mut _ as u64, &mut optLen as *mut _ as u64)
        };

        if res < 0 {
            return Err(Error::SysError(-res as i32))
        }

        return Ok(optLen as i64)
    }

    fn SetSockOpt(&self, task: &Task, level: i32, name: i32, opt: &[u8]) -> Result<i64> {
        
        /*let optlen = match level as u64 {
            LibcConst::SOL_IPV6 => {
                match name as u64 {
                    LibcConst::IPV6_V6ONLY => SocketSize::SIZEOF_INT32,
                    _ => 0,
                }
            }
            LibcConst::SOL_SOCKET => {
                match name as u64 {
                    LibcConst::SO_SNDBUF
                    | LibcConst::SO_RCVBUF
                    | LibcConst::SO_REUSEADDR => {
                        SocketSize::SIZEOF_INT32
                    }
                    _ => 0,
                }
            }
            LibcConst::SOL_TCP => {
                match name as u64 {
                    LibcConst::TCP_NODELAY => SocketSize::SIZEOF_INT32,
                    _ => 0,
                }
            }
            _ => 0,
        };

        if optlen == 0 {
            return Err(Error::SysError(SysErr::ENOPROTOOPT))
        }

        if opt.len() < optlen {
            return Err(Error::SysError(SysErr::EINVAL))
        }

        let opt = &opt[..optlen];*/

        if (level as u64) == LibcConst::SOL_SOCKET &&
            (name as u64) == LibcConst::SO_RCVTIMEO {
                if opt.len() >= SocketSize::SIZEOF_TIMEVAL {
                    let timeVal = task.CopyInObj::<Timeval>(&opt[0] as *const _ as u64)?;
                    self.SetRecvTimeout(timeVal.ToDuration() as i64);
                } else {
                    //TODO: to be aligned with Linux, Linux allows shorter length for this flag.
                    return Err(Error::SysError(SysErr::EINVAL));
                }
            }

        // TCP_INQ is bound to buffer implementation
        if (level as u64) == LibcConst::SOL_TCP &&
            (name as u64) == LibcConst::TCP_INQ {
                let val = unsafe {
                    *(&opt[0] as * const _ as u64 as * const i32)
                };
                if val == 1 {
                    self.passInq.store(true, Ordering::Relaxed);
                } else {
                    self.passInq.store(false, Ordering::Relaxed);
                }
        }

        let optLen = opt.len();
        let res = if optLen == 0 {
            Kernel::HostSpace::SetSockOpt(self.fd, level, name, ptr::null::<u8>() as u64, optLen as u32)
        } else {
            Kernel::HostSpace::SetSockOpt(self.fd, level, name, &opt[0] as *const _ as u64, optLen as u32)
        };

        if res < 0 {
            return Err(Error::SysError(-res as i32))
        }

        return Ok(res)
    }

    fn GetSockName(&self, _task: &Task, socketaddr: &mut [u8]) -> Result<i64> {
        let len = socketaddr.len() as i32;

        let res = Kernel::HostSpace::GetSockName(self.fd, &socketaddr[0] as *const _ as u64, &len as *const _ as u64);
        if res < 0 {
            return Err(Error::SysError(-res as i32))
        }

        return Ok(len as i64)
    }

    fn GetPeerName(&self, _task: &Task, socketaddr: &mut [u8]) -> Result<i64> {
        let len = socketaddr.len() as i32;
        let res = Kernel::HostSpace::GetPeerName(self.fd, &socketaddr[0] as *const _ as u64, &len as *const _ as u64);
        if res < 0 {
            return Err(Error::SysError(-res as i32))
        }

        return Ok(len as i64)
    }

    fn RecvMsg(&self, task: &Task, dsts: &mut [IoVec], flags: i32, deadline: Option<Time>, senderRequested: bool, controlDataLen: usize)
        -> Result<(i64, i32, Option<(SockAddr, usize)>, Vec<u8>)>  {

        if self.SocketBufEnabled() {
            /*
            if controlDataLen != 0 {
                panic!("Hostnet RecvMsg Socketbuf doesn't support control data");
            }
            */
            let len = Iovs(dsts).Count();
            let mut count = 0;
            let mut dsts = dsts;
            let mut tmp;
            let mut res = 0;
            loop {
                match IOURING.BufSockRead(task, self, dsts) {
                    Err(Error::SysError(SysErr::EWOULDBLOCK)) => {
                        if flags & MsgType::MSG_DONTWAIT != 0 {
                            if count > 0 {
                                let (retFlags, controlData) = self.prepareControlMessage(controlDataLen); 
                                return Ok((count as i64, retFlags, None, controlData))
                            }
                            return Err(Error::SysError(SysErr::EWOULDBLOCK))
                        }

                        break;
                    }
                    Err(e) => {
                        if count > 0 {
                            let (retFlags, controlData) = self.prepareControlMessage(controlDataLen); 
                            return Ok((count as i64, retFlags, None, controlData))
                        }
                        return Err(e)
                    },
                    Ok(n) => {
                        if n == 0 {
                            let (retFlags, controlData) = self.prepareControlMessage(controlDataLen); 
                            return Ok((count, retFlags, None, controlData))
                        }

                        count += n;
                        if count == len as i64 {
                            let (retFlags, controlData) = self.prepareControlMessage(controlDataLen); 
                            return Ok((count as i64, retFlags, None, controlData))
                        }

                        tmp = Iovs(dsts).DropFirst(n as usize);
                        dsts = &mut tmp;
                    }
                }
            }

            let general = task.blocker.generalEntry.clone();
            self.EventRegister(task, &general, EVENT_READ);
            defer!(self.EventUnregister(task, &general));

            'main: loop {
                loop {
                    match IOURING.BufSockRead(task, self, dsts) {
                        Err(Error::SysError(SysErr::EWOULDBLOCK)) => {
                            if count > 0 {
                                res = count;
                                break 'main;
                            }
                            break;
                        },
                        Err(e) => {
                            if count > 0 {
                                res = count;
                                break 'main;
                            }
                            return Err(e);
                        }
                        Ok(n) => {
                            if n == 0 {
                                break 'main;
                            }

                            count += n;
                            if count == len as i64 {
                                res = count;
                                break 'main;
                            }

                            tmp = Iovs(dsts).DropFirst(n as usize);
                            dsts = &mut tmp;
                        }
                    };
                }


                match task.blocker.BlockWithMonoTimer(true, deadline) {
                    Err(e) => {
                        if count > 0 {
                            res = count;
                            break 'main;
                        }
                        match e {
                            Error::SysError(SysErr::ETIMEDOUT) => {
                                return Err(Error::SysError(SysErr::EWOULDBLOCK));
                            }
                            Error::ErrInterrupted => {
                                return Err(Error::SysError(SysErr::ERESTARTSYS));
                            }
                            _ => {
                                return Err(e);
                            }
                        }
                    }
                    _ => ()
                }
            }

            let senderAddr = if senderRequested {
                let addr = self.remoteAddr.lock().as_ref().unwrap().clone();
                let l = addr.Len();
                Some((addr, l))
            } else {
                None
            };

            let (retFlags, controlData) = self.prepareControlMessage(controlDataLen);
            return Ok((res as i64, retFlags, senderAddr, controlData))
        }

        //todo: we don't support MSG_ERRQUEUE
        if flags & !(MsgType::MSG_DONTWAIT | MsgType::MSG_PEEK | MsgType::MSG_TRUNC | MsgType::MSG_CTRUNC | MsgType::MSG_WAITALL) != 0 {
            return Err(Error::SysError(SysErr::EINVAL))
        }

        /* 
        if IoVec::NumBytes(dsts) == 0 {
            return Ok((0, 0, None, SCMControlMessages::default()))
        }
        */

        /*defer!(task.GetMut().iovs.clear());
        task.V2PIovs(dsts, true, &mut task.GetMut().iovs)?;
        let iovs = &mut task.GetMut().iovs;*/

        let size = IoVec::NumBytes(dsts);
        let buf = DataBuff::New(size);
        let iovs = buf.Iovs();

        let mut msgHdr = MsgHdr::default();
        if IoVec::NumBytes(dsts) != 0 {
            msgHdr.iov = &iovs[0] as *const _ as u64;
        } else {
            msgHdr.iov = ptr::null::<IoVec>() as u64;
        }
        
        msgHdr.iovLen = iovs.len();

        let mut addr : [u8; SIZEOF_SOCKADDR] = [0; SIZEOF_SOCKADDR];
        if senderRequested {
            msgHdr.msgName = &mut addr[0] as * mut _ as u64;
            msgHdr.nameLen = SIZEOF_SOCKADDR as u32;
        }

        let mut controlVec: Vec<u8> = vec![0; controlDataLen];
        msgHdr.msgControlLen = controlDataLen;
        if msgHdr.msgControlLen != 0 {
            msgHdr.msgControl = &mut controlVec[0] as *mut _ as u64;
        } else {
            msgHdr.msgControl = ptr::null::<u8>() as u64;
        }

        let mut res = Kernel::HostSpace::IORecvMsg(self.fd, &mut msgHdr as *mut _ as u64, flags | MsgType::MSG_DONTWAIT, false) as i32;
        while res == -SysErr::EWOULDBLOCK && flags & MsgType::MSG_DONTWAIT == 0 {
            let general = task.blocker.generalEntry.clone();

            self.EventRegister(task, &general, EVENT_READ);
            defer!(self.EventUnregister(task, &general));
            match task.blocker.BlockWithMonoTimer(true, deadline) {
                Err(Error::ErrInterrupted) => {
                    return Err(Error::SysError(SysErr::ERESTARTSYS));
                }
                Err(Error::SysError(SysErr::ETIMEDOUT)) => {
                    return Err(Error::SysError(SysErr::EAGAIN));
                }
                Err(e) => {
                    return Err(e);
                }
                _ => ()
            }

            res = Kernel::HostSpace::IORecvMsg(self.fd, &mut msgHdr as *mut _ as u64, flags | MsgType::MSG_DONTWAIT, false) as i32;
        }

        if res < 0 {
            return Err(Error::SysError(-res as i32))
        }

        let msgFlags = msgHdr.msgFlags & !MsgType::MSG_CTRUNC;
        let senderAddr = if senderRequested
            // for tcp connect, recvmsg get nameLen=0 msg
            && msgHdr.nameLen >= 4 {

            let addr = GetAddr(addr[0] as i16, &addr[0..msgHdr.nameLen as usize])?;
            let l = addr.Len();
            Some((addr, l))
        } else {
            None
        };

        controlVec.resize(msgHdr.msgControlLen, 0);

        task.CopyDataOutToIovs(&buf.buf[0..res as usize], dsts)?;
        return Ok((res as i64, msgFlags, senderAddr, controlVec))
    }

    fn SendMsg(&self, task: &Task, srcs: &[IoVec], flags: i32, msgHdr: &mut MsgHdr, deadline: Option<Time>) -> Result<i64> {
        if self.SocketBufEnabled() {
            if msgHdr.msgName != 0 || msgHdr.msgControl != 0 {
                panic!("Hostnet Socketbuf doesn't supprot MsgHdr");
            }

            let len = Iovs(srcs).Count();
            let mut count = 0;
            let mut srcs = srcs;
            let mut tmp;

            loop {
                loop {
                    match IOURING.BufSockWrite(task, self, srcs) {
                        Err(Error::SysError(SysErr::EWOULDBLOCK)) => {
                            if count > 0 {
                                return Ok(count)
                            }

                            if flags & MsgType::MSG_DONTWAIT != 0 {
                                return Err(Error::SysError(SysErr::EWOULDBLOCK))
                            }

                            break;
                        }
                        Err(e) => {
                            if count > 0 {
                                return Ok(count)
                            }

                            return Err(e)
                        },
                        Ok(n) => {
                            count += n;
                            if count == len as i64 {
                                return Ok(count)
                            }
                            tmp = Iovs(srcs).DropFirst(n as usize);
                            srcs = &mut tmp;
                        },
                    }
                }

                let general = task.blocker.generalEntry.clone();
                self.EventRegister(task, &general, EVENT_WRITE);
                defer!(self.EventUnregister(task, &general));

                match task.blocker.BlockWithMonoTimer(true, deadline) {
                    Err(Error::SysError(SysErr::ETIMEDOUT)) => {
                        if count > 0 {
                            return Ok(count)
                        }
                        return Err(Error::SysError(SysErr::EWOULDBLOCK));
                    }
                    Err(e) => {
                        if count > 0 {
                            return Ok(count)
                        }
                        return Err(e);
                    }
                    _ => ()
                }
            }

        }

        if flags & !(MsgType::MSG_DONTWAIT | MsgType::MSG_EOR | MsgType::MSG_FASTOPEN | MsgType::MSG_MORE | MsgType::MSG_NOSIGNAL) != 0 {
            return Err(Error::SysError(SysErr::EINVAL))
        }
        
        /*defer!(task.GetMut().iovs.clear());
        task.V2PIovs(srcs, false, &mut task.GetMut().iovs)?;
        let iovs = &task.GetMut().iovs;*/

        let size = IoVec::NumBytes(srcs);
        let mut buf = DataBuff::New(size);
        let iovs = buf.Iovs();

        task.CopyDataInFromIovs(&mut buf.buf, srcs)?;

        if IoVec::NumBytes(srcs) != 0 {
            msgHdr.iov = &iovs[0] as *const _ as u64;
        } else {
            msgHdr.iov = ptr::null::<IoVec>() as u64;
        }
        msgHdr.iovLen = iovs.len();
        msgHdr.msgFlags = 0;

        let mut res = Kernel::HostSpace::IOSendMsg(self.fd, msgHdr as *const _ as u64, flags | MsgType::MSG_DONTWAIT, false) as i32;
        while res == -SysErr::EWOULDBLOCK && flags & MsgType::MSG_DONTWAIT == 0 {
            let general = task.blocker.generalEntry.clone();

            self.EventRegister(task, &general, EVENT_WRITE);
            defer!(self.EventUnregister(task, &general));
            match task.blocker.BlockWithMonoTimer(true, deadline) {
                Err(e) => {
                    return Err(e);
                }
                _ => ()
            }

            res = Kernel::HostSpace::IOSendMsg(self.fd, msgHdr as *const _ as u64, flags | MsgType::MSG_DONTWAIT, false) as i32;
        }

        if res < 0 {
            return Err(Error::SysError(-res as i32))
        }

        return Ok(res as i64)
    }

    fn SetRecvTimeout(&self, ns: i64) {
        self.recv.store(ns, Ordering::Relaxed)
    }

    fn SetSendTimeout(&self, ns: i64) {
        self.send.store(ns, Ordering::Relaxed)
    }

    fn RecvTimeout(&self) -> i64 {
        return self.recv.load(Ordering::Relaxed)
    }

    fn SendTimeout(&self) -> i64 {
        return self.send.load(Ordering::Relaxed)
    }
}

pub struct SocketProvider {
    pub family: i32,
}

impl Provider for SocketProvider {
    fn Socket(&self, task: &Task, stype: i32, protocol: i32) -> Result<Option<Arc<File>>> {
        let stype = stype & SocketType::SOCK_TYPE_MASK;

        let res = Kernel::HostSpace::Socket(self.family, stype | SocketFlags::SOCK_CLOEXEC, protocol);
        if res < 0 {
            return Err(Error::SysError(-res as i32))
        }

        let fd = res as i32;

        let file = newSocketFile(task, self.family, fd, stype & SocketType::SOCK_TYPE_MASK, stype & SocketFlags::SOCK_NONBLOCK != 0, false, None)?;
        return Ok(Some(Arc::new(file)))
    }

    fn Pair(&self, _task: &Task, _stype: i32, _protocol: i32) -> Result<Option<(Arc<File>, Arc<File>)>> {
        /*if self.family == AFType::AF_UNIX {
            let fds: [i32; 2] = [0; 2];

            let res = Kernel::HostSpace::SocketPair(self.family, stype | SocketFlags::SOCK_CLOEXEC, protocol, &fds[0] as *const _ as u64);
            if res < 0 {
                return Err(Error::SysError(-res as i32))
            }

            let file0 = newSocketFile(task, self.family, fds[0], stype & SocketFlags::SOCK_NONBLOCK != 0)?;
            let file1 = newSocketFile(task, self.family, fds[1], stype & SocketFlags::SOCK_NONBLOCK != 0)?;

            return Ok(Some((Arc::new(file0), Arc::new(file1))));
        }*/

        return Err(Error::SysError(SysErr::EOPNOTSUPP))
    }
}

pub fn Init() {
    for family in [AFType::AF_INET, AFType::AF_INET6, AFType::AF_NETLINK].iter() {
        FAMILIAES.write().RegisterProvider(*family, Box::new(SocketProvider { family: *family }))
    }
}