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
use alloc::collections::btree_map::BTreeMap;
use alloc::string::ToString;

use super::super::super::qlib::device::*;
use super::super::super::qlib::mutex::*;
use super::super::super::qlib::linux_def::*;
use super::super::super::qlib::auth::*;
use super::super::super::task::*;
use super::super::mount::*;
use super::super::inode::*;
use super::super::attr::*;
use super::super::ramfs::dir::*;
use super::super::ramfs::symlink::*;
use super::null::*;
use super::zero::*;
use super::full::*;
use super::random::*;
use super::tty::*;

const MEM_DEV_MAJOR: u16 = 1;

// Mem device minors.
const NULL_DEV_MINOR: u32 = 3;
const ZERO_DEV_MINOR: u32 = 5;
const FULL_DEV_MINOR: u32 = 7;
const RANDOM_DEV_MINOR: u32 = 8;
const URANDOM_DEV_MINOR: u32 = 9;

fn NewTTYDevice(iops: &Arc<TTYDevice>, msrc: &Arc<QMutex<MountSource>>) -> Inode {
    let deviceId = DEV_DEVICE.lock().id.DeviceID();
    let inodeId = DEV_DEVICE.lock().NextIno();

    let stableAttr = StableAttr {
        Type: InodeType::CharacterDevice,
        DeviceId: deviceId,
        InodeId: inodeId,
        BlockSize: MemoryDef::PAGE_SIZE as i64,
        DeviceFileMajor: 5 ,
        DeviceFileMinor: 0,
    };

    let inodeInternal = InodeIntern {
        InodeOp: iops.clone(),
        StableAttr: stableAttr,
        LockCtx: LockCtx::default(),
        MountSource: msrc.clone(),
        Overlay: None,
        ..Default::default()
    };

    return Inode(Arc::new(QMutex::new(inodeInternal)))
}

fn NewNullDevice(iops: &Arc<NullDevice>, msrc: &Arc<QMutex<MountSource>>) -> Inode {
    let deviceId = DEV_DEVICE.lock().id.DeviceID();
    let inodeId = DEV_DEVICE.lock().NextIno();

    let stableAttr = StableAttr {
        Type: InodeType::CharacterDevice,
        DeviceId: deviceId,
        InodeId: inodeId,
        BlockSize: MemoryDef::PAGE_SIZE as i64,
        DeviceFileMajor: MEM_DEV_MAJOR,
        DeviceFileMinor: NULL_DEV_MINOR,
    };

    let inodeInternal = InodeIntern {
        InodeOp: iops.clone(),
        StableAttr: stableAttr,
        LockCtx: LockCtx::default(),
        MountSource: msrc.clone(),
        Overlay: None,
        ..Default::default()
    };

    return Inode(Arc::new(QMutex::new(inodeInternal)))
}

fn NewZeroDevice(iops: &Arc<ZeroDevice>, msrc: &Arc<QMutex<MountSource>>) -> Inode {
    let deviceId = DEV_DEVICE.lock().id.DeviceID();
    let inodeId = DEV_DEVICE.lock().NextIno();

    let stableAttr = StableAttr {
        Type: InodeType::CharacterDevice,
        DeviceId: deviceId,
        InodeId: inodeId,
        BlockSize: MemoryDef::PAGE_SIZE as i64,
        DeviceFileMajor: MEM_DEV_MAJOR,
        DeviceFileMinor: ZERO_DEV_MINOR,
    };

    let inodeInternal = InodeIntern {
        InodeOp: iops.clone(),
        StableAttr: stableAttr,
        LockCtx: LockCtx::default(),
        MountSource: msrc.clone(),
        Overlay: None,
        ..Default::default()
    };

    return Inode(Arc::new(QMutex::new(inodeInternal)))
}

fn NewFullDevice(iops: &Arc<FullDevice>, msrc: &Arc<QMutex<MountSource>>) -> Inode {
    let deviceId = DEV_DEVICE.lock().id.DeviceID();
    let inodeId = DEV_DEVICE.lock().NextIno();

    let stableAttr = StableAttr {
        Type: InodeType::CharacterDevice,
        DeviceId: deviceId,
        InodeId: inodeId,
        BlockSize: MemoryDef::PAGE_SIZE as i64,
        DeviceFileMajor: MEM_DEV_MAJOR,
        DeviceFileMinor: FULL_DEV_MINOR,
    };

    let inodeInternal = InodeIntern {
        InodeOp: iops.clone(),
        StableAttr: stableAttr,
        LockCtx: LockCtx::default(),
        MountSource: msrc.clone(),
        Overlay: None,
        ..Default::default()
    };

    return Inode(Arc::new(QMutex::new(inodeInternal)))
}

fn NewRandomDevice(iops: &Arc<RandomDevice>, msrc: &Arc<QMutex<MountSource>>, minor: u32) -> Inode {
    let deviceId = DEV_DEVICE.lock().id.DeviceID();
    let inodeId = DEV_DEVICE.lock().NextIno();

    let stableAttr = StableAttr {
        Type: InodeType::CharacterDevice,
        DeviceId: deviceId,
        InodeId: inodeId,
        BlockSize: MemoryDef::PAGE_SIZE as i64,
        DeviceFileMajor: MEM_DEV_MAJOR,
        DeviceFileMinor: minor,
    };

    let inodeInternal = InodeIntern {
        InodeOp: iops.clone(),
        StableAttr: stableAttr,
        LockCtx: LockCtx::default(),
        MountSource: msrc.clone(),
        Overlay: None,
        ..Default::default()
    };

    return Inode(Arc::new(QMutex::new(inodeInternal)))
}

