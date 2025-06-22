use std::fmt;
use std::fs::File;
use std::io::Read;
use object::{Object, ObjectSegment};
use std::fs;

const MEM_BASE: u64 = 0x8000_0000; 
const MEM_SIZE: usize = 0x80_00000; 

// MemoryError
#[derive(Debug)]
pub enum MemoryError {
    InvalidAddress { addr: u64 },
    InvalidReadLength { len: usize },
    InvalidWriteLength { len: usize },
    ZeroPc,
    FileError(std::io::Error),
    EmptyFilePath,
    ImageLoadFailed,
}

// implement Display for MemoryError
impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MemoryError::InvalidAddress { addr } => 
                write!(f, "Invalid address: 0x{:x}", addr),
            MemoryError::InvalidReadLength { len } => 
                write!(f, "Invalid read length: {}", len),
            MemoryError::InvalidWriteLength { len } => 
                write!(f, "Invalid write length: {}", len),
            MemoryError::ZeroPc => 
                write!(f, "PC is zero"),
            MemoryError::FileError(e) => 
                write!(f, "File operation error: {}", e),
            MemoryError::EmptyFilePath => 
                write!(f, "Image file path is empty"),
            MemoryError::ImageLoadFailed => 
                write!(f, "Image load failed"),
        }
    }
}

// implement std::error::Error for MemoryError
impl std::error::Error for MemoryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MemoryError::FileError(e) => Some(e),
            _ => None,
        }
    }
}

// implement From<std::io::Error> for MemoryError
impl From<std::io::Error> for MemoryError {
    fn from(err: std::io::Error) -> Self {
        MemoryError::FileError(err)
    }
}


//////////////
/// Memory ///
//////////////
pub struct Memory {
    mem: Box<[u8; MEM_SIZE]>, // 使用 Box 避免栈溢出
}

impl Memory {
    pub fn new() -> Self {
        let mem = vec![0u8; MEM_SIZE].into_boxed_slice();
        let mem = mem.try_into().expect("Failed to create memory array");
        
        Self { mem }
    }

    /// translate guest address to host address
    pub fn guest_to_host(&self, addr: u64) -> Result<*const u8, MemoryError> {
        if addr < MEM_BASE || addr >= MEM_BASE + MEM_SIZE as u64 {
            return Err(MemoryError::InvalidAddress { addr });
        }
        
        let offset = (addr - MEM_BASE) as usize;
        Ok(unsafe { self.mem.as_ptr().add(offset) })
    }

    /// translate guest address to host mutable address
    pub fn guest_to_host_mut(&mut self, addr: u64) -> Result<*mut u8, MemoryError> {
        if addr < MEM_BASE || addr >= MEM_BASE + MEM_SIZE as u64 {
            return Err(MemoryError::InvalidAddress { addr });
        }
        
        let offset = (addr - MEM_BASE) as usize;
        Ok(unsafe { self.mem.as_mut_ptr().add(offset) })
    }

    /// read data from host address
    fn host_read<T: Copy>(addr: *const u8) -> T {
        unsafe { (addr as *const T).read_unaligned() }
    }

    /// write data to host address
    fn host_write<T>(addr: *mut u8, value: T) {
        unsafe { (addr as *mut T).write_unaligned(value) };
    }

    /// read data from memory
    pub fn mem_read(&self, addr: u64, len: usize) -> Result<u64, MemoryError> {
        let host_addr = self.guest_to_host(addr)? as *const u8;
        
        match len {
            1 => Ok(Self::host_read::<u8>(host_addr) as u64),
            2 => Ok(Self::host_read::<u16>(host_addr) as u64),
            4 => Ok(Self::host_read::<u32>(host_addr) as u64),
            8 => Ok(Self::host_read::<u64>(host_addr)),
            _ => Err(MemoryError::InvalidReadLength { len }),
        }
    }

    /// write data to memory
    pub fn mem_write(&mut self, addr: u64, len: usize, data: u64) -> Result<(), MemoryError> {
        let host_addr = self.guest_to_host_mut(addr)? as *mut u8;
        
        match len {
            1 => {
                Self::host_write(host_addr, data as u8);
                Ok(())
            }
            2 => {
                Self::host_write(host_addr, data as u16);
                Ok(())
            }
            4 => {
                Self::host_write(host_addr, data as u32);
                Ok(())
            }
            8 => {
                Self::host_write(host_addr, data);
                Ok(())
            }
            _ => Err(MemoryError::InvalidWriteLength { len }),
        }
    }

