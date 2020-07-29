use std::fs::OpenOptions;
use std::os::unix::prelude::FileExt;
use std::path::PathBuf;
use std::slice;

use structopt::StructOpt;

mod structs;
use structs::*;

/// Physical address of the first superblock
const BTRFS_SUPERBLOCK_OFFSET: u64 = 0x10_000;
const BTRFS_SUPERBLOCK_MAGIC: [u8; 0x8] = *b"_BHRfS_M";

#[derive(Debug, StructOpt)]
#[structopt(name = "btrfs-walk", about = "Walk an on-disk btrfs filesystem")]
struct Opt {
    /// Block device or file to process
    #[structopt(parse(from_os_str))]
    device: PathBuf,
}

fn main() {
    let opt = Opt::from_args();

    let file = OpenOptions::new()
        .read(true)
        .open(opt.device.as_path())
        .expect("Failed to open path");

    // Read superblock
    let mut superblock: BtrfsSuperblock = unsafe { std::mem::zeroed() };
    let superblock_size = std::mem::size_of::<BtrfsSuperblock>();
    let slice;
    unsafe {
        slice = slice::from_raw_parts_mut(&mut superblock as *mut _ as *mut u8, superblock_size);
    }
    file.read_exact_at(slice, BTRFS_SUPERBLOCK_OFFSET)
        .expect("failed to read superblock");
    if superblock.magic != BTRFS_SUPERBLOCK_MAGIC {
        panic!("superblock magic is wrong");
    }
}
