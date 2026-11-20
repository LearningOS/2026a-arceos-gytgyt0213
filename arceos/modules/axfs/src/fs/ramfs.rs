//! Mount the in-tree RAM filesystem without changing the course dependency graph.

use alloc::sync::Arc;
use axfs_vfs::{VfsNodeRef, VfsOps, VfsResult};
use axsync::{Mutex, MutexGuard};

// The shared node implementation only requires read/write access to a lock.
// Use the kernel's existing synchronization primitive in this module.
struct RwLock<T>(Mutex<T>);

impl<T> RwLock<T> {
    const fn new(value: T) -> Self {
        Self(Mutex::new(value))
    }

    fn read(&self) -> MutexGuard<'_, T> {
        self.0.lock()
    }

    fn write(&self) -> MutexGuard<'_, T> {
        self.0.lock()
    }
}

#[path = "../../../../axfs_ramfs/src/dir.rs"]
mod dir;
#[path = "../../../../axfs_ramfs/src/file.rs"]
mod file;

/// An in-memory filesystem backed by the local directory and file nodes.
pub struct RamFileSystem {
    root: Arc<dir::DirNode>,
}

impl RamFileSystem {
    pub fn new() -> Self {
        Self {
            root: dir::DirNode::new(None),
        }
    }
}

impl VfsOps for RamFileSystem {
    fn mount(&self, _path: &str, mount_point: VfsNodeRef) -> VfsResult {
        self.root.set_parent(mount_point.parent().as_ref());
        Ok(())
    }

    fn root_dir(&self) -> VfsNodeRef {
        self.root.clone()
    }
}