    /// fetch instruction from memory (4 bytes)
    pub fn inst_fetch(&self, pc: u64) -> Result<u32, MemoryError> {
        if pc == 0 {
            return Err(MemoryError::ZeroPc);
        }
        
        let host_addr = self.guest_to_host(pc)?;
        Ok(Self::host_read::<u32>(host_addr))
    }

    /// load image file to memory (bin or elf)
    pub fn load_image(&mut self, filepath: &str) -> Result<(), MemoryError> {
        println!("Physical Memory Range: [0x{:016x}, 0x{:016x}]", 
                 MEM_BASE, MEM_BASE + MEM_SIZE as u64 - 1);
        
        if filepath.is_empty() {
            return Err(MemoryError::EmptyFilePath);
        }
        
        let mut file = File::open(filepath)?;
        let mut size = file.metadata()?.len() as usize;
        
        println!("The image is {}, size = {}", filepath, size);
        
        // make sure the image size is not too large
        if size > MEM_SIZE {
            size = MEM_SIZE;
            println!("Warning: Image truncated to fit in memory");
        }
        
        // load the file to the start of the memory
        let host_ptr = self.guest_to_host_mut(MEM_BASE)? as *mut u8;
        let slice = unsafe { std::slice::from_raw_parts_mut(host_ptr, size) };
        
        file.read_exact(slice)?;

        Ok(())
    }

    pub fn load_elf(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let data = fs::read(path)?;
        let obj = object::File::parse(&*data)?;
        
        // load each segments
        for segment in obj.segments() {
            if segment.address() == 0 { continue; }
            
            let data = segment.data()?;
            let addr = (segment.address() - MEM_BASE) as usize;
            let size = segment.size() as usize;
            
            // make sure the segment is not too large
            if size > MEM_SIZE {
                return Err("Segment out of memory bounds".into());
            }
            
            // copy the segment data to memory
            self.mem[addr..addr+size].copy_from_slice(data);
        }
        
        // 初始化栈指针 (根据 ELF 中的 .bss 或自定义链接脚本)
        // if let Some(stack_section) = obj.section_by_name(".stack") {
        //     self.regs[2] = stack_section.address() + stack_section.size(); // sp = stack_top
        // }
        
        Ok(())
    }

    pub fn print_elf(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let data = fs::read(path)?;
        let obj = object::File::parse(&*data)?;

        let entry = obj.entry();
        println!("Entry: {:?}", entry);
        
        // load each segments
        for segment in obj.segments() {
            if segment.address() == 0 { continue; }
            
            let data = segment.data()?;
            let addr = (segment.address()) as usize;
            let size = segment.size() as usize;

            println!("Segment: {:?}", segment);
            println!("Data: {:?}", data);
            println!("Addr: {:?}", addr);
            println!("Size: {:?}", size);
        }

        Ok(())
    }
}

// 单元测试
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_rw() {
        let mut mem = Memory::new();
        
        assert!(mem.mem_read(MEM_BASE - 1, 4).is_err());
        assert!(mem.mem_read(MEM_BASE + MEM_SIZE as u64, 4).is_err());
        
        mem.mem_write(MEM_BASE, 1, 0x12).unwrap();
        assert_eq!(mem.mem_read(MEM_BASE, 1).unwrap(), 0x12);
        
        mem.mem_write(MEM_BASE, 2, 0x1234).unwrap();
        assert_eq!(mem.mem_read(MEM_BASE, 2).unwrap(), 0x1234);
        
        mem.mem_write(MEM_BASE, 4, 0x12345678).unwrap();
        assert_eq!(mem.mem_read(MEM_BASE, 4).unwrap(), 0x12345678);
        
        mem.mem_write(MEM_BASE, 8, 0x0123456789ABCDEF).unwrap();
        assert_eq!(mem.mem_read(MEM_BASE, 8).unwrap(), 0x0123456789ABCDEF);
        
        mem.mem_write(MEM_BASE, 4, 0xDEADBEEF).unwrap();
        assert_eq!(mem.inst_fetch(MEM_BASE).unwrap(), 0xDEADBEEF);
    }

    #[test]
    fn test_image_loading() {
        let mut mem = Memory::new();
        
        mem.print_elf("testcase/elf/exp.elf").unwrap();
        
        // std::fs::remove_file(path).unwrap();
    }
}