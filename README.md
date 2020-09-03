# btrfs-walk

Prints the absolute path of all regular files in an unmounted btrfs filesystem
image.

`btrfs-walk` walks on-disk btrfs data structures without external btrfs
libraries or `ioctl(2)` calls. Intended to be an educational exercise for
learning about btrfs's on-disk data format.

## Example output

```bash
$ sudo ./target/debug/btrfs-walk ~/scratch/btrfsimg
warning: 2 stripes detected but only processing 1
chunk tree root at logical offset=22036480, physical offset=22036480, size=8388608
chunk tree node level=0, bytenr=22036480, nritems=4
root tree root at logical offset=30867456, physical offset=39256064, size=268435456
root tree root level=0, bytenr=30867456, nritems=13
fs tree root at logical offset=30834688, physical offset=39223296, size=16384
fs tree node level=0, bytenr=30834688, nritems=53
filename=/medir/mefile
filename=/medir/mefile2
filename=/medir/mefile4
filename=/medir/mefile5
filename=/medir/mefile3
filename=/medir/medir2/medir3/mefile6

$ sudo mount ~/scratch/btrfsimg /mnt/btrfs

$ tree /mnt/btrfs
/mnt/btrfs
└── medir
    ├── medir2
    │   └── medir3
    │       └── mefile6
    ├── mefile
    ├── mefile2
    ├── mefile3
    ├── mefile4
    └── mefile5

3 directories, 6 files
```

## Warning

I've totally ignored endianness on purpose. btrfs uses little-endian on-disk
and fortunately x86 is little-endian. So I can save some effort and ignore
endianness.

Other things I've ignored:

* Striping
* Checksums
* Really any kind of validity checks
