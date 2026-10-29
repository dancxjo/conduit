use super::*;
use sha2::{Digest, Sha256};
use std::{
    ffi::{CStr, CString},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::{FileExt, MetadataExt},
    },
};

pub(super) struct Directory {
    descriptor: File,
}
pub(super) struct Entry {
    pub id: ResourceEntryId,
    pub name: CString,
    // Retention prevents inode-number reuse while this membership exists.
    identity: File,
}
impl Directory {
    pub fn prepare(descriptor: File) -> Result<Self, NamespaceRefusal> {
        if !descriptor.metadata()?.is_dir() {
            return Err(NamespaceRefusal::WrongRoot);
        }
        Ok(Self { descriptor })
    }
    pub fn snapshot(
        &self,
        limits: NamespaceLimits,
        scope: &[&str],
    ) -> Result<Vec<Entry>, NamespaceRefusal> {
        // An independent directory stream avoids changing the retained fd's offset.
        let scan = self.open(c".", libc::O_RDONLY | libc::O_DIRECTORY)?;
        let raw = std::os::fd::IntoRawFd::into_raw_fd(scan);
        // SAFETY: raw is an owned directory fd. fdopendir takes ownership on success.
        let stream = unsafe { libc::fdopendir(raw) };
        if stream.is_null() {
            let error = std::io::Error::last_os_error();
            drop(unsafe { File::from_raw_fd(raw) });
            return Err(error.into());
        }
        struct Stream(*mut libc::DIR);
        impl Drop for Stream {
            fn drop(&mut self) {
                unsafe {
                    libc::closedir(self.0);
                }
            }
        }
        let stream = Stream(stream);
        let mut entries = Vec::with_capacity(limits.maximum_entries as usize);
        let mut names = 0usize;
        loop {
            // Linux readdir distinguishes EOF from error through errno.
            unsafe {
                *libc::__errno_location() = 0;
            }
            let item = unsafe { libc::readdir(stream.0) };
            if item.is_null() {
                let errno = unsafe { *libc::__errno_location() };
                if errno != 0 {
                    return Err(std::io::Error::from_raw_os_error(errno).into());
                }
                break;
            }
            let name = unsafe { CStr::from_ptr((*item).d_name.as_ptr()) };
            if name.to_bytes() == b"." || name.to_bytes() == b".." {
                continue;
            }
            if entries.len() == limits.maximum_entries as usize {
                return Err(NamespaceRefusal::EntryBound);
            }
            names = names
                .checked_add(name.to_bytes().len())
                .ok_or(NamespaceRefusal::NameBound)?;
            if name.to_str().is_err() {
                return Err(NamespaceRefusal::UnsupportedName);
            }
            if names > limits.maximum_name_bytes as usize {
                return Err(NamespaceRefusal::NameBound);
            }
            let identity = self.open(name, libc::O_PATH)?;
            // Reopen only our already-resolved regular inode. Resolving a
            // caller name with O_RDONLY first could touch a raced device.
            // This Linux preparation profile requires mounted /proc/self/fd.
            let identity = if identity.metadata()?.is_file() {
                File::open(format!("/proc/self/fd/{}", identity.as_raw_fd()))?
            } else {
                identity
            };
            let mut hash = Sha256::new();
            hash.update(b"conduit/scoped-namespace-entry@1");
            for part in scope {
                hash.update((part.len() as u64).to_le_bytes());
                hash.update(part.as_bytes());
            }
            hash.update(name.to_bytes());
            let metadata = identity.metadata()?;
            hash.update(metadata.dev().to_le_bytes());
            hash.update(metadata.ino().to_le_bytes());
            let digest = hash.finalize();
            entries.push(Entry {
                id: ResourceEntryId(format!("entry/{digest:x}")),
                name: name.to_owned(),
                identity,
            });
        }
        entries.sort_by(|left, right| left.name.as_bytes().cmp(right.name.as_bytes()));
        Ok(entries)
    }
    fn open(&self, name: &CStr, flags: i32) -> Result<File, NamespaceRefusal> {
        // SAFETY: the descriptor stays alive and the name is terminated. Every
        // caller supplies a fixed direct component, never caller path syntax.
        let fd = unsafe {
            libc::openat(
                self.descriptor.as_raw_fd(),
                name.as_ptr(),
                flags | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
            )
        };
        if fd < 0 {
            Err(std::io::Error::last_os_error().into())
        } else {
            Ok(unsafe { File::from_raw_fd(fd) })
        }
    }
    pub fn read(
        &self,
        entry: &Entry,
        output: &mut [u8],
        maximum: u32,
    ) -> Result<usize, NamespaceRefusal> {
        let current = self.open(&entry.name, libc::O_PATH)?;
        let metadata = current.metadata()?;
        let identity = entry.identity.metadata()?;
        if metadata.dev() != identity.dev() || metadata.ino() != identity.ino() {
            return Err(NamespaceRefusal::ChangedEntry);
        }
        if !metadata.is_file() {
            return Err(NamespaceRefusal::NotRegularFile);
        }
        let bound = output.len().min(maximum as usize);
        if metadata.len() > bound as u64 {
            return Err(NamespaceRefusal::ByteBound);
        }
        let mut count = 0;
        while count < bound {
            let read = entry
                .identity
                .read_at(&mut output[count..bound], count as u64)?;
            if read == 0 {
                return Ok(count);
            }
            count += read;
        }
        let mut probe = [0];
        if entry.identity.read_at(&mut probe, count as u64)? != 0 {
            return Err(NamespaceRefusal::ByteBound);
        }
        Ok(count)
    }
}
