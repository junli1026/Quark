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
use alloc::string::ToString;
use spin::RwLock;
//use spin::Mutex;
use core::ops::Deref;
use alloc::collections::btree_map::BTreeMap;
use alloc::vec::Vec;
use alloc::sync::Arc;
use core::any::Any;
use core::sync::atomic::AtomicI64;

use super::file::*;
use super::super::mount::*;
use super::super::attr::*;
use super::super::inode::*;
use super::super::flags::*;
use super::super::file::*;
use super::super::dirent::*;
use super::super::super::qlib::linux_def::*;
use super::super::super::task::*;
use super::super::super::qlib::common::*;
use super::super::super::qlib::auth::*;
use super::super::super::kernel::time::*;
use super::super::super::kernel::waiter::lock::*;
use super::super::host::hostinodeop::*;
use super::super::super::id_mgr::*;

pub struct InodeSimpleExtendedAttributesInternal {
    pub xattrs: BTreeMap<String, String>
}

impl Default for InodeSimpleExtendedAttributesInternal {
    fn default() -> Self {
        return Self {
            xattrs: BTreeMap::new(),
        }
    }
}

pub struct InodeSimpleExtendedAttributes(pub RwLock<InodeSimpleExtendedAttributesInternal>);

impl Default for InodeSimpleExtendedAttributes {
    fn default() -> Self {
        return Self(RwLock::new(Default::default()))
    }
}

impl Deref for InodeSimpleExtendedAttributes {
    type Target = RwLock<InodeSimpleExtendedAttributesInternal>;

    fn deref(&self) -> &RwLock<InodeSimpleExtendedAttributesInternal> {
        &self.0
    }
}

impl InodeSimpleExtendedAttributes {
    fn Getxattr(&self, _dir: &Inode, name: &str) -> Result<String> {
        match self.read().xattrs.get(name) {
            None => Err(Error::SysError(SysErr::ENOATTR)),
            Some(s) => Ok(s.clone())
        }
    }

    fn Setxattr(&self, _dir: &mut Inode, name: &str, value: &str) -> Result<()> {
        self.write().xattrs.insert(name.to_string(), value.to_string());
        return Ok(())
    }

    fn Listxattr(&self, _dir: &Inode) -> Result<Vec<String>> {
        let mut res = Vec::new();
        for (name, _) in &self.read().xattrs {
            res.push(name.clone());
        }

        return Ok(res)
    }
}

pub struct InodeStaticFileGetterInternal {
    pub content: Arc<Vec<u8>>
}

impl Default for InodeStaticFileGetterInternal {
    fn default() -> Self {
        return Self {
            content: Arc::new(Vec::new())
        }
    }
}

pub struct InodeStaticFileGetter(pub RwLock<InodeStaticFileGetterInternal>);

impl Default for InodeStaticFileGetter {
    fn default() -> Self {
        return Self(RwLock::new(Default::default()))
    }
}

impl Deref for InodeStaticFileGetter {
    type Target = RwLock<InodeStaticFileGetterInternal>;

    fn deref(&self) -> &RwLock<InodeStaticFileGetterInternal> {
        &self.0
    }
}

impl InodeStaticFileGetter {
    fn GetFile(&self, _dir: &Inode, dirent: &Dirent, flags: FileFlags) -> Result<File> {
        return Ok(File(Arc::new(FileInternal {
            UniqueId: UniqueID(),
            Dirent: dirent.clone(),
            flags: QMutex::new((flags.clone(), None)),
            offsetLock: QLock::default(),
            offset: AtomicI64::new(0),
            FileOp: Arc::new(StaticFileOps { content: self.read().content.clone() }),
        })))
    }
}

pub struct InodeNotDirectoryInternal {}

impl InodeNotDirectoryInternal {
    fn Lookup(&self, _task: &Task, _dir: &Inode, _name: &str) -> Result<Dirent> {
        return Err(Error::SysError(SysErr::ENOTDIR))
    }

    fn Create(&self, _task: &Task, _dir: &mut Inode, _name: &str, _flags: &FileFlags, _perm: &FilePermissions) -> Result<File> {
        return Err(Error::SysError(SysErr::ENOTDIR))
    }

    fn CreateDirectory(&self, _task: &Task, _dir: &mut Inode, _name: &str, _perm: &FilePermissions) -> Result<()> {
        return Err(Error::SysError(SysErr::ENOTDIR))
    }

    fn CreateLink(&self, _task: &Task, _dir: &mut Inode, _oldname: &str, _newname: &str) -> Result<()> {
        return Err(Error::SysError(SysErr::ENOTDIR))
    }

    fn CreateHardLink(&self, _task: &Task, _dir: &mut Inode, _target: &Inode, _name: &str) -> Result<()> {
        return Err(Error::SysError(SysErr::ENOTDIR))
    }

    fn CreateFifo(&self, _task: &Task, _dir: &mut Inode, _name: &str, _perm: &FilePermissions) -> Result<()> {
        return Err(Error::SysError(SysErr::ENOTDIR))
    }