fn NewDirectory(task: &Task, msrc: &Arc<QMutex<MountSource>>) -> Inode {
    let iops = Dir::New(task, BTreeMap::new(), &ROOT_OWNER, &FilePermissions::FromMode(FileMode(0o0555)));

    let deviceId = PROC_DEVICE.lock().id.DeviceID();
    let inodeId = PROC_DEVICE.lock().NextIno();

    let stableAttr = StableAttr {
        Type: InodeType::Directory,
        DeviceId: deviceId,
        InodeId: inodeId,
        BlockSize: MemoryDef::PAGE_SIZE as i64,
        DeviceFileMajor: 0,
        DeviceFileMinor: 0,
    };

    let inodeInternal = InodeIntern {
        InodeOp: Arc::new(iops),
        StableAttr: stableAttr,
        LockCtx: LockCtx::default(),
        MountSource: msrc.clone(),
        Overlay: None,
        ..Default::default()
    };

    return Inode(Arc::new(QMutex::new(inodeInternal)))
}

fn NewSymlink(task: &Task, target: &str, msrc: &Arc<QMutex<MountSource>>) -> Inode {
    let iops = Symlink::New(task, &ROOT_OWNER, target);

    let deviceId = DEV_DEVICE.lock().id.DeviceID();
    let inodeId = DEV_DEVICE.lock().NextIno();

    let stableAttr = StableAttr {
        Type: InodeType::Symlink,
        DeviceId: deviceId,
        InodeId: inodeId,
        BlockSize: MemoryDef::PAGE_SIZE as i64,
        DeviceFileMajor: 0,
        DeviceFileMinor: 0,
    };

    let inodeInternal = InodeIntern {
        InodeOp: Arc::new(iops),
        StableAttr: stableAttr,
        LockCtx: LockCtx::default(),
        MountSource: msrc.clone(),
        Overlay: None,
        ..Default::default()
    };

    return Inode(Arc::new(QMutex::new(inodeInternal)))
}

pub fn NewDev(task: &Task, msrc: &Arc<QMutex<MountSource>>) -> Inode {
    let mut contents = BTreeMap::new();

    contents.insert("fd".to_string(), NewSymlink(task, &"/proc/self/fd".to_string(), msrc));
    contents.insert("stdin".to_string(), NewSymlink(task, &"/proc/self/fd/0".to_string(), msrc));
    contents.insert("stdout".to_string(), NewSymlink(task, &"/proc/self/fd/1".to_string(), msrc));
    contents.insert("stderr".to_string(), NewSymlink(task, &"/proc/self/fd/2".to_string(), msrc));

    contents.insert("null".to_string(), NewNullDevice(&Arc::new(NullDevice::New(task, &ROOT_OWNER, &FileMode(0o0666))), msrc));
    contents.insert("zero".to_string(), NewZeroDevice(&Arc::new(ZeroDevice::New(task, &ROOT_OWNER, &FileMode(0o0666))), msrc));
    contents.insert("full".to_string(), NewFullDevice(&Arc::new(FullDevice::New(task, &ROOT_OWNER, &FileMode(0o0666))), msrc));

    // This is not as good as /dev/random in linux because go
    // runtime uses sys_random and /dev/urandom internally.
    // According to 'man 4 random', this will be sufficient unless
    // application uses this to generate long-lived GPG/SSL/SSH
    // keys.
    contents.insert("random".to_string(), NewRandomDevice(&Arc::new(RandomDevice::New(task, &ROOT_OWNER, &FileMode(0o0666))), msrc, RANDOM_DEV_MINOR));
    contents.insert("urandom".to_string(), NewRandomDevice(&Arc::new(RandomDevice::New(task, &ROOT_OWNER, &FileMode(0o0666))), msrc, URANDOM_DEV_MINOR));

    // A devpts is typically mounted at /dev/pts to provide
    // pseudoterminal support. Place an empty directory there for
    // the devpts to be mounted over.
    //contents.insert("pts".to_string(), NewDirectory(task, msrc));

    // Similarly, applications expect a ptmx device at /dev/ptmx
    // connected to the terminals provided by /dev/pts/. Rather
    // than creating a device directly (which requires a hairy
    // lookup on open to determine if a devpts exists), just create
    // a symlink to the ptmx provided by devpts. (The Linux devpts
    // documentation recommends this).
    //
    // If no devpts is mounted, this will simply be a dangling
    // symlink, which is fine.
    contents.insert("ptmx".to_string(), NewSymlink(task, &"pts/ptmx".to_string(), msrc));

    let ttyDevice = TTYDevice::New(task, &ROOT_OWNER, &FileMode(0o0666));
    contents.insert("tty".to_string(), NewTTYDevice(&Arc::new(ttyDevice), msrc));

    let iops = Dir::New(task, contents, &ROOT_OWNER, &FilePermissions::FromMode(FileMode(0o0555)));

    let deviceId = DEV_DEVICE.lock().id.DeviceID();
    let inodeId = DEV_DEVICE.lock().NextIno();

    let stableAttr = StableAttr {
        Type: InodeType::Directory,
        DeviceId: deviceId,
        InodeId: inodeId,
        BlockSize: MemoryDef::PAGE_SIZE as i64,
        DeviceFileMajor: 0,
        DeviceFileMinor: 0,
    };

    let inodeInternal = InodeIntern {
        InodeOp: Arc::new(iops),
        StableAttr: stableAttr,
        LockCtx: LockCtx::default(),
        MountSource: msrc.clone(),
        Overlay: None,
        ..Default::default()
    };

    return Inode(Arc::new(QMutex::new(inodeInternal)))
}