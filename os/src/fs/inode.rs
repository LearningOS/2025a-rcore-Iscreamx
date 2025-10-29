//! `Arc<Inode>` -> `OSInodeInner`: In order to open files concurrently
//! we need to wrap `Inode` into `Arc`,but `Mutex` in `Inode` prevents
//! file systems from being accessed simultaneously
//!
//! `UPSafeCell<OSInodeInner>` -> `OSInode`: for static `ROOT_INODE`,we
//! need to wrap `OSInodeInner` into `UPSafeCell`
use super::{File, Stat, StatMode};
use crate::drivers::BLOCK_DEVICE;
use crate::mm::UserBuffer;
use crate::sync::UPSafeCell;
use alloc::sync::Arc;
use alloc::vec::Vec;
use bitflags::*;
use easy_fs::{EasyFileSystem, Inode};
use lazy_static::*;

/// inode in memory
/// A wrapper around a filesystem inode
/// to implement File trait atop
pub struct OSInode {
    readable: bool,
    writable: bool,
    inner: UPSafeCell<OSInodeInner>,
}
/// The OS inode inner in 'UPSafeCell'
pub struct OSInodeInner {
    offset: usize,
    stat: Stat,
    inode: Arc<Inode>,
}

impl OSInode {
    /// create a new inode in memory
    pub fn new(readable: bool, writable: bool, inode: Arc<Inode>) -> Self {
        let is_dir = inode.is_dir();
        let nlink = inode.get_nlink();
        
        let stat = Stat {
            dev: 0,
            ino: 0,  // 暂时填 0
            mode: if is_dir {
                StatMode::DIR
            } else {
                StatMode::FILE
            },
            nlink,
            ..Default::default()
        };
        
        Self {
            readable,
            writable,
            inner: unsafe { 
                UPSafeCell::new(OSInodeInner { 
                    offset: 0, 
                    inode,
                    stat,
                }) 
            },
        }
    }
    /// Create a hard link to this file
    pub fn create_link(&self, new_name: &str) -> bool {
        let inner = self.inner.exclusive_access();
        let inode = inner.inode.clone();
        drop(inner);
        
        if ROOT_INODE.link(new_name, &inode).is_ok() {
            inode.modify_disk_inode(|disk_inode| {
                disk_inode.inc_nlink();
            });
            
            let mut inner = self.inner.exclusive_access();
            inner.stat.nlink += 1;
            
            true
        } else {
            false
        }
    }
    /// Decrease link count when unlinking
    pub fn decrease_link(&self) -> u32 {
        let inner = self.inner.exclusive_access();
        
        // 减少磁盘上的 nlink
        let nlink = inner.inode.modify_disk_inode(|disk_inode| {
            disk_inode.dec_nlink()
        });
        
        drop(inner);
        
        let mut inner = self.inner.exclusive_access();
        inner.stat.nlink = nlink;
        
        nlink
    }
    /// read all data from the inode
    pub fn read_all(&self) -> Vec<u8> {
        let mut inner = self.inner.exclusive_access();
        let mut buffer: Vec<u8> = Vec::with_capacity(512);
        buffer.resize(512, 0);
        let mut v: Vec<u8> = Vec::new();
        loop {
            let len = inner.inode.read_at(inner.offset, &mut buffer);
            if len == 0 {
                break;
            }
            inner.offset += len;
            v.extend_from_slice(&buffer[..len]);
        }
        v
    }
}

lazy_static! {
    pub static ref ROOT_INODE: Arc<Inode> = {
        let efs = EasyFileSystem::open(BLOCK_DEVICE.clone());
        Arc::new(EasyFileSystem::root_inode(&efs))
    };
}

/// List all apps in the root directory
pub fn list_apps() {
    println!("/**** APPS ****");
    for app in ROOT_INODE.ls() {
        println!("{}", app);
    }
    println!("**************/");
}

bitflags! {
    ///  The flags argument to the open() system call is constructed by ORing together zero or more of the following values:
    pub struct OpenFlags: u32 {
        /// readyonly
        const RDONLY = 0;
        /// writeonly
        const WRONLY = 1 << 0;
        /// read and write
        const RDWR = 1 << 1;
        /// create new file
        const CREATE = 1 << 9;
        /// truncate file size to 0
        const TRUNC = 1 << 10;
    }
}