    fn Remove(&self, _task: &Task, _dir: &mut Inode, _name: &str) -> Result<()> {
        return Err(Error::SysError(SysErr::ENOTDIR))
    }

    fn RemoveDirectory(&self, _task: &Task, _dir: &mut Inode, _name: &str) -> Result<()> {
        return Err(Error::SysError(SysErr::ENOTDIR))
    }

    fn Rename(&self, _task: &Task, _dir: &mut Inode, _oldParent: &Inode, _oldname: &str, _newParent: &Inode, _newname: &str, _replacement: bool) -> Result<()> {
        return Err(Error::SysError(SysErr::EINVAL))
    }
}

pub struct InodeNotTruncatable {}

impl InodeNotTruncatable {
    fn Truncate(&self, _task: &Task, _dir: &mut Inode, _size: i64) -> Result<()> {
        return Err(Error::SysError(SysErr::EINVAL))
    }
}

pub struct InodeIsDirTruncate {}

impl InodeIsDirTruncate {
    fn Truncate(&self, _task: &Task, _dir: &mut Inode, _size: i64) -> Result<()> {
        return Err(Error::SysError(SysErr::EISDIR))
    }
}

pub struct InodeNoopTruncate {}

impl InodeNoopTruncate {
    fn Truncate(&self, _task: &Task, _dir: &mut Inode, _size: i64) -> Result<()> {
        return Ok(())
    }
}

pub struct InodeNotRenameable {}

impl InodeNotRenameable {
    fn Rename(&self, _task: &Task, _dir: &mut Inode, _oldParent: &Inode, _oldname: &str, _newParent: &Inode, _newname: &str, _replacement: bool) -> Result<()> {
        return Err(Error::SysError(SysErr::EINVAL))
    }
}

pub struct InodeNotOpenable {}

impl InodeNotOpenable {
    fn GetFile(&self, _dir: &Inode, _dirent: &Dirent, _flags: FileFlags) -> Result<Arc<QMutex<File>>> {
        return Err(Error::SysError(SysErr::EIO))
    }
}

pub struct InodeNotVirtual {}

impl InodeNotVirtual {
    fn IsVirtual(&self) -> bool {
        return false
    }
}

pub struct InodeVirtual {}

impl InodeVirtual {
    fn IsVirtual(&self) -> bool {
        return true
    }
}

pub struct InodeNotSymlink {}

impl InodeNotSymlink {
    fn ReadLink(&self, _task: &Task,_dir: &Inode) -> Result<String> {
        return Err(Error::SysError(SysErr::ENOLINK))
    }

    fn GetLink(&self, _task: &Task, _dir: &Inode) -> Result<Dirent> {
        return Err(Error::SysError(SysErr::ENOLINK))
    }
}

pub struct InodeNoExtendedAttributes {}

impl InodeNoExtendedAttributes {
    fn Getxattr(&self, _dir: &Inode, _name: &str) -> Result<String> {
        return Err(Error::SysError(SysErr::EOPNOTSUPP))
    }

    fn Setxattr(&self, _dir: &mut Inode, _name: &str, _value: &str) -> Result<()> {
        return Err(Error::SysError(SysErr::EOPNOTSUPP))
    }

    fn Listxattr(&self, _dir: &Inode) -> Result<Vec<String>> {
        return Err(Error::SysError(SysErr::EOPNOTSUPP))
    }
}

pub struct InodeGenericChecker {}

impl InodeGenericChecker {
    fn Check(&self, task: &Task, inode: &Inode, reqPerms: &PermMask) -> Result<bool> {
        return ContextCanAccessFile(task, inode, reqPerms)
    }
}

pub struct InodeDenyWriteChecker {}

impl InodeDenyWriteChecker {
    fn Check(&self, task: &Task, inode: &Inode, reqPerms: &PermMask) -> Result<bool> {
        if reqPerms.write {
            return Ok(false)
        }

        return ContextCanAccessFile(task, inode, reqPerms)
    }
}

pub struct InodeNotAllocatable {}

impl InodeNotAllocatable {
    fn Allocate(&self, _task: &Task, _dir: &mut Inode, _offset: i64, _length: i64) -> Result<()> {
        return Err(Error::SysError(SysErr::EOPNOTSUPP))
    }
}

pub struct InodeNoopAllocate {}

impl InodeNoopAllocate {
    fn Allocate(&self, _task: &Task, _dir: &mut Inode, _offset: i64, _length: i64) -> Result<()> {
        return Ok(())
    }
}

pub struct InodeIsDirAllocate {}

impl InodeIsDirAllocate {
    fn Allocate(&self, _task: &Task, _dir: &mut Inode, _offset: i64, _length: i64) -> Result<()> {
        return Err(Error::SysError(SysErr::EISDIR))
    }
}

pub struct InodeNotMappable {}

impl InodeNotMappable {
    fn Mmap(&self, _task: &Task, _len: u64, _hugePage: bool, _offset: u64, _share: bool) -> Result<u64> {
        return Err(Error::SysError(SysErr::EACCES))
    }
}