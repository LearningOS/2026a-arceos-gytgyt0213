use crate::VM_ENTRY;
use axhal::mem::{MemoryAddr, VirtAddr};
use axhal::paging::MappingFlags;
use axmm::AddrSpace;
use std::fs::File;
use std::io::{self, Read};

pub fn load_vm_image(fname: &str, uspace: &mut AddrSpace) -> io::Result<()> {
    ax_println!("app: {}", fname);
    let mut file = File::open(fname)?;
    let mut buf = alloc::vec::Vec::new();
    file.read_to_end(&mut buf)?;
    if buf.is_empty() {
        return Err(io::Error::InvalidInput);
    }
    let size = VirtAddr::from(buf.len()).align_up_4k().as_usize();
    uspace.map_alloc(
        VM_ENTRY.into(),
        size,
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE | MappingFlags::USER,
        true,
    )?;
    uspace.write(VM_ENTRY.into(), &buf)
}
