use super::path::Path;
use crate::{
    device::DeviceDriverInfo,
    error::{Error, Result},
    kwarn,
    sync::mutex::Mutex,
};
use alloc::{
    boxed::Box,
    collections::{btree_map::BTreeMap, vec_deque::VecDeque},
    string::{String, ToString},
    vec::Vec,
};
use core::{
    cmp::min,
    fmt,
    sync::atomic::{AtomicUsize, Ordering},
};

static VFS: Mutex<VirtualFileSystem> = Mutex::new(VirtualFileSystem::new());

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceFileDescriptor {
    pub device_driver_info: fn() -> Result<DeviceDriverInfo>,
    pub open: fn() -> Result<()>,
    pub close: fn() -> Result<()>,
    pub read: fn(usize, usize) -> Result<Vec<u8>>,
    pub write: fn(&[u8]) -> Result<()>,
}

#[derive(Debug)]
struct PipeBuffer {
    buf: VecDeque<u8>,
    write_closed: bool,
}

impl Default for PipeBuffer {
    fn default() -> Self {
        Self {
            buf: VecDeque::new(),
            write_closed: false,
        }
    }
}

#[derive(Clone)]
enum PipeEnd {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VfsFileId(usize);

impl VfsFileId {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileDescriptorNumber(usize);

impl fmt::Display for FileDescriptorNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FileDescriptorNumber {
    pub const STDIN: Self = Self(0);
    pub const STDOUT: Self = Self(1);
    pub const STDERR: Self = Self(2);

    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(3);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }

