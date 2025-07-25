use super::{fat::Fat, path::Path};
use crate::{
    device::DeviceDriverInfo,
    error::{Error, Result},
    fs::fat::dir_entry::Attribute,
    sync::mutex::Mutex,
    warn,
};
use alloc::{collections::btree_map::BTreeMap, string::String, vec::Vec};
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

static mut VFS: Mutex<VirtualFileSystem> = Mutex::new(VirtualFileSystem::new());

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct VfsFileId(usize);

impl VfsFileId {
    fn new() -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileDescriptorNumber(u64);

impl FileDescriptorNumber {
    pub const STDIN: Self = Self(0);
    pub const STDOUT: Self = Self(1);
    pub const STDERR: Self = Self(2);

    pub fn new() -> Self {
        static NEXT_NUM: AtomicU64 = AtomicU64::new(3);
        Self(NEXT_NUM.fetch_add(1, Ordering::Relaxed))
    }

    pub fn new_val(value: i32) -> Result<Self> {
        if value < 0 {
            return Err(Error::Failed("Invalid file descriptor number"));
        }

        Ok(Self(value as u64))
    }

    pub fn get(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileDescriptorStatus {
    Open,
    Close,
}

#[derive(Debug, Clone)]
pub struct FileDescriptor {
    num: FileDescriptorNumber,
    status: FileDescriptorStatus,
    file_id: VfsFileId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceFileDescriptor {
    pub get_device_driver_info: fn() -> Result<DeviceDriverInfo>,
    pub open: fn() -> Result<()>,
    pub close: fn() -> Result<()>,
    pub read: fn() -> Result<Vec<u8>>,
    pub write: fn(&[u8]) -> Result<()>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum VfsFileType {
    VirtualFile, // for file system
    DeviceFile(DeviceFileDescriptor),
    Directory,
}

#[derive(Debug, PartialEq, Eq)]
pub enum FileSystem {
    Fat(Fat),
}

#[derive(Debug)]
struct FileInfo {
    ty: VfsFileType,
    name: String,
    fs: Option<FileSystem>,
    parent: VfsFileId,
    children: Vec<VfsFileId>,
    data: Option<Vec<u8>>,
}

impl FileInfo {
    fn new(ty: VfsFileType, name: String, parent: VfsFileId) -> Self {
        Self {
            ty,
            name,
            fs: None,
            parent,
            children: Vec::new(),
            data: None,
        }
    }

    fn check_integrity(&self) -> Result<()> {
        if self.ty != VfsFileType::Directory && !self.children.is_empty() {
            return Err(Error::Failed("File must be a directory"));
        }

        if self.fs.is_some() && self.ty != VfsFileType::Directory {
            return Err(Error::Failed("File system mountpoint must be a directory"));
        }

        if self.name.is_empty() {
            return Err(Error::Failed("File name must not be empty"));
        }

        if ["", Path::CURRENT_DIR, Path::PARENT_DIR].contains(&self.name.as_str()) {
            return Err(Error::Failed("File name is invalid"));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum VirtualFileSystemError {
    NoSuchFileOrDirectoryError(Option<Path>),
    FileOrDirectoryAlreadyExistsError(Path),
    InvalidFileTypeError((VfsFileType, Option<Path>)),
    BlockingFileResourceError(FileDescriptorNumber),
    ReleasedFileResourceError(FileDescriptorNumber),
}

#[derive(Debug)]
struct VirtualFileSystem {
    cwd_id: Option<VfsFileId>,
    root_id: Option<VfsFileId>,
    files: BTreeMap<VfsFileId, FileInfo>,
    fds: Vec<FileDescriptor>,
}

impl VirtualFileSystem {
    const fn new() -> Self {
        Self {
            cwd_id: None,
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
            self.cwd_id = Some(id);
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

    fn find_file(&self, id: &VfsFileId) -> Option<&FileInfo> {
        self.files.get(id)
    }

    fn find_file_mut(&mut self, id: &VfsFileId) -> Option<&mut FileInfo> {
        self.files.get_mut(id)
    }

    fn find_file_by_path(&self, path: &Path) -> Option<(VfsFileId, &FileInfo)> {
        let normalized_path = path.normalize();
        let mut file_id = if normalized_path.is_abs() {
            self.root_id
        } else {
            self.cwd_id
        }?;
        let mut file_ref = self.find_file(&file_id)?;

        for name in normalized_path.names() {
            match name {
                Path::CURRENT_DIR => continue,
                Path::PARENT_DIR => {
                    let parent_id = file_ref.parent;
                    file_ref = self.find_file(&parent_id)?;
                    file_id = parent_id;
                    continue;
                }
                _ => (),
            }

            let mut found = false;
            for child_id in &file_ref.children {
                let child_ref = self.find_file(child_id)?;
                if child_ref.name == name {
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

        Some((file_id, file_ref))
    }

    fn find_file_by_path_mut(&mut self, path: &Path) -> Option<(VfsFileId, &mut FileInfo)> {
        let (file_id, _) = self.find_file_by_path(path)?;
        let file_ref_mut = self.find_file_mut(&file_id)?;
        Some((file_id, file_ref_mut))
    }

    fn files_by_path(&self, path: &Path) -> Result<Vec<&FileInfo>> {
        let mut files = Vec::new();

        let (_, file_ref) = self.find_file_by_path(&path).ok_or(
            VirtualFileSystemError::NoSuchFileOrDirectoryError(Some(path.clone())),
        )?;

        for child_id in &file_ref.children {
            if let Some(child_ref) = self.find_file(child_id) {
                files.push(child_ref);
            }
        }

        Ok(files)
    }

    fn chdir(&mut self, path: &Path) -> Result<()> {
        let (file_id, file_ref) = self.find_file_by_path(path).ok_or(
            VirtualFileSystemError::NoSuchFileOrDirectoryError(Some(path.clone())),
        )?;
        if file_ref.ty != VfsFileType::Directory {
            return Err(VirtualFileSystemError::InvalidFileTypeError((
                file_ref.ty.clone(),
                Some(path.clone()),
            ))
            .into());
        }

        self.cwd_id = Some(file_id);

        Ok(())
    }

    fn add_file(&mut self, path: &Path, file_ty: VfsFileType) -> Result<()> {
        if self.root_id.is_none() {
            return Err(Error::NotInitialized);
        }

        let (parent_id, parent_ref) = self.find_file_by_path(&path.parent()).ok_or(
            VirtualFileSystemError::NoSuchFileOrDirectoryError(Some(path.clone())),
        )?;

        if parent_ref.ty != VfsFileType::Directory {
            return Err(VirtualFileSystemError::InvalidFileTypeError((
                parent_ref.ty.clone(),
                Some(path.clone()),
            ))
            .into());
        }

        let file_name = path.name();
        let children_ids = parent_ref.children.clone();
        if children_ids
            .iter()
            .any(|id| self.find_file(id).map_or(false, |f| f.name == file_name))
        {
            return Err(
                VirtualFileSystemError::FileOrDirectoryAlreadyExistsError(path.clone()).into(),
            );
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

    fn mount_fs(&mut self, path: &Path, fs: FileSystem) -> Result<()> {
        let (mp_file_id, mp_file_ref) = self.find_file_by_path_mut(path).ok_or(
            VirtualFileSystemError::NoSuchFileOrDirectoryError(Some(path.clone())),
        )?;

        if mp_file_ref.ty != VfsFileType::Directory {
            return Err(VirtualFileSystemError::InvalidFileTypeError((
                mp_file_ref.ty.clone(),
                Some(path.clone()),
            ))
            .into());
        }

        // mount point's children are removed
        mp_file_ref.children.clear();

        // cache fs
        // TODO: use add_file()
        let cached_files: Vec<(VfsFileId, FileInfo)> = match &fs {
            FileSystem::Fat(fat) => {
                fn cache_recursively(
                    fat: &Fat,
                    dir_cluster_num: Option<usize>,
                    parent_file: (&VfsFileId, &mut FileInfo),
                ) -> Vec<(VfsFileId, FileInfo)> {
                    let (p_file_id, p_file_ref) = parent_file;

                    let mut files = Vec::new();
                    let metadata = fat.scan_dir(dir_cluster_num);
                    for meta in metadata {
                        match meta.name.trim() {
                            "." | ".." => continue,
                            _ => (),
                        }

                        let file_id = VfsFileId::new();
                        let mut file_info = FileInfo::new(
                            match meta.attr {
                                Attribute::Directory => VfsFileType::Directory,
                                _ => VfsFileType::VirtualFile,
                            },
                            meta.name,
                            *p_file_id,
                        );

                        if file_info.ty == VfsFileType::Directory {
                            let child_files = cache_recursively(
                                fat,
                                Some(meta.target_cluster_num),
                                (&file_id, &mut file_info),
                            );
                            files.extend(child_files);
                        }

                        files.push((file_id, file_info));
                        p_file_ref.children.push(file_id);
                    }

                    files
                }

                let files = cache_recursively(fat, None, (&mp_file_id, mp_file_ref));
                files
            }
        };

        mp_file_ref.fs = Some(fs);

        for (id, info) in cached_files {
            self.insert_file(id, info)?;
        }

        Ok(())
    }

    fn find_fs<'a>(&'a self, file_ref: &'a FileInfo) -> Option<(&'a FileSystem, Path)> {
        if let Some(fs) = &file_ref.fs {
            return Some((fs, self.abs_path_by_file(file_ref)?));
        }

        let mut parent_id = file_ref.parent;
        loop {
            let parent_ref = self.find_file(&parent_id)?;
            if let Some(fs) = &parent_ref.fs {
                return Some((fs, self.abs_path_by_file(parent_ref)?));
            }

            parent_id = parent_ref.parent;

            if parent_id == self.root_id? {
                break;
            }
        }

        None
    }

    fn abs_path_by_file(&self, file_ref: &FileInfo) -> Option<Path> {
        let mut s = file_ref.name.clone();

        let mut parent_id = file_ref.parent;
        loop {
            if parent_id == self.root_id? {
                break;
            }

            let parent_ref = self.find_file(&parent_id)?;
            s = format!("{}{}{}", parent_ref.name, Path::SEPARATOR, s);
            parent_id = parent_ref.parent;
        }

        s = format!("{}{}", Path::ROOT, s);
        Some(Path::new(s).normalize())
    }

    fn open_file(&mut self, path: &Path, create: bool) -> Result<FileDescriptor> {
        let file_id;
        let file_ref;

        if let Some((id, ref_)) = self.find_file_by_path(path) {
            file_id = id;
            file_ref = ref_;
        } else if create {
            self.add_file(path, VfsFileType::VirtualFile)?;
            (file_id, file_ref) = self.find_file_by_path(path).ok_or(
                VirtualFileSystemError::NoSuchFileOrDirectoryError(Some(path.clone())),
            )?;
        } else {
            return Err(
                VirtualFileSystemError::NoSuchFileOrDirectoryError(Some(path.clone())).into(),
            );
        }

        match &file_ref.ty {
            VfsFileType::VirtualFile | VfsFileType::DeviceFile(_) => (),
            _ => {
                return Err(VirtualFileSystemError::InvalidFileTypeError((
                    file_ref.ty.clone(),
                    Some(path.clone()),
                ))
                .into());
            }
        }

        if let Some(fd) = self.fds.iter().find(|fd| fd.file_id == file_id) {
            return Err(VirtualFileSystemError::BlockingFileResourceError(fd.num).into());
        }

        let fd_num = FileDescriptorNumber::new();
        let fd = FileDescriptor {
            num: fd_num,
            status: FileDescriptorStatus::Open,
            file_id,
        };

        // device file
        match &file_ref.ty {
            VfsFileType::DeviceFile(desc) => {
                (desc.open)()?;
            }
            _ => (),
        }

        self.fds.push(fd.clone());
        Ok(fd)
    }

    fn close_file(&mut self, fd_num: &FileDescriptorNumber) -> Result<()> {
        if let Some(index) = self.fds.iter().position(|f| f.num == *fd_num) {
            let file_id = self.fds[index].file_id;
            let file_ref = self
                .find_file(&file_id)
                .ok_or(VirtualFileSystemError::NoSuchFileOrDirectoryError(None))?;

            // device file
            match &file_ref.ty {
                VfsFileType::DeviceFile(desc) => {
                    (desc.close)()?;
                }
                _ => (),
            }

            self.fds.remove(index);
        } else {
            return Err(VirtualFileSystemError::ReleasedFileResourceError(*fd_num).into());
        }

        Ok(())
    }

    fn read_file(&self, fd_num: &FileDescriptorNumber) -> Result<Vec<u8>> {
        let fd = if let Some(fd) = self
            .fds
            .iter()
            .find(|f| f.num == *fd_num && f.status == FileDescriptorStatus::Open)
        {
            fd
        } else {
            return Err(VirtualFileSystemError::ReleasedFileResourceError(*fd_num).into());
        };

        let file_ref = self
            .find_file(&fd.file_id)
            .ok_or(VirtualFileSystemError::NoSuchFileOrDirectoryError(None))?;
        let file_path = self
            .abs_path_by_file(file_ref)
            .ok_or(VirtualFileSystemError::NoSuchFileOrDirectoryError(None))?;
        match &file_ref.ty {
            VfsFileType::VirtualFile => {
                if let Some(data) = &file_ref.data {
                    Ok(data.clone())
                } else if let Some((fs, fs_path)) = self.find_fs(file_ref) {
                    match fs {
                        FileSystem::Fat(fat) => {
                            let (_, bytes) = fat.get_file_by_abs_path(&file_path.diff(&fs_path))?;
                            Ok(bytes)
                        }
                    }
                } else {
                    Ok(Vec::new())
                }
            }
            VfsFileType::DeviceFile(desc) => (desc.read)(),
            _ => Err(VirtualFileSystemError::InvalidFileTypeError((
                file_ref.ty.clone(),
                Some(file_path),
            ))
            .into()),
        }
    }

    fn write_file(&mut self, fd_num: &FileDescriptorNumber, data: &[u8]) -> Result<()> {
        let fd = if let Some(fd) = self
            .fds
            .iter()
            .find(|f| f.num == *fd_num && f.status == FileDescriptorStatus::Open)
        {
            fd
        } else {
            return Err(VirtualFileSystemError::ReleasedFileResourceError(*fd_num).into());
        };

        let file_id = fd.file_id;
        let file_path;
        {
            let file_ref = self
                .find_file(&file_id)
                .ok_or(VirtualFileSystemError::NoSuchFileOrDirectoryError(None))?;
            file_path = self
                .abs_path_by_file(file_ref)
                .ok_or(VirtualFileSystemError::NoSuchFileOrDirectoryError(None))?;
        }

        let file_ref_mut = self
            .find_file_mut(&file_id)
            .ok_or(VirtualFileSystemError::NoSuchFileOrDirectoryError(None))?;

        match &mut file_ref_mut.ty {
            VfsFileType::VirtualFile => {
                file_ref_mut.data = Some(data.to_vec());

                // TODO
                warn!(
                    "VFS: Write to File system is unimplemented. Using temporary buffer: {}",
                    file_path
                );
            }
            VfsFileType::DeviceFile(desc) => (desc.write)(data)?,
            _ => {
                return Err(VirtualFileSystemError::InvalidFileTypeError((
                    file_ref_mut.ty.clone(),
                    Some(file_path),
                ))
                .into())
            }
        }

        Ok(())
    }

    fn file_size(&self, fd_num: &FileDescriptorNumber) -> Result<usize> {
        let fd = if let Some(fd) = self
            .fds
            .iter()
            .find(|f| f.num == *fd_num && f.status == FileDescriptorStatus::Open)
        {
            fd
        } else {
            return Err(VirtualFileSystemError::ReleasedFileResourceError(*fd_num).into());
        };

        let file_ref = self
            .find_file(&fd.file_id)
            .ok_or(VirtualFileSystemError::NoSuchFileOrDirectoryError(None))?;
        let file_path = self
            .abs_path_by_file(file_ref)
            .ok_or(VirtualFileSystemError::NoSuchFileOrDirectoryError(None))?;
        match &file_ref.ty {
            VfsFileType::VirtualFile => {
                if let Some(data) = &file_ref.data {
                    Ok(data.len())
                } else if let Some((fs, fs_path)) = self.find_fs(file_ref) {
                    match fs {
                        FileSystem::Fat(fat) => {
                            let (_, bytes) = fat.get_file_by_abs_path(&file_path.diff(&fs_path))?;
                            Ok(bytes.len())
                        }
                    }
                } else {
                    Ok(0)
                }
            }
            VfsFileType::DeviceFile(desc) => {
                let len = (desc.read)()?.len();
                Ok(len)
            }
            _ => Err(VirtualFileSystemError::InvalidFileTypeError((
                file_ref.ty.clone(),
                Some(file_path),
            ))
            .into()),
        }
    }
}

pub fn init() -> Result<()> {
    let mut vfs = unsafe { VFS.try_lock() }?;
    vfs.init()
}

pub fn chdir(path: &Path) -> Result<()> {
    let mut vfs = unsafe { VFS.try_lock() }?;
    vfs.chdir(path)
}

pub fn mount_fs(path: &Path, fs: FileSystem) -> Result<()> {
    let mut vfs = unsafe { VFS.try_lock() }?;
    vfs.mount_fs(path, fs)
}

pub fn entry_names(path: &Path) -> Result<Vec<String>> {
    let vfs = unsafe { VFS.try_lock() }?;
    let names = vfs
        .files_by_path(path)?
        .iter()
        .map(|f| f.name.clone())
        .collect();
    Ok(names)
}

pub fn cwd_path() -> Result<Path> {
    let vfs = unsafe { VFS.try_lock() }?;
    let cwd_id = vfs.cwd_id.ok_or(Error::NotInitialized)?;
    let file_ref = vfs.find_file(&cwd_id).ok_or(Error::NotInitialized)?;
    let path = vfs
        .abs_path_by_file(file_ref)
        .ok_or(Error::NotInitialized)?;

    Ok(path)
}

pub fn open_file(path: &Path, create: bool) -> Result<FileDescriptorNumber> {
    let mut vfs = unsafe { VFS.try_lock() }?;
    let fd = vfs.open_file(path, create)?;
    Ok(fd.num)
}

pub fn close_file(fd_num: &FileDescriptorNumber) -> Result<()> {
    let mut vfs = unsafe { VFS.try_lock() }?;
    vfs.close_file(fd_num)
}

pub fn read_file(fd_num: &FileDescriptorNumber) -> Result<Vec<u8>> {
    let vfs = unsafe { VFS.try_lock() }?;
    vfs.read_file(fd_num)
}

pub fn write_file(fd_num: &FileDescriptorNumber, data: &[u8]) -> Result<()> {
    let mut vfs = unsafe { VFS.try_lock() }?;
    vfs.write_file(fd_num, data)
}

pub fn file_size(fd_num: &FileDescriptorNumber) -> Result<usize> {
    let vfs = unsafe { VFS.try_lock() }?;
    vfs.file_size(fd_num)
}

pub fn create_file(path: &Path) -> Result<()> {
    let mut vfs = unsafe { VFS.try_lock() }?;
    vfs.add_file(path, VfsFileType::VirtualFile)
}

pub fn add_dev_file(desc: DeviceFileDescriptor, file_name: &str) -> Result<()> {
    let mut vfs = unsafe { VFS.try_lock() }?;
    vfs.add_dev_file(desc, file_name)
}
