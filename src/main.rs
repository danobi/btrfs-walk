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
mod tree;

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

    let mut root = vec![0; size as usize];
    file.read_exact_at(&mut root, physical)?;

    println!(
        "chunk tree root at logical offset={}, physical offset={}, size={}",
        chunk_root_logical, physical, size,
    );

    Ok(root)
}

fn read_root_tree_root(
    file: &File,
    root_tree_root_logical: u64,
    cache: &ChunkTreeCache,
) -> Result<Vec<u8>> {
    let size = cache
        .mapping_kv(root_tree_root_logical)
        .ok_or_else(|| anyhow!("Root tree root logical addr not mapped"))?
        .0
        .size;
    let physical = cache
        .offset(root_tree_root_logical)
        .ok_or_else(|| anyhow!("Root tree root logical addr not mapped"))?;

    let mut root = vec![0; size as usize];
    file.read_exact_at(&mut root, physical)?;

    println!(
        "root tree root at logical offset={}, physical offset={}, size={}",
        root_tree_root_logical, physical, size,
    );

    Ok(root)
}

fn read_chunk_tree(
    file: &File,
    root: &[u8],
    chunk_tree_cache: &mut ChunkTreeCache,
    superblock: &BtrfsSuperblock,
) -> Result<()> {
    let header = tree::parse_btrfs_header(root).expect("failed to parse chunk root header");
    unsafe {
        println!(
            "chunk tree node level={}, bytenr={}, nritems={}",
            header.level, header.bytenr, header.nritems
        );
    }

    // Level 0 is leaf node, !0 is internal node
    if header.level == 0 {
        let items = tree::parse_btrfs_leaf(root)?;
        for item in items {
            if item.key.ty != BTRFS_CHUNK_ITEM_KEY {
                continue;
            }

            let chunk = unsafe {
                // `item.offset` is offset from data portion of `BtrfsLeaf` where associated
                // `BtrfsChunk` starts
                &*(root
                    .as_ptr()
                    .add(std::mem::size_of::<BtrfsHeader>() + item.offset as usize)
                    as *const BtrfsChunk)
            };

            chunk_tree_cache.insert(
                ChunkTreeKey {
                    start: item.key.offset,
                    size: chunk.length,
                },
                ChunkTreeValue {
                    offset: chunk.stripe.offset,
                },
            );
        }
    } else {
        let ptrs = tree::parse_btrfs_node(root)?;
        for ptr in ptrs {
            let physical = chunk_tree_cache
                .offset(ptr.blockptr)
                .ok_or_else(|| anyhow!("Chunk tree node not mapped"))?;
            let mut node = vec![0; superblock.node_size as usize];
            file.read_exact_at(&mut node, physical)?;
            read_chunk_tree(file, &node, chunk_tree_cache, superblock)?;
        }
    }

    Ok(())
}

fn read_fs_tree_root(
    file: &File,
    superblock: &BtrfsSuperblock,
    root_tree_root: &[u8],
    cache: &ChunkTreeCache,
) -> Result<Vec<u8>> {
    let header =
        tree::parse_btrfs_header(root_tree_root).expect("failed to parse root tree root header");
    unsafe {
        println!(
            "root tree root level={}, bytenr={}, nritems={}",
            header.level, header.bytenr, header.nritems
        );
    }

    if header.level == 0 {
        let items = tree::parse_btrfs_leaf(root_tree_root)?;
        for item in items.iter().rev() {
            if item.key.objectid != BTRFS_FS_TREE_OBJECTID || item.key.ty != BTRFS_ROOT_ITEM_KEY {
                continue;
            }

            let root_item = unsafe {
                &*(root_tree_root
                    .as_ptr()
                    .add(std::mem::size_of::<BtrfsHeader>() + item.offset as usize)
                    as *const BtrfsRootItem)
            };

            let physical = cache
                .offset(root_item.bytenr)
                .ok_or_else(|| anyhow!("fs tree root not mapped"))?;
            let mut node = vec![0; superblock.node_size as usize];
            file.read_exact_at(&mut node, physical)?;

            unsafe {
                println!(
                    "fs tree root at logical offset={}, physical offset={}, size={}",
                    root_item.bytenr, physical, superblock.node_size,
                );
            }

            return Ok(node);
        }

        bail!("Failed to find root tree item for fs tree root");
    } else {
        // I'm not sure if the root tree root can be an internal node. Either way, let's
        // see if we can ignore this and still make things work.
        bail!("Root tree root is not a leaf node");
    }
}

