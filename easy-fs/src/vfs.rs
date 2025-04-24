use super::{
    block_cache_sync_all, get_block_cache, BlockDevice, DirEntry, DiskInode, DiskInodeType,
    EasyFileSystem, DIRENT_SZ,
};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::{Mutex, MutexGuard};

/// The max length of inode name
const NAME_LENGTH_LIMIT: usize = 27;
/// Virtual filesystem layer over easy-fs
pub struct Inode {
    block_id: usize,
    block_offset: usize,
    fs: Arc<Mutex<EasyFileSystem>>,
    block_device: Arc<dyn BlockDevice>,
}

impl Inode {
    /// Create a vfs inode
    pub fn new(
        block_id: u32,
        block_offset: usize,
        fs: Arc<Mutex<EasyFileSystem>>,
        block_device: Arc<dyn BlockDevice>,
    ) -> Self {
        Self {
            block_id: block_id as usize,
            block_offset,
            fs,
            block_device,
        }
    }
    /// Call a function over a disk inode to read it
    pub fn read_disk_inode<V>(&self, f: impl FnOnce(&DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .read(self.block_offset, f)
    }
    /// Call a function over a disk inode to modify it
    fn modify_disk_inode<V>(&self, f: impl FnOnce(&mut DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .modify(self.block_offset, f)
    }
    /// Find inode under a disk inode by name
    fn find_inode_id(&self, name: &str, disk_inode: &DiskInode) -> Option<u32> {
        // assert it is a directory
        assert!(disk_inode.is_dir());
        let file_count = (disk_inode.size as usize) / DIRENT_SZ;
        let mut dirent = DirEntry::empty();
        for i in 0..file_count {
            assert_eq!(
                disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device,),
                DIRENT_SZ,
            );
            if dirent.name() == name {
                return Some(dirent.inode_id() as u32);
            }
        }
        None
    }
    /// Find inode under current inode by name
    pub fn find(&self, name: &str) -> Option<Arc<Inode>> {
        let fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            self.find_inode_id(name, disk_inode).map(|inode_id| {
                let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
                Arc::new(Self::new(
                    block_id,
                    block_offset,
                    self.fs.clone(),
                    self.block_device.clone(),
                ))
            })
        })
    }
    /// Increase the size of a disk inode
    fn increase_size(
        &self,
        new_size: u32,
        disk_inode: &mut DiskInode,
        fs: &mut MutexGuard<EasyFileSystem>,
    ) {
        if new_size < disk_inode.size {
            return;
        }
        let blocks_needed = disk_inode.blocks_num_needed(new_size);
        let mut v: Vec<u32> = Vec::new();
        for _ in 0..blocks_needed {
            v.push(fs.alloc_data());
        }
        disk_inode.increase_size(new_size, v, &self.block_device);
    }
    /// Create inode under current inode by name
    pub fn create(&self, name: &str) -> Option<Arc<Inode>> {
        let mut fs = self.fs.lock();
        let op = |root_inode: &DiskInode| {
            // assert it is a directory
            assert!(root_inode.is_dir());
            // has the file been created?
            self.find_inode_id(name, root_inode)
        };
        if self.read_disk_inode(op).is_some() {
            return None;
        }
        // create a new file
        // alloc a inode with an indirect block
        let new_inode_id = fs.alloc_inode();
        // initialize inode
        let (new_inode_block_id, new_inode_block_offset) = fs.get_disk_inode_pos(new_inode_id);
        get_block_cache(new_inode_block_id as usize, Arc::clone(&self.block_device))
            .lock()
            .modify(new_inode_block_offset, |new_inode: &mut DiskInode| {
                new_inode.initialize(DiskInodeType::File);
            });
        self.modify_disk_inode(|root_inode| {
            // append file in the dirent
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            // increase size
            self.increase_size(new_size as u32, root_inode, &mut fs);
            // write dirent
            let dirent = DirEntry::new(name, new_inode_id);
            root_inode.write_at(
                file_count * DIRENT_SZ,
                dirent.as_bytes(),
                &self.block_device,
            );
        });

        let (block_id, block_offset) = fs.get_disk_inode_pos(new_inode_id);
        block_cache_sync_all();
        // return inode
        Some(Arc::new(Self::new(
            block_id,
            block_offset,
            self.fs.clone(),
            self.block_device.clone(),
        )))
        // release efs lock automatically by compiler
    }
    /// List inodes under current inode
    pub fn ls(&self) -> Vec<String> {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            let file_count = (disk_inode.size as usize) / DIRENT_SZ;
            let mut v: Vec<String> = Vec::new();
            for i in 0..file_count {
                let mut dirent = DirEntry::empty();
                assert_eq!(
                    disk_inode.read_at(i * DIRENT_SZ, dirent.as_bytes_mut(), &self.block_device,),
                    DIRENT_SZ,
                );
                v.push(String::from(dirent.name()));
            }
            v
        })
    }
    /// Read data from current inode
    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| disk_inode.read_at(offset, buf, &self.block_device))
    }
    /// Write data to current inode
    pub fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        let mut fs = self.fs.lock();
        let size = self.modify_disk_inode(|disk_inode| {
            self.increase_size((offset + buf.len()) as u32, disk_inode, &mut fs);
            disk_inode.write_at(offset, buf, &self.block_device)
        });
        block_cache_sync_all();
        size
    }
    /// Clear the data in current inode
    pub fn clear(&self) {
        let mut fs = self.fs.lock();
        self.modify_disk_inode(|disk_inode| {
            let size = disk_inode.size;
            let data_blocks_dealloc = disk_inode.clear_size(&self.block_device);
            assert!(data_blocks_dealloc.len() == DiskInode::total_blocks(size) as usize);
            for data_block in data_blocks_dealloc.into_iter() {
                fs.dealloc_data(data_block);
            }
        });
        block_cache_sync_all();
    }
    /// get_link_count in inode
    pub fn get_link_count(&self) -> u32 {
        self.read_disk_inode(|disk_inode| disk_inode.get_link_count())
    }
    /// modify_link_count in inode
    pub fn modify_link_count(&self, add_minus: bool) {
        match add_minus {
            true => self.modify_disk_inode(|disk_inode| {
                disk_inode.add_link_count();
            }),
            false => self.modify_disk_inode(|disk_inode| {
                disk_inode.minus_link_count();
            }),
        }
        if self.get_link_count() == 0 {
            self.clear();
        }
    }
    /// create link only used by root-inode
    pub fn link(&self, old_name: &str, new_name: &str) -> bool {
        let mut fs = self.fs.lock();

        let old_inode_id = self.read_disk_inode(|root_inode| {
            assert!(root_inode.is_dir());
            self.find_inode_id(old_name, root_inode)
        });

        let old_inode_id = match old_inode_id {
            Some(id) => id,
            None => return false,
        };

        let new_inode_id = self.read_disk_inode(|root_inode| {
            assert!(root_inode.is_dir());
            self.find_inode_id(new_name, root_inode)
        });
        if new_inode_id.is_some() {
            return false;
        }

        self.modify_disk_inode(|root_inode| {
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            // increase size
            self.increase_size(new_size as u32, root_inode, &mut fs);
            // write dirent
            let dirent = DirEntry::new(new_name, old_inode_id);
            root_inode.write_at(
                file_count * DIRENT_SZ,
                dirent.as_bytes(),
                &self.block_device,
            );
        });

        let (block_id, block_offset) = fs.get_disk_inode_pos(old_inode_id);
        get_block_cache(block_id as usize, Arc::clone(&self.block_device))
            .lock()
            .modify(block_offset, |inode: &mut DiskInode| {
                inode.add_link_count();
            });

        true
    }
    ///
    pub fn unlink(&self, name: &str) -> bool {
        let mut fs = self.fs.lock();

        let inode_id = self.read_disk_inode(|root_inode| {
            assert!(root_inode.is_dir());
            self.find_inode_id(name, root_inode)
        });
        if inode_id == None {
            return false;
        }

        self.modify_disk_inode(|root_inode| {
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let mut offset = None;
            for i in 0..file_count {
                let mut dirent_data = [0u8; DIRENT_SZ];
                root_inode.read_at(i * DIRENT_SZ, &mut dirent_data, &self.block_device);

                let mut now_name = [0u8; NAME_LENGTH_LIMIT + 1];
                for j in 0..(NAME_LENGTH_LIMIT + 1) {
                    now_name[j] = dirent_data[j];
                }

                let inode_number = (dirent_data[NAME_LENGTH_LIMIT + 1] as u32)
                    | ((dirent_data[NAME_LENGTH_LIMIT + 2] as u32) << 8)
                    | ((dirent_data[NAME_LENGTH_LIMIT + 3] as u32) << 16)
                    | ((dirent_data[NAME_LENGTH_LIMIT + 4] as u32) << 24);

                let mut name_str_len = 0;
                while name_str_len < name.len() && now_name[name_str_len] != 0 {
                    name_str_len += 1;
                }
                let dir_name = core::str::from_utf8(&now_name[..name_str_len]).unwrap();

                if dir_name == name {
                    offset = Some(i);
                    break;
                }
            }

            if let Some(i) = offset {
                if i != file_count - 1 {
                    let mut last_entry = [0u8; DIRENT_SZ];
                    root_inode.read_at(
                        (file_count - 1) * DIRENT_SZ,
                        &mut last_entry,
                        &self.block_device,
                    );
                    root_inode.write_at(i * DIRENT_SZ, &last_entry, &self.block_device);
                }
                root_inode.size -= DIRENT_SZ as u32;
            }
        });

        let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id.unwrap());
        let inode = Self::new(
            block_id,
            block_offset,
            self.fs.clone(),
            self.block_device.clone(),
        );

        let mut link_zero = false;
        inode.modify_disk_inode(|disk_inode| {
            assert!(disk_inode.is_file());
            if disk_inode.link_count > 0 {
                disk_inode.link_count -= 1;
            }
            if disk_inode.link_count == 0 {
                link_zero = true;
            }
        });

        if link_zero {
            inode.modify_disk_inode(|disk_inode| {
                let size = disk_inode.size;
                let data_blocks_dealloc = disk_inode.clear_size(&self.block_device);
                assert!(data_blocks_dealloc.len() == DiskInode::total_blocks(size) as usize);
                for data_block in data_blocks_dealloc.into_iter() {
                    fs.dealloc_data(data_block);
                }
            });

            get_block_cache(block_id as usize, Arc::clone(&self.block_device))
                .lock()
                .modify(block_offset, |inode: &mut DiskInode| {
                    inode.size = 0;
                    inode.link_count = 0;
                    inode.direct = [0; 27];
                    inode.indirect1 = 0;
                    inode.indirect2 = 0;
                });
            fs.inode_bitmap.dealloc(&self.block_device, inode_id.unwrap() as usize);
        }

        block_cache_sync_all();
        true
    }
}