    pub fn get(&self) -> usize {
        self.0
    }
}

impl FileDescriptorNumber {
    pub fn try_new(value: i32) -> Result<Self> {
        if value < 0 {
            return Err(VirtualFileSystemError::InvalidFileDescriptorNumber.into());
        }
        Ok(Self(value as usize))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileDescriptorStatus {
    Open,
    Close,
}

#[derive(Clone)]
enum FileBacking {
    Vfs(VfsFileId),
    Fs { mount_id: VfsFileId, rel_path: Path },
}

#[derive(Debug, Clone, Copy)]
pub enum SeekFrom {
    Start(i64),
    Current(i64),
    End(i64),
}

#[derive(Clone)]
pub struct FileDescriptor {
    num: FileDescriptorNumber,
    status: FileDescriptorStatus,
    backing: FileBacking,
    offset: usize,
    pipe_end: Option<PipeEnd>,
    fs_content_cache: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum VfsFileType {
    VirtualFile, // for file system
    DeviceFile(DeviceFileDescriptor),
    Pipe,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsFileType {
    File,
    Directory,
}

pub struct FsMetaData {
    pub file_type: FsFileType,
    pub size: usize,
}

pub trait FileSystem {
    fn read_entry_names(&self, path: &Path) -> Result<Vec<String>>;
    fn read_file(&self, path: &Path, offset: usize, max_len: usize) -> Result<Vec<u8>>;
    fn write_file(&self, path: &Path, offset: usize, data: &[u8]) -> Result<()>;
    fn metadata(&self, path: &Path) -> Result<FsMetaData>;
}

struct FileInfo {
    ty: VfsFileType,
    name: String,
    fs: Option<Box<dyn FileSystem>>,
    parent: VfsFileId,
    children: Vec<VfsFileId>,
    buf: Option<Vec<u8>>,
    pipe_buf: Option<PipeBuffer>,
}

impl FileInfo {
    fn new(ty: VfsFileType, name: String, parent: VfsFileId) -> Self {
        Self {
            ty,
            name,
            fs: None,
            parent,
            children: Vec::new(),
            buf: None,
            pipe_buf: None,
        }
    }

    fn check_integrity(&self) -> Result<()> {
        if self.ty != VfsFileType::Directory && (!self.children.is_empty() || self.fs.is_some()) {
            return Err(VirtualFileSystemError::invalid_type(&self.ty, None).into());
        }

        if self.name.is_empty()
            || [Path::CURRENT_DIR, Path::PARENT_DIR].contains(&self.name.as_str())
        {
            return Err(VirtualFileSystemError::InvalidFileName.into());
        }

        Ok(())
    }
}

enum Resolved<'a> {
    Vfs(VfsFileId, &'a FileInfo),
    Fs {
        mount_id: VfsFileId,
        fs: &'a dyn FileSystem,
        rel_path: Path,
        metadata: FsMetaData,
    },
}

impl<'a> Resolved<'a> {
    fn vfs_type(&self) -> VfsFileType {
        match self {
            Self::Vfs(_, file_ref) => file_ref.ty.clone(),
            Self::Fs { metadata, .. } => fs_file_type_as_vfs(&metadata.file_type),
        }
    }

    fn backing(&self) -> FileBacking {
        match self {
            Self::Vfs(id, _) => FileBacking::Vfs(*id),
            Self::Fs {
                mount_id, rel_path, ..
            } => FileBacking::Fs {
                mount_id: *mount_id,
                rel_path: rel_path.clone(),
            },
        }
    }
}

fn fs_file_type_as_vfs(ty: &FsFileType) -> VfsFileType {
    match ty {
        FsFileType::Directory => VfsFileType::Directory,
        FsFileType::File => VfsFileType::VirtualFile,
    }
}

fn resolve_mount(mount_id: VfsFileId, fs: &dyn FileSystem, rel_path: Path) -> Option<Resolved<'_>> {
    let metadata = if rel_path.as_str() == Path::ROOT {
        FsMetaData {
            file_type: FsFileType::Directory,
            size: 0,
        }
    } else {
        fs.metadata(&rel_path).ok()?
    };

    Some(Resolved::Fs {
        mount_id,
        fs,
        rel_path,
        metadata,
    })
}

#[derive(Debug)]
pub enum VirtualFileSystemError {
    NoSuchFileOrDirectory(Option<Path>),
    FileOrDirectoryAlreadyExists(Path),
    InvalidFileType { ty: VfsFileType, path: Option<Path> },
    BlockingFileResource(FileDescriptorNumber),
    ReleasedFileResource(FileDescriptorNumber),
    InvalidFileName,
    InvalidFileDescriptorNumber,
}

impl VirtualFileSystemError {
    fn invalid_type(ty: &VfsFileType, path: Option<Path>) -> Self {
        Self::InvalidFileType {
            ty: ty.clone(),
            path,
        }
    }
}

impl core::fmt::Display for VirtualFileSystemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSuchFileOrDirectory(path) => {
                write!(f, "No such file or directory")?;

                if let Some(p) = path {
                    write!(f, ": {}", p)?;
                }

                Ok(())
            }
            Self::FileOrDirectoryAlreadyExists(path) => {
                write!(f, "File or directory already exists: {}", path)
            }
            Self::InvalidFileType { ty, path } => {
                write!(f, "Invalid file type: Type: {:?}", ty)?;

                if let Some(p) = path {
                    write!(f, ", Path: {}", p)?;
                }

                Ok(())
            }
            Self::BlockingFileResource(fd) => write!(f, "Blocking file resource: {}", fd),
            Self::ReleasedFileResource(fd) => write!(f, "Released file resource: {}", fd),
            Self::InvalidFileName => write!(f, "Invalid file name"),
            Self::InvalidFileDescriptorNumber => write!(f, "Invalid file descriptor number"),
        }
    }
}

struct VirtualFileSystem {
    cwd_path: Option<Path>,
    root_id: Option<VfsFileId>,
    files: BTreeMap<VfsFileId, FileInfo>,
    fds: Vec<FileDescriptor>,
}

impl VirtualFileSystem {
    const fn new() -> Self {
        Self {
            cwd_path: None,
            root_id: None,
            files: BTreeMap::new(),
            fds: Vec::new(),
        }
    }

    fn insert_file(&mut self, id: VfsFileId, info: FileInfo) -> Result<()> {
        info.check_integrity()?;

        // root
        if id == info.parent {
            self.root_id = Some(id);
            self.cwd_path = Some(Path::root());
        }

        self.files.insert(id, info);

        Ok(())
    }

