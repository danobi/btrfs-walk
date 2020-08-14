const BTRFS_CSUM_SIZE: usize = 32;
const BTRFS_LABEL_SIZE: usize = 256;
const BTRFS_FSID_SIZE: usize = 16;
const BTRFS_UUID_SIZE: usize = 16;
const BTRFS_SYSTEM_CHUNK_ARRAY_SIZE: usize = 2048;

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct BtrfsDevItem {
    /// the internal btrfs device id
    pub devid: u64,
    /// size of the device
    pub total_bytes: u64,
    /// bytes used
    pub bytes_used: u64,
    /// optimal io alignment for this device
    pub io_align: u32,
    /// optimal io width for this device
    pub io_width: u32,
    /// minimal io size for this device
    pub sector_size: u32,
    /// type and info about this device
    pub ty: u64,
    /// expected generation for this device
    pub generation: u64,
    /// starting byte of this partition on the device, to allow for stripe alignment in the future
    pub start_offset: u64,
    /// grouping information for allocation decisions
    pub dev_group: u32,
    /// seek speed 0-100 where 100 is fastest
    pub seek_speed: u8,
    /// bandwidth 0-100 where 100 is fastest
    pub bandwidth: u8,
    /// btrfs generated uuid for this device
    pub uuid: [u8; BTRFS_UUID_SIZE],
    /// uuid of FS who owns this device
    pub fsid: [u8; BTRFS_UUID_SIZE],
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct BtrfsRootBackup {
    pub tree_root: u64,
    pub tree_root_gen: u64,
    pub chunk_root: u64,
    pub chunk_root_gen: u64,
    pub extent_root: u64,
    pub extent_root_gen: u64,
    pub fs_root: u64,
    pub fs_root_gen: u64,
    pub dev_root: u64,
    pub dev_root_gen: u64,
    pub csum_root: u64,
    pub csum_root_gen: u64,
    pub total_bytes: u64,
    pub bytes_used: u64,
    pub num_devices: u64,
    /// future
    pub unused_64: [u64; 4],
    pub tree_root_level: u8,
    pub chunk_root_level: u8,
    pub extent_root_level: u8,
    pub fs_root_level: u8,
    pub dev_root_level: u8,
    pub csum_root_level: u8,
    /// future and to align
    pub unused_8: [u8; 10],
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct BtrfsSuperblock {
    pub csum: [u8; BTRFS_CSUM_SIZE],
    pub fsid: [u8; BTRFS_FSID_SIZE],
    /// Physical address of this block
    pub bytenr: u64,
    pub flags: u64,
    pub magic: [u8; 0x8],
    pub generation: u64,
    /// Logical address of the root tree root
    pub root: u64,
    /// Logical address of the chunk tree root
    pub chunk_root: u64,
    /// Logical address of the log tree root
    pub log_root: u64,
    pub log_root_transid: u64,
    pub total_bytes: u64,
    pub bytes_used: u64,
    pub root_dir_objectid: u64,
    pub num_devices: u64,
    pub sector_size: u32,
    pub node_size: u32,
    /// Unused and must be equal to `nodesize`
    pub leafsize: u32,
    pub stripesize: u32,
    pub sys_chunk_array_size: u32,
    pub chunk_root_generation: u64,
    pub compat_flags: u64,
    pub compat_ro_flags: u64,
    pub incompat_flags: u64,
    pub csum_type: u16,
    pub root_level: u8,
    pub chunk_root_level: u8,
    pub log_root_level: u8,
    pub dev_item: BtrfsDevItem,
    pub label: [u8; BTRFS_LABEL_SIZE],
    pub cache_generation: u64,
    pub uuid_tree_generation: u64,
    pub metadata_uuid: [u8; BTRFS_FSID_SIZE],
    /// Future expansion
    pub _reserved: [u64; 28],
    pub sys_chunk_array: [u8; BTRFS_SYSTEM_CHUNK_ARRAY_SIZE],
    pub root_backups: [BtrfsRootBackup; 4],
}