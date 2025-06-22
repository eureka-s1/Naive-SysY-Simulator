

// fn load_elf(&mut self, elf_data: &[u8]) -> Result<()> {
//     let elf = ElfFile::parse(elf_data)?;
    
//     // 加载所有段到内存
//     for segment in elf.segments() {
//         if segment.size() > 0 {
//             let vaddr = segment.address() as usize;
//             let data = segment.data()?;
//             self.memory[vaddr..vaddr + data.len()].copy_from_slice(data);
//         }
//     }
    
//     // 设置 PC 到入口点（ELF 头中指定）
//     self.pc = elf.entry() as u64;
    
//     // 初始化栈指针（根据链接器脚本中的定义）
//     self.regs[2] = 0x80000000 + 128 * 1024;  // sp = RAM_BASE + RAM_SIZE
    
//     Ok(())
// }