/// Returns `BtrfsInodeRef` associated with `inode` number. Also returns `BtrfsKey` associated
/// with the `BtrfsInodeRef`.
fn get_inode_ref(
    inode: u64,
    file: &File,
    superblock: &BtrfsSuperblock,
    fs_root_node: &[u8],
    cache: &ChunkTreeCache,
) -> Result<Option<(BtrfsKey, BtrfsInodeRef)>> {
    unimplemented!();
}

fn walk_fs_tree(
    file: &File,
    superblock: &BtrfsSuperblock,
    node: &[u8],
    root_fs_node: &[u8],
    cache: &ChunkTreeCache,
) -> Result<()> {
    let header = tree::parse_btrfs_header(node)?;
    unsafe {
        println!(
            "fs tree node level={}, bytenr={}, nritems={}",
            header.level, header.bytenr, header.nritems
        );
    }

    // Leaf node
    if header.level == 0 {
        let items = tree::parse_btrfs_leaf(node)?;
        for item in items {
            if item.key.ty != BTRFS_DIR_ITEM_KEY.into() {
                continue;
            }

            let dir_item = unsafe {
                &*(node
                    .as_ptr()
                    .add(std::mem::size_of::<BtrfsHeader>() + item.offset as usize)
                    as *const BtrfsDirItem)
            };

            if dir_item.ty != BTRFS_FT_REG_FILE {
                continue;
            }

            let name_slice = unsafe {
                std::slice::from_raw_parts(
                    (dir_item as *const BtrfsDirItem as *const u8)
                        .add(std::mem::size_of::<BtrfsDirItem>()),
                    dir_item.name_len.into(),
                )
            };
            let name = std::str::from_utf8(name_slice)?;

            // Now compute the path leading up to current file
            //
            // Capacity 1 so we don't panic the first `String::insert`
            let mut path_prefix = String::with_capacity(1);
            // `item.key.objectid` is parent inode number
            let mut current_inode_nr = item.key.objectid;
            loop {
                let (current_key, current_inode) =
                    get_inode_ref(current_inode_nr, file, superblock, root_fs_node, cache)?
                        .ok_or_else(|| {
                            anyhow!("Failed to find inode_ref for inode={}", current_inode_nr)
                        })?;
                assert_eq!(current_key.objectid, current_inode_nr);
                // `current_key.offset` is parent inode # of `current_inode`
                if current_key.offset == current_inode_nr {
                    // If parent inode # and current inode # match, that means ".." points to
                    // itself and we've hit the filesystem root.
                    path_prefix.insert(0, '/');
                    break;
                }

                let s = unsafe {
                    std::slice::from_raw_parts(
                        (&current_inode as *const BtrfsInodeRef as *const u8)
                            .add(std::mem::size_of::<BtrfsInodeRef>()),
                        current_inode.name_len.into(),
                    )
                };
                let n = std::str::from_utf8(s)?;
                path_prefix.insert_str(0, &format!("{}/", n));

                current_inode_nr = current_key.offset;
            }

            println!("filename={}{}", path_prefix, name);
        }
    } else {
        let ptrs = tree::parse_btrfs_node(node)?;
        for ptr in ptrs {
            let physical = cache
                .offset(ptr.blockptr)
                .ok_or_else(|| anyhow!("fs tree node not mapped"))?;
            let mut node = vec![0; superblock.node_size as usize];
            file.read_exact_at(&mut node, physical)?;
            walk_fs_tree(file, superblock, &node, root_fs_node, cache)?;
        }
    }

    Ok(())
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
    let mut chunk_tree_cache =
        bootstrap_chunk_tree(&superblock).expect("failed to bootstrap chunk tree");

    // Read root chunk tree node
    let chunk_root = read_chunk_tree_root(&file, superblock.chunk_root, &chunk_tree_cache)
        .expect("failed to read chunk tree root");

    // Read rest of chunk tree
    read_chunk_tree(&file, &chunk_root, &mut chunk_tree_cache, &superblock)
        .expect("failed to read chunk tree");

    // Read root tree root node
    let root_tree_root = read_root_tree_root(&file, superblock.root, &chunk_tree_cache)
        .expect("failed to read root tree root");

    // Read filesystem tree root node
    let fs_tree_root = read_fs_tree_root(&file, &superblock, &root_tree_root, &chunk_tree_cache)
        .expect("failed to read fs tree root");

    // Now start walking fs tree
    walk_fs_tree(
        &file,
        &superblock,
        &fs_tree_root,
        &fs_tree_root,
        &chunk_tree_cache,
    )
    .expect("failed to walk fs tree");
}
