use std::collections::VecDeque;

use crate::cpu::CPU;

// PC, Opcode, is_thumb
pub static mut PC_BACKTRACE: VecDeque<(u32, u32, bool, String, String)> = VecDeque::new();

pub fn replace_registers_in_string(cpu: &CPU, str: &String) -> String {
    str.replace("R0", format!("0x{:X}", cpu.read_register(0)).as_str())
        .replace("R1", format!("0x{:X}", cpu.read_register(1)).as_str())
        .replace("R2", format!("0x{:X}", cpu.read_register(2)).as_str())
        .replace("R3", format!("0x{:X}", cpu.read_register(3)).as_str())
        .replace("R4", format!("0x{:X}", cpu.read_register(4)).as_str())
        .replace("R5", format!("0x{:X}", cpu.read_register(5)).as_str())
        .replace("R6", format!("0x{:X}", cpu.read_register(6)).as_str())
        .replace("R7", format!("0x{:X}", cpu.read_register(7)).as_str())
        .replace("R8", format!("0x{:X}", cpu.read_register(8)).as_str())
        .replace("R9", format!("0x{:X}", cpu.read_register(9)).as_str())
        .replace("R10", format!("0x{:X}", cpu.read_register(10)).as_str())
        .replace("R11", format!("0x{:X}", cpu.read_register(11)).as_str())
        .replace("R12", format!("0x{:X}", cpu.read_register(12)).as_str())
        .replace("R13", format!("0x{:X}", cpu.read_register(13)).as_str())
        .replace("R14", format!("0x{:X}", cpu.read_register(14)).as_str())
        .replace("R15", format!("0x{:X}", cpu.read_register(15)).as_str())
        .replace("SP", format!("0x{:X}", cpu.read_register(13)).as_str())
        .replace("LR", format!("0x{:X}", cpu.read_register(14)).as_str())
        .replace("PC", format!("0x{:X}", cpu.read_register(15)).as_str())
}

#[cfg(not(feature = "backtrace"))]
pub fn print_cpu_backtrace() {
    println!("CPU Backtrace disabled. Enable with `--features backtrace`");
}

#[cfg(feature = "backtrace")]
pub fn print_cpu_backtrace() {
    println!("CPU Backtrace:");
    // Accesses static mut pc_backtrace. Must be unsafe due to multiple thread access
    unsafe {
        let mut i = 0;
        for (pc, opcode, thumb, asm_reg, asm) in PC_BACKTRACE.iter() {
            match thumb {
                false => {
                    //let (asm, _) = disassembler::disassemble_arm(*opcode, *pc);
                    println!("\t{i:2}: [0x{pc:08X}] => 0x{opcode:08X} => {asm_reg}")
                }
                true => {
                    //let (asm, _) = disassembler::disassemble_thumb(*opcode as u16);
                    println!("\t{i:2}: [0x{pc:08X}] =>     0x{opcode:04X} => {asm_reg}")
                }
            }
            println!("\t                               => {asm}");
            i += 1;
        }
    }
}