    fn init(&mut self) -> Result<()> {
        let root_dir_path = Path::root();
        let mnt_dir_path = root_dir_path.join("mnt");
        let dev_dir_path = root_dir_path.join("dev");
        let initramfs_dir_path = mnt_dir_path.join("initramfs");

        // create root directory
        let root_id = VfsFileId::new();
        let root_dir = FileInfo::new(VfsFileType::Directory, root_dir_path.name(), root_id);
        self.insert_file(root_id, root_dir)?;

        self.mkdir(&mnt_dir_path)?;
        self.mkdir(&dev_dir_path)?;
        self.mkdir(&initramfs_dir_path)?;

        Ok(())
    }

    fn find_file(&self, id: VfsFileId) -> Option<&FileInfo> {
        self.files.get(&id)
    }

    fn find_file_mut(&mut self, id: VfsFileId) -> Option<&mut FileInfo> {
        self.files.get_mut(&id)
    }

    fn absolutize(&self, path: &Path) -> Option<Path> {
        if path.is_abs() {
            Some(path.normalize())
        } else {
            Some(self.cwd_path.as_ref()?.join(path.as_str()))
        }
    }

    fn find_file_by_path<'a>(&'a self, path: &Path) -> Option<Resolved<'a>> {
        let abs_path = self.absolutize(path)?;
        let mut file_id = self.root_id?;
        let mut file_ref = self.find_file(file_id)?;
        let names = abs_path.names();

        for (i, name) in names.iter().enumerate() {
            if let Some(fs) = &file_ref.fs {
                let rel_path = Path::new(format!(
                    "{}{}",
                    Path::ROOT,
                    names[i..].join(&Path::SEPARATOR.to_string())
                ));

                return resolve_mount(file_id, fs.as_ref(), rel_path);
            }

            let mut found = false;
            for child_id in &file_ref.children {
                let child_ref = self.find_file(*child_id)?;
                if child_ref.name == *name {
                    file_ref = child_ref;
                    file_id = *child_id;
                    found = true;
                    break;
                }
            }

            if !found {
                return None;
            }
        }

        if let Some(fs) = &file_ref.fs {
            return resolve_mount(file_id, fs.as_ref(), Path::root());
        }

