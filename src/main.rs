use std::fs::{File, OpenOptions};
use std::os::unix::prelude::FileExt;
use std::path::PathBuf;
use std::slice;

use anyhow::{anyhow, bail, Result};
use structopt::StructOpt;

mod structs;
use structs::*;
mod chunk_tree;
use chunk_tree::{ChunkTreeCache, ChunkTreeKey, ChunkTreeValue};

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

fn parse_superblock(file: &File) -> Result<BtrfsSuperblock> {
    let mut superblock: BtrfsSuperblock = unsafe { std::mem::zeroed() };
    let superblock_size = std::mem::size_of::<BtrfsSuperblock>();
    let slice;
    unsafe {
        slice = slice::from_raw_parts_mut(&mut superblock as *mut _ as *mut u8, superblock_size);
    }
    file.read_exact_at(slice, BTRFS_SUPERBLOCK_OFFSET)?;
    if superblock.magic != BTRFS_SUPERBLOCK_MAGIC {
        bail!("superblock magic is wrong");
    }

    Ok(superblock)
}

fn bootstrap_chunk_tree(superblock: &BtrfsSuperblock) -> Result<ChunkTreeCache> {
    let array_size = superblock.sys_chunk_array_size as usize;
    let mut offset: usize = 0;
    let mut chunk_tree_cache = ChunkTreeCache::default();

    while offset < array_size {
        let key_size = std::mem::size_of::<BtrfsKey>();
        if offset + key_size > array_size as usize {
            bail!("short key read");
        }

        let key_slice = &superblock.sys_chunk_array[offset..];
        let key = unsafe { &*(key_slice.as_ptr() as *const BtrfsKey) };
        if key.ty != BTRFS_CHUNK_ITEM_KEY {
            bail!(
                "unknown item type={} in sys_array at offset={}",
                key.ty,
                offset
            );
        }
        offset += key_size;

        if offset + std::mem::size_of::<BtrfsChunk>() > array_size {
            bail!("short chunk item read");
        }

        let chunk_slice = &superblock.sys_chunk_array[offset..];
        let chunk = unsafe { &*(chunk_slice.as_ptr() as *const BtrfsChunk) };
        if chunk.num_stripes == 0 {
            bail!("num_stripes cannot be 0");
        }

        // To keep things simple, we'll only process 1 stripe, as stripes should have
        // identical content. The device the stripe is on will be the device passed in
        // via cmd line args.
        let num_stripes = chunk.num_stripes; // copy to prevent unaligned access
        if num_stripes != 1 {
            println!(
                "warning: {} stripes detected but only processing 1",
                num_stripes
            );
        }

        // Add chunk to cache if not already in cache
        let logical = key.offset;
        if chunk_tree_cache.offset(logical).is_none() {
            chunk_tree_cache.insert(
                ChunkTreeKey {
                    start: logical,
                    size: chunk.length,
                },
                ChunkTreeValue {
                    offset: chunk.stripe.offset,
                },
            );
        }

        // Despite only processing one stripe, we need to be careful to skip over the
        // entire chunk item.
        let chunk_item_size = std::mem::size_of::<BtrfsChunk>()
            + (std::mem::size_of::<BtrfsStripe>() * (chunk.num_stripes as usize - 1));
        if offset + chunk_item_size > array_size {
            bail!("short chunk item + stripe read");
        }
        offset += chunk_item_size;
    }

    Ok(chunk_tree_cache)
}

fn read_chunk_tree_root(
    file: &File,
    chunk_root_logical: u64,
    cache: &ChunkTreeCache,
) -> Result<Vec<u8>> {
    let size = cache
        .mapping_kv(chunk_root_logical)
        .ok_or_else(|| anyhow!("Chunk tree root not bootstrapped"))?
        .0
        .size;
    let physical = cache
        .offset(chunk_root_logical)
        .ok_or_else(|| anyhow!("Chunk tree root not bootstrapped"))?;

    let mut root = Vec::with_capacity(size as usize);
    // with_capacity() does not affect len() but resize() does
    root.resize(size as usize, 0);
    file.read_exact_at(&mut root, physical)?;

    Ok(root)
}

fn main() {
    let opt = Opt::from_args();

    let file = OpenOptions::new()
        .read(true)
        .open(opt.device.as_path())
        .expect("Failed to open path");

    // Read superblock
    let superblock = parse_superblock(&file).expect("failed to parse superblock");

    // Bootstrap chunk tree
    let chunk_tree_cache =
        bootstrap_chunk_tree(&superblock).expect("failed to bootstrap chunk tree");
    println!(
        "chunk tree root at physical offset={}, size={}",
        chunk_tree_cache
            .offset(superblock.chunk_root)
            .expect("chunk tree root not in bootstraped chunk tree"),
        chunk_tree_cache
            .mapping_kv(superblock.chunk_root)
            .expect("chunk tree root not in bootstrapped chunk tree")
            .0
            .size
    );

    let _chunk_root = read_chunk_tree_root(&file, superblock.chunk_root, &chunk_tree_cache)
        .expect("failed to read chunk tree root");
}