impl OpenFlags {
    /// Do not check validity for simplicity
    /// Return (readable, writable)
    pub fn read_write(&self) -> (bool, bool) {
        if self.is_empty() {
            (true, false)
        } else if self.contains(Self::WRONLY) {
            (false, true)
        } else {
            (true, true)
        }
    }
}

/// Open a file
pub fn open_file(name: &str, flags: OpenFlags) -> Option<Arc<OSInode>> {
    let (readable, writable) = flags.read_write();
    if flags.contains(OpenFlags::CREATE) {
        if let Some(inode) = ROOT_INODE.find(name) {
            // clear size
            inode.clear();
            Some(Arc::new(OSInode::new(readable, writable, inode)))
        } else {
            // create file
            ROOT_INODE
                .create(name)
                .map(|inode| Arc::new(OSInode::new(readable, writable, inode)))
        }
    } else {
        ROOT_INODE.find(name).map(|inode| {
            if flags.contains(OpenFlags::TRUNC) {
                inode.clear();
            }
            Arc::new(OSInode::new(readable, writable, inode))
        })
    }
}

/// Create a hard link
pub fn linkat(old_name: &str, new_name: &str) -> bool {
    println!("[DEBUG] linkat: old_name={}, new_name={}", old_name, new_name);
    
    println!("[DEBUG] linkat: finding old_name");
    if let Some(old_inode) = ROOT_INODE.find(old_name) {
        println!("[DEBUG] linkat: old_name found");
        
        println!("[DEBUG] linkat: checking if new_name exists");
        if ROOT_INODE.find(new_name).is_some() {
            println!("[DEBUG] linkat: new_name already exists, returning false");
            return false;
        }
        
        println!("[DEBUG] linkat: creating link");
        if ROOT_INODE.link(new_name, &old_inode).is_ok() {
            println!("[DEBUG] linkat: link created, increasing nlink");
            
            old_inode.modify_disk_inode(|disk_inode| {
                disk_inode.inc_nlink();
            });
            println!("[DEBUG] linkat: success");
            true
        } else {
            println!("[DEBUG] linkat: link creation failed");
            false
        }
    } else {
        println!("[DEBUG] linkat: old_name not found");
        false
    }
}

/// Remove a hard link
pub fn unlinkat(name: &str) -> bool {
    println!("[DEBUG] unlinkat: name={}", name);
    
    println!("[DEBUG] unlinkat: finding file");
    if let Some(inode) = ROOT_INODE.find(name) {
        println!("[DEBUG] unlinkat: file found, decreasing nlink");
        
        let new_nlink = inode.modify_disk_inode(|disk_inode| {
            disk_inode.dec_nlink()
        });
        
        println!("[DEBUG] unlinkat: new_nlink={}", new_nlink);
        
        println!("[DEBUG] unlinkat: removing directory entry");
        ROOT_INODE.unlink(name);
        
        if new_nlink == 0 {
            println!("[DEBUG] unlinkat: nlink is 0, clearing file");
            inode.clear();
        }
        
        println!("[DEBUG] unlinkat: success");
        true
    } else {
        println!("[DEBUG] unlinkat: file not found");
        false
    }
}

impl File for OSInode {
    fn readable(&self) -> bool {
        self.readable
    }
    fn writable(&self) -> bool {
        self.writable
    }
    fn read(&self, mut buf: UserBuffer) -> usize {
        let mut inner = self.inner.exclusive_access();
        let mut total_read_size = 0usize;
        for slice in buf.buffers.iter_mut() {
            let read_size = inner.inode.read_at(inner.offset, *slice);
            if read_size == 0 {
                break;
            }
            inner.offset += read_size;
            total_read_size += read_size;
        }
        total_read_size
    }
    fn write(&self, buf: UserBuffer) -> usize {
        let mut inner = self.inner.exclusive_access();
        let mut total_write_size = 0usize;
        for slice in buf.buffers.iter() {
            let write_size = inner.inode.write_at(inner.offset, *slice);
            assert_eq!(write_size, slice.len());
            inner.offset += write_size;
            total_write_size += write_size;
        }
        total_write_size
    }
    fn read_stat(&self) -> Stat {
        let mut inner = self.inner.exclusive_access();
        
        let nlink = inner.inode.read_disk_inode(|disk_inode| {
            disk_inode.get_nlink()
        });
        
        // 更新内存中的 stat
        inner.stat.nlink = nlink;
        
        inner.stat.clone()
    }
}