        Some(Resolved::Vfs(file_id, file_ref))
    }

    fn find_file_by_path_mut(&mut self, path: &Path) -> Option<(VfsFileId, &mut FileInfo)> {
        let file_id = match self.find_file_by_path(path)? {
            Resolved::Vfs(id, _) => id,
            Resolved::Fs { .. } => return None,
        };
        let file_ref_mut = self.find_file_mut(file_id)?;
        Some((file_id, file_ref_mut))
    }

    fn entry_names(&self, path: &Path) -> Result<Vec<String>> {
        let resolved =
            self.find_file_by_path(path)
                .ok_or(VirtualFileSystemError::NoSuchFileOrDirectory(Some(
                    path.clone(),
                )))?;

        if resolved.vfs_type() != VfsFileType::Directory {
            return Err(VirtualFileSystemError::invalid_type(
                &resolved.vfs_type(),
                Some(path.clone()),
            )
            .into());
        }

        let mut names = match resolved {
            Resolved::Vfs(_, file_ref) => file_ref
                .children
                .iter()
                .filter_map(|id| self.find_file(*id))
                .map(|f| f.name.clone())
                .collect(),
            Resolved::Fs { fs, rel_path, .. } => fs.read_entry_names(&rel_path)?,
        };
        names.retain(|n| n.as_str() != Path::CURRENT_DIR && n.as_str() != Path::PARENT_DIR);

        Ok(names)
    }

    fn chdir(&mut self, path: &Path) -> Result<()> {
        let abs_path = self.absolutize(path).ok_or(Error::NotInitialized)?;

        let resolved = self.find_file_by_path(&abs_path).ok_or(
            VirtualFileSystemError::NoSuchFileOrDirectory(Some(path.clone())),
        )?;

        if resolved.vfs_type() != VfsFileType::Directory {
            return Err(VirtualFileSystemError::invalid_type(
                &resolved.vfs_type(),
                Some(path.clone()),
            )
            .into());
        }

        self.cwd_path = Some(abs_path);

        Ok(())
    }

    fn add_file(&mut self, path: &Path, file_ty: VfsFileType) -> Result<()> {
        if self.root_id.is_none() {
            return Err(Error::NotInitialized.into());
        }

        let (parent_id, parent_ref) = self.find_file_by_path_mut(&path.parent()).ok_or(
            VirtualFileSystemError::NoSuchFileOrDirectory(Some(path.clone())),
        )?;

        if parent_ref.ty != VfsFileType::Directory {
            return Err(
                VirtualFileSystemError::invalid_type(&parent_ref.ty, Some(path.clone())).into(),
            );
        }

        let file_name = path.name();
        let children_ids = parent_ref.children.clone();
        if children_ids
            .iter()
            .any(|id| self.find_file(*id).map_or(false, |f| f.name == file_name))
        {
            return Err(VirtualFileSystemError::FileOrDirectoryAlreadyExists(path.clone()).into());
        }

        let file_id = VfsFileId::new();
        let file_ref = FileInfo::new(file_ty, file_name, parent_id);

        // reacquire parent_ref
        let (_, parent_ref) = self.find_file_by_path_mut(&path.parent()).unwrap();
        parent_ref.children.push(file_id);

        self.insert_file(file_id, file_ref)?;

        Ok(())
    }

    fn mkdir(&mut self, path: &Path) -> Result<()> {
        self.add_file(path, VfsFileType::Directory)
    }

    fn add_dev_file(&mut self, desc: DeviceFileDescriptor, file_name: &str) -> Result<()> {
        let dev_file_path = Path::root().join("dev").join(file_name);
        self.add_file(&dev_file_path, VfsFileType::DeviceFile(desc))
    }

    fn mount_fs(&mut self, path: &Path, fs: Box<dyn FileSystem>) -> Result<()> {
        let (_, mp_file_ref) = self.find_file_by_path_mut(path).ok_or(
            VirtualFileSystemError::NoSuchFileOrDirectory(Some(path.clone())),
        )?;

        if mp_file_ref.ty != VfsFileType::Directory {
            return Err(
                VirtualFileSystemError::invalid_type(&mp_file_ref.ty, Some(path.clone())).into(),
            );
        }

        mp_file_ref.fs = Some(fs);

        Ok(())
    }

    fn abs_path_by_file(&self, file_ref: &FileInfo) -> Option<Path> {
        let mut s = file_ref.name.clone();

        let mut parent_id = file_ref.parent;
        loop {
            if parent_id == self.root_id? {
                break;
            }

            let parent_ref = self.find_file(parent_id)?;
            s = format!("{}{}{}", parent_ref.name, Path::SEPARATOR, s);
            parent_id = parent_ref.parent;
        }

        s = format!("{}{}", Path::ROOT, s);
        Some(Path::new(s).normalize())
    }

    fn mount_fs_ref(&self, mount_id: VfsFileId) -> Result<&dyn FileSystem> {
        self.file_ref(mount_id)?
            .fs
            .as_deref()
            .ok_or(VirtualFileSystemError::NoSuchFileOrDirectory(None).into())
    }

    fn open_file(&mut self, path: &Path, create: bool) -> Result<FileDescriptor> {
        let backing = match self.find_file_by_path(path) {
            Some(Resolved::Vfs(file_id, file_ref)) => {
                if !matches!(
                    file_ref.ty,
                    VfsFileType::VirtualFile | VfsFileType::DeviceFile(_)
                ) {
                    return Err(VirtualFileSystemError::invalid_type(
                        &file_ref.ty,
                        Some(path.clone()),
                    )
                    .into());
                }

                if let Some(fd) = self
                    .fds
                    .iter()
                    .find(|fd| matches!(&fd.backing, FileBacking::Vfs(id) if *id == file_id))
                {
                    return Err(VirtualFileSystemError::BlockingFileResource(fd.num).into());
                }

                if let VfsFileType::DeviceFile(desc) = &file_ref.ty {
                    (desc.open)()?;
                }

                FileBacking::Vfs(file_id)
            }
            Some(resolved @ Resolved::Fs { .. }) => {
                if resolved.vfs_type() != VfsFileType::VirtualFile {
                    return Err(VirtualFileSystemError::invalid_type(
                        &resolved.vfs_type(),
                        Some(path.clone()),
                    )
                    .into());
                }

                resolved.backing()
            }
            None if create => {
                self.add_file(path, VfsFileType::VirtualFile)?;
                match self.find_file_by_path(path) {
                    Some(Resolved::Vfs(file_id, _)) => FileBacking::Vfs(file_id),
                    _ => {
                        return Err(VirtualFileSystemError::NoSuchFileOrDirectory(Some(
                            path.clone(),
                        ))
                        .into())
                    }
                }
            }
            None => {
                return Err(
                    VirtualFileSystemError::NoSuchFileOrDirectory(Some(path.clone())).into(),
                )
            }
        };

        let fd_num = FileDescriptorNumber::new();
        let fd = FileDescriptor {
            num: fd_num,
            status: FileDescriptorStatus::Open,
            backing,
            offset: 0,
            pipe_end: None,
            fs_content_cache: None,
        };

        self.fds.push(fd.clone());
        Ok(fd)
    }

    fn close_file(&mut self, fd_num: FileDescriptorNumber) -> Result<()> {
        if let Some(index) = self.fds.iter().position(|f| f.num == fd_num) {
            let backing = self.fds[index].backing.clone();

            if let FileBacking::Vfs(file_id) = backing {
                let file_ref = self.file_ref(file_id)?;

                if let VfsFileType::DeviceFile(desc) = &file_ref.ty {
                    (desc.close)()?;
                }

                // pipe
                let pipe_end = self.fds[index].pipe_end.clone();
                if matches!(pipe_end, Some(PipeEnd::Write)) {
                    if let Some(f) = self.find_file_mut(file_id) {
                        if let Some(pipe) = f.pipe_buf.as_mut() {
                            pipe.write_closed = true;
                        }
                    }
                }
            }

            self.fds.remove(index);
        } else {
            return Err(VirtualFileSystemError::ReleasedFileResource(fd_num).into());
        }

        Ok(())
    }

    fn file_desc(&self, fd_num: FileDescriptorNumber) -> Result<&FileDescriptor> {
        self.fds
            .iter()
            .find(|f| f.num == fd_num && f.status == FileDescriptorStatus::Open)
            .ok_or(VirtualFileSystemError::ReleasedFileResource(fd_num).into())
    }

    fn file_desc_mut(&mut self, fd_num: FileDescriptorNumber) -> Result<&mut FileDescriptor> {
        self.fds
            .iter_mut()
            .find(|f| f.num == fd_num && f.status == FileDescriptorStatus::Open)
            .ok_or(VirtualFileSystemError::ReleasedFileResource(fd_num).into())
    }

    fn file_ref(&self, file_id: VfsFileId) -> Result<&FileInfo> {
        self.find_file(file_id)
            .ok_or(VirtualFileSystemError::NoSuchFileOrDirectory(None).into())
    }

    fn file_ref_mut(&mut self, file_id: VfsFileId) -> Result<&mut FileInfo> {
        self.find_file_mut(file_id)
            .ok_or(VirtualFileSystemError::NoSuchFileOrDirectory(None).into())
    }

    fn read_file(&mut self, fd_num: FileDescriptorNumber, max_len: usize) -> Result<Vec<u8>> {
        let backing = self.file_desc(fd_num)?.backing.clone();

        match backing {
            FileBacking::Fs { mount_id, rel_path } => {
                let offset = self.file_desc(fd_num)?.offset;

                if self.file_desc(fd_num)?.fs_content_cache.is_none() {
                    let content =
                        self.mount_fs_ref(mount_id)?
                            .read_file(&rel_path, 0, usize::MAX)?;
                    self.file_desc_mut(fd_num)?.fs_content_cache = Some(content);
                }

                let content = self.file_desc(fd_num)?.fs_content_cache.as_ref().unwrap();
                let start = min(offset, content.len());
                let end = min(start.saturating_add(max_len), content.len());
                let bytes = content[start..end].to_vec();

                self.file_desc_mut(fd_num)?.offset = start + bytes.len();
                Ok(bytes)
            }
            FileBacking::Vfs(file_id) => {
                if matches!(
                    self.find_file(file_id).map(|f| &f.ty),
                    Some(VfsFileType::Pipe)
                ) {
                    let pipe = self
                        .file_ref_mut(file_id)?
                        .pipe_buf
                        .as_mut()
                        .ok_or(VirtualFileSystemError::NoSuchFileOrDirectory(None))?;
                    let bytes: Vec<u8> = pipe.buf.drain(..min(max_len, pipe.buf.len())).collect();
                    return Ok(bytes);
                }

                let offset = self.file_desc(fd_num)?.offset;

                let device_read = match self.find_file(file_id).map(|f| &f.ty) {
                    Some(VfsFileType::DeviceFile(desc)) => Some(desc.read),
                    _ => None,
                };

                if let Some(device_read) = device_read {
                    let bytes = device_read(offset, max_len)?;
                    self.file_desc_mut(fd_num)?.offset = offset + bytes.len();
                    return Ok(bytes);
                }

                let bytes = self.file_ref(file_id)?.buf.clone().unwrap_or_default();
                let start = min(offset, bytes.len());
                let end = min(start.saturating_add(max_len), bytes.len());
                let bytes_slice = &bytes.as_slice()[start..end];
                self.file_desc_mut(fd_num)?.offset = start + bytes_slice.len();

                Ok(bytes_slice.to_vec())
            }
        }
    }

    fn write_file(&mut self, fd_num: FileDescriptorNumber, data: &[u8]) -> Result<()> {
        let backing = self.file_desc(fd_num)?.backing.clone();
        let offset = self.file_desc(fd_num)?.offset;

        match backing {
            FileBacking::Fs { mount_id, rel_path } => {
                self.mount_fs_ref(mount_id)?
                    .write_file(&rel_path, offset, data)?;
                self.file_desc_mut(fd_num)?.offset = offset + data.len();
                Ok(())
            }
            FileBacking::Vfs(file_id) => {
                let ty = self.file_ref(file_id)?.ty.clone();
                let is_virtual_file = matches!(ty, VfsFileType::VirtualFile);

                let file_path = if matches!(ty, VfsFileType::DeviceFile(_) | VfsFileType::Pipe) {
                    None
                } else {
                    self.abs_path_by_file(self.file_ref(file_id)?)
                };
                let file_ref_mut = self.file_ref_mut(file_id)?;

                match &mut file_ref_mut.ty {
                    VfsFileType::VirtualFile => {
                        // TODO
                        kwarn!(
                            "VFS: Write to File system is unimplemented. Using temporary buffer: {}",
                            file_path.clone().unwrap_or_else(Path::root)
                        );

                        let buf_mut = file_ref_mut.buf.get_or_insert_with(Vec::new);
                        let end = offset + data.len();

                        if end > buf_mut.len() {
                            buf_mut.resize(end, 0);
                        }

                        buf_mut[offset..end].copy_from_slice(data);
                    }
                    VfsFileType::DeviceFile(desc) => (desc.write)(data)?,
                    VfsFileType::Pipe => {
                        let pipe = file_ref_mut
                            .pipe_buf
                            .as_mut()
                            .ok_or(VirtualFileSystemError::NoSuchFileOrDirectory(None))?;
                        pipe.buf.extend(data);
                    }
                    _ => {
                        return Err(VirtualFileSystemError::invalid_type(
                            &file_ref_mut.ty,
                            file_path,
                        )
                        .into())
                    }
                }

                if is_virtual_file {
                    self.file_desc_mut(fd_num)?.offset = offset + data.len();
                }

                Ok(())
            }
        }
    }

    fn file_size(&self, fd_num: FileDescriptorNumber) -> Result<usize> {
        match self.file_desc(fd_num)?.backing.clone() {
            FileBacking::Fs { mount_id, rel_path } => {
                let metadata = self.mount_fs_ref(mount_id)?.metadata(&rel_path)?;
                Ok(metadata.size)
            }
            FileBacking::Vfs(file_id) => {
                let file_ref = self.file_ref(file_id)?;

                match &file_ref.ty {
                    VfsFileType::VirtualFile => Ok(file_ref.buf.as_ref().map_or(0, |b| b.len())),
                    VfsFileType::DeviceFile(_) => Ok(0),
                    _ => {
                        let file_path = self.abs_path_by_file(file_ref);
                        Err(VirtualFileSystemError::invalid_type(&file_ref.ty, file_path).into())
                    }
                }
            }
        }
    }

    fn seek(&mut self, fd_num: FileDescriptorNumber, pos: SeekFrom) -> Result<usize> {
        let cur = self.file_desc(fd_num)?.offset as i64;

        let target = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::Current(offset) => cur.saturating_add(offset),
            SeekFrom::End(offset) => (self.file_size(fd_num)? as i64).saturating_add(offset),
        };

        if target < 0 {
            return Err(Error::InvalidData.with_context("seek target"));
        }
        let target = target as usize;

        self.file_desc_mut(fd_num)?.offset = target;

        Ok(target)
    }

    fn create_pipe(&mut self) -> Result<(FileDescriptor, FileDescriptor)> {
        let root_id = self.root_id.ok_or(Error::NotInitialized)?;

        let file_id = VfsFileId::new();
        let mut info = FileInfo::new(VfsFileType::Pipe, format!("pipe:{}", file_id.0), root_id);
        info.pipe_buf = Some(PipeBuffer::default());
        self.files.insert(file_id, info);

        let read_fd = FileDescriptor {
            num: FileDescriptorNumber::new(),
            status: FileDescriptorStatus::Open,
            backing: FileBacking::Vfs(file_id),
            offset: 0,
            pipe_end: Some(PipeEnd::Read),
            fs_content_cache: None,
        };
        let write_fd = FileDescriptor {
            num: FileDescriptorNumber::new(),
            status: FileDescriptorStatus::Open,
            backing: FileBacking::Vfs(file_id),
            offset: 0,
            pipe_end: Some(PipeEnd::Write),
            fs_content_cache: None,
        };
        self.fds.push(read_fd.clone());
        self.fds.push(write_fd.clone());

        Ok((read_fd, write_fd))
    }
}

