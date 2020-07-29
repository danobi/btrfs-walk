mod structs;
use structs::*;

fn main() {
    println!(
        "sizeof(BtrfsDevItem)={}",
        std::mem::size_of::<BtrfsDevItem>()
    );
    println!(
        "sizeof(BtrfsRootBackup)={}",
        std::mem::size_of::<BtrfsRootBackup>()
    );
    println!(
        "sizeof(BtrfsSuperblock)={}",
        std::mem::size_of::<BtrfsSuperblock>()
    );
}
