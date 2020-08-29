# btrfs-walk

Walks on-disk btrfs data structures. Intended to be an educational exercise for
learning about btrfs.

## Warning

I've totally ignored endianness on purpose. btrfs uses little-endian on-disk
and fortunately x86 is little-endian. So I can save some effort and ignore
endianness.

Other things I've ignored:

* Striping
* Checksums
* Really any kind of validity checks
