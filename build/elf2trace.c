#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <libelf.h>
#include <gelf.h>
#include <fcntl.h>
#include <unistd.h>
#include <assert.h>

// 错误处理宏
#define ERROR(...) do { \
    fprintf(stderr, "ERROR: "); \
    fprintf(stderr, __VA_ARGS__); \
    fprintf(stderr, "\n"); \
    exit(EXIT_FAILURE); \
} while(0)

// 指令信息结构
typedef struct {
    uint64_t address;
    uint32_t instruction;
    char disasm[64];
} InstructionInfo;

// 简单反汇编函数
const char* disassemble_riscv(uint32_t inst) {
    uint8_t opcode = inst & 0x7F;
    
    switch(opcode) {
        case 0x03: return "LOAD";
        case 0x0F: return "FENCE";
        case 0x13: return "OP-IMM";
        case 0x17: return "AUIPC";
        case 0x1B: return "OP-IMM-32";
        case 0x23: return "STORE";
        case 0x2F: return "AMO";
        case 0x33: return "OP";
        case 0x3B: return "OP-32";
        case 0x37: return "LUI";
        case 0x63: return "BRANCH";
        case 0x67: return "JALR";
        case 0x6F: return "JAL";
        case 0x73: return "SYSTEM";
        default:   return "UNKNOWN";
    }
}

// 生成 trace 文件
void generate_trace(const char* output_path, InstructionInfo* instructions, size_t count) {
    FILE* trace_file = fopen(output_path, "w");
    if (!trace_file) {
        ERROR("Failed to open trace file: %s", output_path);
    }
    
    fprintf(trace_file, "# RISC-V Instruction Trace\n");
    fprintf(trace_file, "# Generated from ELF file\n");
    fprintf(trace_file, "# Address       Instruction   Disassembly\n");
    fprintf(trace_file, "# ---------------------------------------\n");
    
    for (size_t i = 0; i < count; i++) {
        fprintf(trace_file, "0x%016lx: %08x   %s\n", 
                instructions[i].address,
                instructions[i].instruction,
                instructions[i].disasm);
    }
    
    fclose(trace_file);
}

int main(int argc, char** argv) {
    if (argc != 3) {
        fprintf(stderr, "Usage: %s <input.elf> <output.trace>\n", argv[0]);
        return EXIT_FAILURE;
    }
    
    const char* elf_path = argv[1];
    const char* trace_path = argv[2];
    
    // 初始化 ELF 库
    if (elf_version(EV_CURRENT) == EV_NONE) {
        ERROR("ELF library initialization failed: %s", elf_errmsg(-1));
    }
    
    // 打开 ELF 文件
    int fd = open(elf_path, O_RDONLY, 0);
    if (fd < 0) {
        ERROR("Failed to open ELF file: %s", elf_path);
    }
    
    Elf* elf = elf_begin(fd, ELF_C_READ, NULL);
    if (!elf) {
        close(fd);
        ERROR("elf_begin() failed: %s", elf_errmsg(-1));
    }
    
    // 验证 ELF 类型
    if (elf_kind(elf) != ELF_K_ELF) {
        elf_end(elf);
        close(fd);
        ERROR("Not an ELF file: %s", elf_path);
    }
    
    // 获取 ELF 头
    GElf_Ehdr ehdr;
    if (gelf_getehdr(elf, &ehdr) == NULL) {
        elf_end(elf);
        close(fd);
        ERROR("gelf_getehdr() failed: %s", elf_errmsg(-1));
    }
    
    // 检查架构
    if (ehdr.e_machine != EM_RISCV) {
        elf_end(elf);
        close(fd);
        ERROR("Not a RISC-V ELF file");
    }
    
    printf("ELF Type: %s\n", ehdr.e_type == ET_EXEC ? "Executable" : "Other");
    printf("Entry Point: 0x%lx\n", (uint64_t)ehdr.e_entry);
    printf("Machine: RISC-V\n");
    
    // 遍历所有段
    size_t n;
    if (elf_getphdrnum(elf, &n) != 0) {
        elf_end(elf);
        close(fd);
        ERROR("elf_getphdrnum() failed: %s", elf_errmsg(-1));
    }
    
    // 分配指令存储
    InstructionInfo* instructions = malloc(100000 * sizeof(InstructionInfo));
    size_t instruction_count = 0;
    
    for (size_t i = 0; i < n; i++) {
        GElf_Phdr phdr;
        if (gelf_getphdr(elf, i, &phdr) == NULL) {
            continue;
        }
        
        // 只处理可加载的可执行段
        if (phdr.p_type != PT_LOAD || !(phdr.p_flags & PF_X)) {
            continue;
        }
        
        printf("Executable segment: 0x%lx - 0x%lx (size: %lu)\n",
               (uint64_t)phdr.p_vaddr,
               (uint64_t)phdr.p_vaddr + phdr.p_memsz,
               (uint64_t)phdr.p_memsz);
        
        // 读取段数据
        Elf_Data* data = elf_getdata_rawchunk(elf, 
                                             phdr.p_offset,
                                             phdr.p_filesz,
                                             ELF_T_BYTE);
        if (!data || !data->d_buf) {
            printf("Warning: Failed to read segment data\n");
            continue;
        }
        
        // 处理段中的指令
        uint64_t address = phdr.p_vaddr;
        uint8_t* buf = (uint8_t*)data->d_buf;
        size_t offset = 0;
        
        while (offset < data->d_size) {
            // 确保有足够的数据
            if (offset + 4 > data->d_size) {
                break;
            }
            
            // 读取指令 (假设32位指令)
            uint32_t inst = *(uint32_t*)(buf + offset);
            
            // 存储指令信息
            instructions[instruction_count].address = address;
            instructions[instruction_count].instruction = inst;
            strncpy(instructions[instruction_count].disasm, 
                    disassemble_riscv(inst), 
                    sizeof(instructions[0].disasm) - 1);
            
            instruction_count++;
            
            // 移动到下一条指令
            address += 4;
            offset += 4;
        }
    }
    
    printf("Extracted %zu instructions\n", instruction_count);
    
    // 生成 trace 文件
    generate_trace(trace_path, instructions, instruction_count);
    
    // 清理资源
    free(instructions);
    elf_end(elf);
    close(fd);
    
    printf("Trace file generated: %s\n", trace_path);
    return EXIT_SUCCESS;
}