pub fn init() -> Result<()> {
    let mut vfs = VFS.spin_lock();
    vfs.init()
}

pub fn chdir(path: &Path) -> Result<()> {
    let mut vfs = VFS.spin_lock();
    vfs.chdir(path)
}

pub fn mount_fs(path: &Path, fs: Box<dyn FileSystem>) -> Result<()> {
    let mut vfs = VFS.spin_lock();
    vfs.mount_fs(path, fs)
}

pub fn entry_names(path: &Path) -> Result<Vec<String>> {
    let vfs = VFS.spin_lock();
    vfs.entry_names(path)
}

pub fn cwd_path() -> Result<Path> {
    let vfs = VFS.spin_lock();
    vfs.cwd_path.clone().ok_or(Error::NotInitialized.into())
}

pub fn open_file(path: &Path, create: bool) -> Result<FileDescriptorNumber> {
    let mut vfs = VFS.spin_lock();
    let fd = vfs.open_file(path, create)?;
    Ok(fd.num)
}

pub fn close_file(fd_num: FileDescriptorNumber) -> Result<()> {
    let mut vfs = VFS.spin_lock();
    vfs.close_file(fd_num)
}

pub fn read_file(fd_num: FileDescriptorNumber, buf_len: usize) -> Result<Vec<u8>> {
    let mut vfs = VFS.spin_lock();
    vfs.read_file(fd_num, buf_len)
}

pub fn write_file(fd_num: FileDescriptorNumber, data: &[u8]) -> Result<()> {
    let mut vfs = VFS.spin_lock();
    vfs.write_file(fd_num, data)
}

pub fn file_size(fd_num: FileDescriptorNumber) -> Result<usize> {
    let vfs = VFS.spin_lock();
    vfs.file_size(fd_num)
}

pub fn seek(fd_num: FileDescriptorNumber, pos: SeekFrom) -> Result<usize> {
    let mut vfs = VFS.spin_lock();
    vfs.seek(fd_num, pos)
}

// TODO
pub fn create_file(path: &Path) -> Result<()> {
    let mut vfs = VFS.spin_lock();
    vfs.add_file(path, VfsFileType::VirtualFile)
}

pub fn add_dev_file(desc: DeviceFileDescriptor, file_name: &str) -> Result<()> {
    let mut vfs = VFS.spin_lock();
    vfs.add_dev_file(desc, file_name)
}

pub fn create_pipe() -> Result<(FileDescriptorNumber, FileDescriptorNumber)> {
    let mut vfs = VFS.spin_lock();
    let (read_fd, write_fd) = vfs.create_pipe()?;
    Ok((read_fd.num, write_fd.num))
}
