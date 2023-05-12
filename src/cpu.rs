use crate::mem::Memory;
use log::*;
use std::fmt;

pub trait MMU {
    fn read_u8(&mut self, intern: bool, addr: u32) -> u8;
    fn read_u32(&mut self, intern: bool, addr: u32) -> u32;
    fn write_u8(&mut self, intern: bool, addr: u32, val: u8);
    fn write_u32(&mut self, intern: bool, addr: u32, val: u32);
    fn addr_valid(&self, addr: u32) -> bool;
}

pub struct CPU {
    pub registers: [u32; 16],
    pub reg_cpsr: u32,
    pub regs_spsr: [u32; 16], // 7 modes total, but this simplifies access a lot for little cost
    pub regs_fiq: [u32; 7],   // R8 -> R14
    pub regs_svc: [u32; 2],   // R13, R14
    pub regs_abt: [u32; 2],   // R13, R14
    pub regs_irq: [u32; 2],   // R13, R14
    pub regs_und: [u32; 2],   // R13, R14
    pub ram_work1: [u8; 256 * 1024],
    pub ram_work2: [u8; 32 * 1024],
    pub ram_palette: [u8; 1 * 1024],
    pub ram_video: [u8; 96 * 1024],
    pub ram_obj_attr: [u8; 1 * 1024],
    pub panic: bool,
    pub rom: Vec<u8>,
    pub bios: Vec<u8>,
    pub mem_ptr: u32,
}

impl MMU for CPU {
    fn read_u8(&mut self, intern: bool, addr: u32) -> u8 {
        if intern {
            self.mem_ptr = addr;

            if addr < 0x00003FFF {
                panic!("In bios `{:08X}`", addr);
            }
        }

        let addr = addr as usize;
        match addr {
            0x00000000..=0x00003FFF => 0, //self.bios[addr],
            0x02000000..=0x0203FFFF => self.ram_work1[addr - 0x02000000],
            0x03000000..=0x03007FFF => self.ram_work2[addr - 0x03000000],
            0x04000000..=0x040003FE => {
                if intern {
                    info!("Read from IO register `{:08X}`", addr);
                }
                0
            }
            0x08000000..=0x09FFFFFC => self.rom[addr - 0x08000000],
            _ => {
                error!("Panicked! Address out of range `{:08X}`", addr);
                self.panic = true;
                0
            }
        }
    }

    fn read_u32(&mut self, intern: bool, addr: u32) -> u32 {
        if intern {
            self.mem_ptr = addr;

            if addr < 0x00003FFF {
                panic!("In bios `{:08X}`", addr);
            }
        }

        match addr {
            0x00000000..=0x00003FFF => {
                let addr = addr as usize;
                0
                //((self.bios[addr + 3] as u32) << 24)
                //    | ((self.bios[addr + 2] as u32) << 16)
                //    | ((self.bios[addr + 1] as u32) << 8)
                //    | (self.bios[addr] as u32)
            }
            0x02000000..=0x0203FFFF => {
                let addr = (addr - 0x02000000) as usize;
                ((self.ram_work1[addr + 3] as u32) << 24)
                    | ((self.ram_work1[addr + 2] as u32) << 16)
                    | ((self.ram_work1[addr + 1] as u32) << 8)
                    | (self.ram_work1[addr] as u32)
            }
            0x03000000..=0x03007FFF => {
                let addr = (addr - 0x03000000) as usize;
                ((self.ram_work2[addr + 3] as u32) << 24)
                    | ((self.ram_work2[addr + 2] as u32) << 16)
                    | ((self.ram_work2[addr + 1] as u32) << 8)
                    | (self.ram_work2[addr] as u32)
            }
            0x04000000..=0x040003FE => {
                if intern {
                    info!("Read from IO register `{:08X}`", addr);
                }

                0
            }
            0x08000000..=0x09FFFFFC => {
                let addr = (addr - 0x08000000) as usize;
                ((self.rom[addr + 3] as u32) << 24)
                    | ((self.rom[addr + 2] as u32) << 16)
                    | ((self.rom[addr + 1] as u32) << 8)
                    | (self.rom[addr] as u32)
            }
            _ => {
                error!("Panicked! Address out of range `{:08X}`", addr);
                self.panic = true;
                0
            }
        }
    }

    fn write_u8(&mut self, intern: bool, addr: u32, val: u8) {
        if intern {
            self.mem_ptr = addr;
        }

        let addr = addr as usize;
        match addr {
            0x00000000..=0x00003FFF => {
                error!(
                    "Panicked! Cannot write to BIOS (`{:08X} => {:02X}`)",
                    addr, val
                );
                self.panic = true
            } //self.bios[addr] = val,
            0x02000000..=0x0203FFFF => self.ram_work1[addr - 0x02000000] = val,
            0x03000000..=0x03007FFF => self.ram_work2[addr - 0x03000000] = val,
            0x04000000..=0x040003FE => {
                info!("Write to IO register `{:08X} = {:02X}`", addr, val);
            }
            0x08000000..=0x09FFFFFC => self.rom[addr - 0x08000000] = val,
            _ => {
                error!("Panicked! Write Address out of range `{:08X}`", addr);
                self.panic = true;
            }
        }
    }

    fn write_u32(&mut self, intern: bool, addr: u32, val: u32) {
        if intern {
            self.mem_ptr = addr;

            if addr < 0x00003FFF {
                panic!("In bios `{:08X}`", addr);
            }
        }

        match addr {
            0x02000000..=0x0203FFFF => {
                let addr = (addr - 0x02000000) as usize;
                self.ram_work1[addr + 3] = ((val >> 24) & 0xFF) as u8;
                self.ram_work1[addr + 2] = ((val >> 16) & 0xFF) as u8;
                self.ram_work1[addr + 1] = ((val >> 8) & 0xFF) as u8;
                self.ram_work1[addr] = (val & 0xFF) as u8;
            }
            0x03000000..=0x03007FFF => {
                let addr = (addr - 0x03000000) as usize;
                self.ram_work2[addr + 3] = ((val >> 24) & 0xFF) as u8;
                self.ram_work2[addr + 2] = ((val >> 16) & 0xFF) as u8;
                self.ram_work2[addr + 1] = ((val >> 8) & 0xFF) as u8;
                self.ram_work2[addr] = (val & 0xFF) as u8;
            }
            0x04000000..=0x040003FE => {
                info!("Write to IO register `{:08X}` => {:08X}", addr, val)
            }
            0x08000000..=0x09FFFFFC => {
                error!(
                    "Panicked! Cannot write to ROM (`{:08X} => {:02X}`)",
                    addr, val
                );
                self.panic = true;
            }
            _ => {
                error!("Address out of range `{:08X}`", addr);
                self.panic = true;
            }
        }
    }

    fn addr_valid(&self, addr: u32) -> bool {
        match addr {
            (0x00000000..=0x00003FFF)
            | (0x02000000..=0x0203FFFF)
            | (0x03000000..=0x03007FFF)
            | (0x04000000..=0x040003FE)
            | (0x08000000..=0x09FFFFFC) => true,
            _ => false,
        }
    }
}

impl fmt::Debug for CPU {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "CPU Core Dump:")?;
        match self.is_thumb() {
            false => {
                writeln!(f, "\tMode: ARM\n")?;
                for i in 0..15 {
                    writeln!(
                        f,
                        "\tR{}\t=> {:08X}h ({})",
                        i, self.registers[i], self.registers[i]
                    )?;
                }

                writeln!(
                    f,
                    "\tR15 (PC)=> {:08X}h ({})\n",
                    self.registers[15], self.registers[15]
                )?;
            }
            true => {
                writeln!(f, "\tMode: THUMB")?;
                writeln!(f, "")?;
            }
        }

        writeln!(f, "\tStatus (CPSR):")?;
        writeln!(f, "\t\t31 30 29 28")?;
        writeln!(f, "\t\t N  Z  C  V")?;
        writeln!(
            f,
            "\t\t {}  {}  {}  {}",
            self.get_flag_n() as u8,
            self.get_flag_z() as u8,
            self.get_flag_c() as u8,
            self.get_flag_v() as u8
        )?;

        Ok(())
    }
}

pub const ARM_MASK_MUL_CLR: u32 = 0xFC00060;
pub const ARM_MASK_MUL_SET: u32 = 0x0000090;
pub const ARM_MASK_MUL_LONG_CLR: u32 = 0xF000090;
pub const ARM_MASK_MUL_LONG_SET: u32 = 0x0800090;
pub const ARM_MASK_SNGL_SWP_CLR: u32 = 0xEB00F60;
pub const ARM_MASK_SNGL_SWP_SET: u32 = 0x1000090;
pub const ARM_MASK_BX_CLR: u32 = 0xED000E0;
pub const ARM_MASK_BX_SET: u32 = 0x12FFF10;
pub const ARM_MASK_HW_REG_CLR: u32 = 0xE400F00;
pub const ARM_MASK_HW_REG_SET: u32 = 0x0000090;
pub const ARM_MASK_HW_IMM_CLR: u32 = 0xE000000;
pub const ARM_MASK_HW_IMM_SET: u32 = 0x0400090;
pub const ARM_MASK_UNDEF_CLR: u32 = 0x08000000;
pub const ARM_MASK_UNDEF_SET: u32 = 0x06000010;
pub const ARM_MASK_MRS_CLR: u32 = 0x0EB00FFF;
pub const ARM_MASK_MRS_SET: u32 = 0x010F0000;
pub const ARM_MASK_MSR_CLR: u32 = 0x0E960FF0;
pub const ARM_MASK_MSR_SET: u32 = 0x0129F000;
pub const ARM_MASK_MSR_BITS_CLR: u32 = 0x0C970000;
pub const ARM_MASK_MSR_BITS_SET: u32 = 0x0128F000;

const FLAG_MASK_N: u32 = 0x80000000;
const FLAG_MASK_Z: u32 = 0x40000000;
const FLAG_MASK_C: u32 = 0x20000000;
const FLAG_MASK_V: u32 = 0x10000000;

const MODE_USER: u8 = 0x0;
const MODE_FIQ: u8 = 0x1;
const MODE_IRQ: u8 = 0x2;
const MODE_SUPERVISOR: u8 = 0x3;
const MODE_ABORT: u8 = 0x7;
const MODE_UNDEFINED: u8 = 0xB;
const MODE_SYSTEM: u8 = 0xF;

const STATUS_FLAG_F: u32 = 0x40;
const STATUS_FLAG_I: u32 = 0x80;

const ALU_AND: u8 = 0x0;
const ALU_EOR: u8 = 0x1;
const ALU_SUB: u8 = 0x2;
const ALU_RSB: u8 = 0x3;
const ALU_ADD: u8 = 0x4;
const ALU_ADC: u8 = 0x5;
const ALU_SBC: u8 = 0x6;
const ALU_RSC: u8 = 0x7;
const ALU_TST: u8 = 0x8;
const ALU_TEQ: u8 = 0x9;
const ALU_CMP: u8 = 0xA;
const ALU_CMN: u8 = 0xB;
const ALU_ORR: u8 = 0xC;
const ALU_MOV: u8 = 0xD;
const ALU_BIC: u8 = 0xE;
const ALU_MVN: u8 = 0xF;

impl CPU {
    pub fn new() -> Self {
        Self {
            mem_ptr: 0,
            registers: [0; 16],
            reg_cpsr: STATUS_FLAG_F | STATUS_FLAG_I | (MODE_SUPERVISOR as u32),
            regs_spsr: [0; 16],
            regs_fiq: [0; 7],
            regs_svc: [0; 2],
            regs_abt: [0; 2],
            regs_irq: [0; 2],
            regs_und: [0; 2],
            ram_work1: [0; 256 * 1024],
            ram_work2: [0; 32 * 1024],
            ram_palette: [0; 1 * 1024],
            ram_video: [0; 96 * 1024],
            ram_obj_attr: [0; 1 * 1024],
            panic: false,
            rom: Vec::new(),
            bios: Vec::new(),
        }
    }

    pub fn load_rom(&mut self, rom: &Vec<u8>) {
        self.rom = rom.to_vec();
    }

    pub fn load_bios(&mut self, bios: &Vec<u8>) {
        self.bios = bios.to_vec();
    }

    fn get_flag_n(&self) -> bool {
        self.reg_cpsr & FLAG_MASK_N > 0
    }

    /// Returns Mode (FIQ, IRQ etc)
    pub fn get_mode(&self) -> u8 {
        (self.reg_cpsr & 0xF) as u8
    }

    fn set_mode(&mut self, mode: u8) {
        self.reg_cpsr = (self.reg_cpsr & 0xFFFFFFE0) | (mode as u32);
    }

    fn set_flag_n(&mut self, set: bool) {
        match set {
            true => self.reg_cpsr |= FLAG_MASK_N,
            false => self.reg_cpsr &= !(FLAG_MASK_N),
        }
    }

    fn get_flag_z(&self) -> bool {
        self.reg_cpsr & FLAG_MASK_Z > 0
    }

    fn set_flag_z(&mut self, set: bool) {
        match set {
            true => self.reg_cpsr |= FLAG_MASK_Z,
            false => self.reg_cpsr &= !(FLAG_MASK_Z),
        }
    }

    fn get_flag_c(&self) -> bool {
        self.reg_cpsr & FLAG_MASK_C > 0
    }

    fn set_flag_c(&mut self, set: bool) {
        match set {
            true => self.reg_cpsr |= FLAG_MASK_C,
            false => self.reg_cpsr &= !(FLAG_MASK_C),
        }
    }

    fn get_flag_v(&self) -> bool {
        self.reg_cpsr & FLAG_MASK_V > 0
    }

    fn set_flag_v(&mut self, set: bool) {
        match set {
            true => self.reg_cpsr |= FLAG_MASK_V,
            false => self.reg_cpsr &= !(FLAG_MASK_V),
        }
    }

    pub fn reset(&mut self) {
        self.registers = [0; 16];
        self.reg_cpsr = STATUS_FLAG_F | STATUS_FLAG_I | (MODE_SUPERVISOR as u32);
        self.regs_spsr = [0; 16];

        self.set_program_counter(0x08000000);
        //self.set_program_counter(0x00000000);

        // Clear panic flag
        self.panic = false;
    }

    pub fn read_register(&self, register: u8) -> u32 {
        let mode = self.get_mode();
        match (
            register,
            mode != MODE_SYSTEM && mode != MODE_USER,
            mode == MODE_FIQ,
        ) {
            (0..=7, _, _) | (8..=14, false, _) | (8..=12, true, false) | (15, _, _) => {
                self.registers[register as usize]
            }
            (8..=14, true, true) => self.regs_fiq[(register - 8) as usize], // FIQ
            (13..=14, true, _) => match mode {
                MODE_SUPERVISOR => self.regs_svc[(register - 13) as usize],
                MODE_ABORT => self.regs_abt[(register - 13) as usize],
                MODE_IRQ => self.regs_irq[(register - 13) as usize],
                MODE_UNDEFINED => self.regs_und[(register - 13) as usize],
                _ => unreachable!("register {} invalid", register),
            },
            _ => unreachable!("register {} invalid (mode={})", register, mode),
        }
    }

    fn write_register(&mut self, register: u8, value: u32) {
        let mode = self.get_mode();
        match (
            register,
            mode != MODE_SYSTEM && mode != MODE_USER,
            mode == MODE_FIQ,
        ) {
            (0..=7, _, _) | (8..=14, false, _) | (8..=12, true, false) | (15, _, _) => {
                self.registers[register as usize] = value
            }
            (8..=14, true, true) => self.regs_fiq[(register - 8) as usize] = value, // FIQ
            (13..=14, true, _) => match mode {
                MODE_SUPERVISOR => self.regs_svc[(register - 13) as usize] = value,
                MODE_ABORT => self.regs_abt[(register - 13) as usize] = value,
                MODE_IRQ => self.regs_irq[(register - 13) as usize] = value,
                MODE_UNDEFINED => self.regs_und[(register - 13) as usize] = value,
                _ => unreachable!("register {} invalid (mode={})", register, mode),
            },
            _ => unreachable!("register {} invalid (mode={})", register, mode),
        }
    }

    pub fn is_thumb(&self) -> bool {
        (self.reg_cpsr & 0x20) != 0
    }

    fn set_thumb(&mut self, thumb: bool) {
        match thumb {
            false => self.reg_cpsr &= !(0x20),
            true => self.reg_cpsr |= 0x20,
        }
    }

    fn disable_irq(&mut self, disable: bool) {
        match disable {
            false => self.reg_cpsr &= !(0x80),
            true => self.reg_cpsr |= 0x80,
        }
    }

    pub fn get_program_counter(&self) -> u32 {
        self.registers[15]
    }

    // TODO: set to private
    pub fn set_program_counter(&mut self, addr: u32) {
        self.registers[15] = addr;
    }

    fn step_program_counter(&mut self, steps: u32) {
        self.registers[15] += steps;
    }

    fn bios_syscall(&mut self, syscall: u8) {
        match syscall {
            0x01 => {
                let flags = self.read_register(0) & 0xFF;

                info!(
                    "HLE: executing syscall `RegisterRamReset` with `{:08b}`",
                    flags
                );

                if (flags & 0x1) != 0 {
                    self.ram_work1 = [0; 256 * 1024];
                }
                if (flags & 0x2) != 0 {
                    for i in 0..(0x7E00) {
                        self.ram_work2[i] = 0;
                    }
                }
                if (flags & 0x4) != 0 {
                    self.ram_palette = [0; 1 * 1024];
                }
                if (flags & 0x8) != 0 {
                    self.ram_video = [0; 96 * 1024];
                }
                if (flags & 0x10) != 0 {
                    self.ram_obj_attr = [0; 1 * 1024];
                }
                if (flags & 0x20) != 0 {
                    // Clear SIO
                }
                if (flags & 0x40) != 0 {
                    // Clear Sound registers
                }
                if (flags & 0x80) != 0 {
                    // Clear other registers
                }
            }
            _ => panic!("Unknown BIOS syscall `{:02X}h`", syscall),
        }
    }

    fn thumb_move_shifted_register(&mut self, opcode: u16) {
        let rd = (opcode & 0x3) as usize;
        let rs = ((opcode >> 3) & 0x3) as usize;

        let offset = ((opcode >> 6) & 0x1F) as u8;
        let op = ((opcode >> 11) & 0x3) as u8;

        match op {
            0x0 => {
                info!("execute: `LSL R{},R{},#{}`", rd, rs, offset);
                self.registers[rd] = self.registers[rs] << offset
            }
            0x1 => {
                info!("execute: `LSR R{},R{},#{}`", rd, rs, offset);
                self.registers[rd] = self.registers[rs] >> offset
            }
            0x2 => {
                info!("execute: `ASR R{},R{},#{}`", rd, rs, offset);
                // Arithmetic shift
                let msb = self.registers[rs] & 0xF0000000;
                self.registers[rd] = msb | (self.registers[rs] >> offset);
            }
            _ => {
                error!("Invalid opcode for move_shifted_register {}", op);
                self.panic = true
            }
        }

        self.step_program_counter(2);
    }

    fn thumb_hi_register_op_bx(&mut self, opcode: u16) {
        let rd = (opcode & 0x7) as u8;
        let rs = ((opcode >> 3) & 0x7) as u8;
        let h2 = (opcode & 0x40) != 0;
        let h1 = (opcode & 0x80) != 0;
        let op = (opcode >> 8) & 0x3;

        let rd = match h1 {
            false => rd,
            true => rd + 8,
        };

        let rs = match h2 {
            false => rs,
            true => rs + 8,
        };

        let rs_val = if rs == 15 {
            (self.read_register(rs) + 4) & 0xFFFFFFFE
        } else {
            self.read_register(rs)
        };

        let rd_val = if rs == 15 {
            (self.read_register(rd) + 4) & 0xFFFFFFFE
        } else {
            self.read_register(rd)
        };

        match op {
            0x0 => {
                info!("execute: `ADD R{},R{}`", rd, rs);
                self.write_register(rd, rd_val.wrapping_add(rs_val));
            }
            0x1 => {
                info!("execute: `CMP R{},R{}", rd, rs);
                todo!("Implement CMP");
            }
            0x2 => {
                info!("execute: `MOV R{},R{}", rd, rs);
                self.write_register(rd, rs_val);
            }
            0x3 => {
                // TODO: Refactor
                let thumb = (rs_val & 0x1) == 0x1;

                info!("execute: `BX R{}` => thumb={}", rs, thumb);

                self.set_thumb(thumb);
                match thumb {
                    false => self.set_program_counter(rs_val),
                    true => self.set_program_counter(rs_val & 0xFFFFFFFE),
                }
            }
            _ => unreachable!("op > 0x3 (`{}`)", opcode),
        }

        self.step_program_counter(2);
    }

    fn thumb_push_pop(&mut self, opcode: u16) {
        let load = (opcode & 0x0800) != 0;
        let store_lr = (opcode & 0x0100) != 0;
        let rlist = (opcode & 0xFF);

        if load {
            info!("execute: `POP`");
            todo!("Implement POP");
        } else {
            let mut sp = self.read_register(13);

            // Push LR
            if store_lr {
                self.write_u32(true, sp, self.read_register(14));
                sp -= 4;
                info!("execute: `PUSH {{R{:08b}, LR}}`", rlist);
            } else {
                info!("execute: `PUSH {{R{:08b}}}`", rlist);
            }

            // Push R0-R7
            for i in 0..8 {
                if (rlist & (1 << i)) != 0 {
                    self.write_u32(true, sp, self.read_register(i));
                    sp -= 4;
                }
            }

            // Fix SP
            self.write_register(13, sp);
        }

        self.step_program_counter(2);
    }

    fn thumb_mov_cmp_add_sub_imm(&mut self, opcode: u16) {
        let offset = (opcode & 0xFF) as u32;
        let rd = ((opcode >> 8) & 0x7) as u8;
        let op = ((opcode >> 11) & 0x3) as u8;
        let rd_val = self.read_register(rd);

        let (val, carry) = match op {
            0b00 => {
                info!("execute: `MOV R{},#{}`", rd, offset);
                (offset, false)
            }
            0b01 => {
                info!("execute: `CMP R{},#{}`", rd, offset);
                rd_val.overflowing_sub(offset)
            }
            0b10 => {
                info!("execute: `ADD R{},#{}`", rd, offset);
                rd_val.overflowing_add(offset)
            }
            0b11 => {
                info!("execute: `SUB R{},#{}`", rd, offset);
                rd_val.overflowing_sub(offset)
            }
            _ => unreachable!(""),
        };

        self.set_flag_n((val & 0x80000000) != 0);
        self.set_flag_z(val == 0);
        self.set_flag_c(carry);
        self.set_flag_v((rd_val & 0x80000000) != (val & 0x80000000));
        // TODO: is this correct for overflow & carry?

        if op != 0b01 {
            self.write_register(rd, val);
        }

        self.step_program_counter(2);
    }

    fn thumb_long_branch_link(&mut self, opcode: u16) {
        let offset = (opcode & 0x7FF) as u32;
        let h = (opcode & 0x0800) != 0;

        match h {
            false => {
                self.write_register(
                    14,
                    (self.get_program_counter().wrapping_add(4)).wrapping_add(offset << 12),
                );
                self.step_program_counter(2);
            }
            true => {
                let next = self.get_program_counter() + 2;
                self.set_program_counter(self.read_register(14).wrapping_add(offset << 1));
                self.write_register(14, next | 1);
            }
        }

        info!("execute: `BL{} {}`", h as u8, offset);
    }

    fn thumb_swi(&mut self, opcode: u16) {
        //let next = self.get_program_counter() + 2;

        //// Store CPSR in SPSR of current mode
        //self.regs_spsr[MODE_SUPERVISOR as usize] = self.reg_cpsr;

        //// Set PC to SWI vector address (0x8)
        //self.set_program_counter(0x8);

        //// Switch to ARM state
        //self.set_thumb(false);

        //// Enter SVC
        //self.set_mode(MODE_SUPERVISOR);

        //// Set IRQ disable
        //self.disable_irq(true);

        //// Move address of next instruction to LR
        //self.write_register(14, next);

        let syscall = (opcode & 0xFF) as u8;

        info!("execute: `SWI {:02X}`", syscall);

        self.bios_syscall(syscall);
        self.step_program_counter(2);
    }

    fn thumb_unconditional_branch(&mut self, opcode: u16) {
        let offset = ((opcode & 0x7FF) << 1) as u32;
        let neg = (offset & 0x400) != 0;

        let extend = match neg {
            false => 0xFFFFF000 | offset,
            true => offset,
        };

        info!("execute: `B {}`", extend as i32);

        self.set_program_counter(
            self.get_program_counter()
                .wrapping_add(extend)
                .wrapping_add(1),
        );
    }

    fn thumb_load_store_immediate(&mut self, opcode: u16) {
        let rd = (opcode & 0x7) as u8;
        let rb = ((opcode >> 3) & 0x7) as u8;
        let offset = ((opcode >> 6) & 0x1F) as u32;
        let load = (opcode & 0x800) != 0;
        let byte = (opcode & 0x1000) != 0;

        let ptr = self.read_register(rb).wrapping_add(offset);

        match (load, byte) {
            (false, false) => {
                info!("execute: `STR R{},[R{},#{}]`", rd, rb, offset);

                self.write_u32(true, ptr, self.read_register(rd));
            }
            (true, false) => {
                info!("execute: `LDR R{},[R{},#{}]`", rd, rb, offset);
                let val = self.read_u32(true, ptr);
                self.write_register(rd, val);
            }
            (false, true) => {
                info!("execute: `STRB R{},[R{},#{}]`", rd, rb, offset);

                self.write_u8(true, ptr, (self.read_register(rd) & 0xFF) as u8);
            }
            (true, true) => {
                info!("execute: `LDRB R{},[R{},#{}]`", rd, rb, offset);
                let val = self.read_u8(true, ptr) as u32;
                self.write_register(rd, val);
            }
        }

        self.step_program_counter(2);
    }

    fn thumb_load_store_halfword(&mut self, opcode: u16) {
        let rd = (opcode & 0x7) as u8;
        let rb = ((opcode >> 3) & 0x7) as u8;
        let offset = (((opcode >> 6) & 0x1F) << 1) as u32;
        let load = (opcode & 0x800) != 0;

        let addr = self.read_register(rb).wrapping_add(offset as u32);
        let addr_val = self.read_u32(true, addr);

        match load {
            false => {
                info!("execute: `STRH R{},[R{}, #0x{:02X}]`", rd, rb, offset);
                self.write_u32(
                    true,
                    addr,
                    (addr_val & 0xFFFF0000) | (self.read_register(rd) & 0xFFFF),
                )
            }
            true => {
                info!("execute: `LDRH R{},[R{}, #0x{:02X}]`", rd, rb, offset);
                self.write_register(rd, addr_val & 0xFFFF)
            }
        }

        self.step_program_counter(2);
    }

    fn thumb_pc_relative_load(&mut self, opcode: u16) {
        let word = ((opcode & 0xFF) << 2) as u32;
        let rd = ((opcode >> 8) & 0x7) as u8;

        info!("execute: `LDR R{},[PC,#0x{:02X}]`", rd, word);

        let addr = self.get_program_counter().wrapping_add(word);
        let val = self.read_u32(true, addr);

        self.write_register(rd, val);

        self.step_program_counter(2);
    }

    fn alu(&mut self, opcode: u8, operand1: u32, operand2: u32, set_condition: bool) -> u32 {
        0
    }

    fn thumb_add_subtract(&mut self, opcode: u16) {
        let offset = (opcode & 0xFF) as u32;
        let rd = ((opcode >> 8) & 0x7) as u8;
        let op = ((opcode >> 11) & 0x3) as u8;

        match op {
            0b00 => (format!("MOV R{},#{}", rd, offset), bits),
            0b01 => (format!("CMP R{},#{}", rd, offset), bits),
            0b10 => (format!("ADD R{},#{}", rd, offset), bits),
            0b11 => (format!("SUB R{},#{}", rd, offset), bits),
            _ => unreachable!(""),
        }
    }

    fn execute_thumb(&mut self, opcode: u16) {
        let high = (opcode >> 8) as u8;

        match high {
            0x00..=0x17 => {
                self.thumb_move_shifted_register(opcode);
            } // Move shifted register
            0x18..=0x1F => {
                todo!("Add/subtract");
            } // Add/subtract
            0x20..=0x3F => {
                self.thumb_mov_cmp_add_sub_imm(opcode);
            } // Move/compare/add/subtract immediate
            0x40..=0x43 => {
                todo!("ALU operations");
            } // ALU operations
            0x44..=0x47 => {
                self.thumb_hi_register_op_bx(opcode);
            } // Hi register operations/branch exchange
            0x48..=0x4F => {
                self.thumb_pc_relative_load(opcode);
            } // PC-relative load
            0x50 | 0x51 | 0x54 | 0x55 | 0x58 | 0x59 | 0x5C | 0x5D => {
                todo!("Load/store with register offset");
            } // Load/store with register offset
            0x52 | 0x53 | 0x56 | 0x57 | 0x5A | 0x5B | 0x5E | 0x5F => {
                todo!("Load/store sign-extended byte/halfword");
            } // Load/store sign-extended byte/halfword
            0x60..=0x7F => {
                self.thumb_load_store_immediate(opcode);
            } // load/store with immediate offset
            0x80..=0x8F => {
                self.thumb_load_store_halfword(opcode);
            } // Load/store halfword
            0x90..=0x9F => {
                todo!("SP-relative load/store");
            } // SP-relative load/store
            0xA0..=0xAF => {
                todo!("Load address");
            } // Load address
            0xB0 => {
                todo!("Add offset to stack pointer");
            } // Add offset to stack pointer
            0xB4 | 0xB5 | 0xBC | 0xBD => {
                self.thumb_push_pop(opcode);
            } // Push/pop registers
            0xC0..=0xCF => {
                todo!("Multiple load/store");
            } // Multiple load/store
            0xD0..=0xDE => {
                todo!("Conditional branch");
            } // Conditional branch
            0xDF => {
                self.thumb_swi(opcode);
            } // Software interrupt
            0xE0..=0xE7 => {
                self.thumb_unconditional_branch(opcode);
            } // Unconditional branch,
            0xF0..=0xFF => {
                self.thumb_long_branch_link(opcode);
            } // Long branch with link
            _ => {
                error!("[THUMB] Unknown opcode: {:04X}h ({:016b}b)", opcode, opcode);
                self.panic = true;
            }
        }
    }

    fn should_execute(&mut self, conditional: u8) -> bool {
        let n = self.get_flag_n();
        let z = self.get_flag_z();
        let c = self.get_flag_c();
        let v = self.get_flag_v();

        match conditional {
            0x0 => z,
            0x1 => !z,
            0x2 => c,
            0x3 => !c,
            0x4 => n,
            0x5 => !n,
            0x6 => v,
            0x7 => !v,
            0x8 => c && !z,
            0x9 => !c || z,
            0xA => n == v,
            0xB => n != v,
            0xC => !z && (n == v),
            0xD => z || (n != v),
            0xE => true,
            _ => {
                error!("Conditional > 0xE");
                self.panic = true;
                false
            }
        }
    }

    fn arm_branch(&mut self, opcode: u32) {
        let link = (opcode & 0x01000000) != 0;
        let mut offset = (opcode & 0xFFFFFF) << 2;

        // Sign extend
        if offset >= 0x02000000 {
            offset |= 0xFC000000;
        }

        // Current pc + 8 bytes for prefetch
        let target = self.get_program_counter() + 8;
        let target = target.wrapping_add(offset);

        if link {
            info!("execute: `BL 0x{:X}` => 0x{:08X}", offset, target);
            self.write_register(14, self.get_program_counter() + 4);
        } else {
            info!("execute: `B 0x{:X}` => 0x{:08X}", offset + 8, target);
        }

        self.set_program_counter(target);
    }

    fn arm_data_processing(&mut self, opcode: u32) {
        let rd = ((opcode >> 12) & 0xF) as u8;
        let rn = ((opcode >> 16) & 0xF) as u8;
        let operand1 = if rn == 15 {
            self.read_register(rn) + 8
        } else {
            self.read_register(rn)
        };

        let set_condition = (opcode & 0x100000) != 0;
        let op = ((opcode >> 21) & 0xF) as u8;
        let i = (opcode & 0x2000000) != 0;

        let s_str = match set_condition {
            false => "",
            true => "S",
        };

        let (operand2, shifter_carry) = if i {
            let imm = opcode & 0xFF;
            let rot = ((opcode >> 8) & 0xF) << 1;

            // TODO: Is this correct, we preserve C flag on immediates
            warn!("Data Processing: Preserving C flag on immediates, correct?");
            (imm.rotate_right(rot), self.get_flag_c())
        } else {
            let rm = (opcode & 0xF) as usize;
            let shift_type = ((opcode >> 5) & 0x2) as u8;
            let is_reg = (opcode & 0x10) != 0x00;

            let rm_value = self.read_register(rm as u8);

            let (shift_amount, rm_value) = if !is_reg {
                let shift = (opcode >> 6) & 0x1F;
                if rm == 15 {
                    (shift, rm_value + 8)
                } else {
                    (shift, rm_value)
                }
            } else {
                let shift = self.read_register(((opcode >> 7) & 0xF) as u8);
                if rm == 15 {
                    (shift, rm_value + 12)
                } else {
                    (shift, rm_value)
                }
            };

            match shift_type {
                0x0 => {
                    let (result, overflow) = rm_value.overflowing_shl(shift_amount);
                    let carry = if shift_amount == 32 {
                        (rm_value & 0x1) == 0x1
                    } else if shift_amount > 32 {
                        false
                    } else {
                        overflow
                    };

                    (result, carry)
                }
                0x1 => {
                    let (result, overflow) = rm_value.overflowing_shr(shift_amount);
                    let carry = if shift_amount == 32 {
                        (rm_value >> 31) == 0x1
                    } else if shift_amount > 32 {
                        false
                    } else {
                        overflow
                    };

                    (result, carry)
                }
                0x2 => todo!("arithmetic right"),
                0x3 => todo!("rotate right"),
                _ => unreachable!(),
            }
        };

        let (result, logical, carry, overflow) = match op {
            0x0 | 0x8 => {
                if op == 0x0 {
                    info!("execute: `AND`");
                } else {
                    info!("execute: `TST`");
                }
                (operand1 & operand2, true, false, false)
            }
            0x1 | 0x9 => {
                if op == 0x1 {
                    info!("execute: `EOR {},{}`", operand1, operand2);
                } else {
                    info!("execute: `TEQ {},{}`", operand1, operand2);
                }
                (operand1 ^ operand2, true, false, false)
            }
            0x2 | 0xA => {
                if op == 0x2 {
                    info!("execute: `SUB ???`");
                } else {
                    info!("execute: `CMP ???`");
                }
                let (result, carry) = operand1.overflowing_sub(operand2);
                let overflow = (operand1 ^ result) & 0x80000000 > 0;
                (result, false, carry, overflow)
            }
            0x3 => {
                info!("execute: `RSB ???`");
                let (result, carry) = operand2.overflowing_sub(operand1);
                let overflow = (operand2 ^ result) & 0x80000000 > 0;
                (result, false, carry, overflow)
            }
            0x4 | 0xB => {
                if op == 0x4 {
                    info!("execute: `ADD{} R{},R{},???`", s_str, rd, rn);
                } else {
                    info!("execute: `CMN ???`");
                }
                let (result, carry) = operand1.overflowing_add(operand2);
                let overflow = (operand1 ^ result) & 0x80000000 > 0;
                (result, false, carry, overflow)
            }
            0x5 => {
                info!("execute: `ADC ???`");
                let (result, carry) = operand1.overflowing_add(self.get_flag_c() as u32);
                let (result, carry2) = result.overflowing_add(operand2);
                let overflow = (operand1 ^ result) & 0x80000000 > 0;
                (result, false, carry | carry2, overflow)
            }
            0x6 => {
                info!("execute: `SBC ???`");
                let (result, carry) = operand2.overflowing_add(self.get_flag_c() as u32);
                let (result, carry2) = result.overflowing_sub(1);
                let (result, carry3) = operand1.overflowing_sub(result);
                let overflow = (operand1 ^ result) & 0x80000000 > 0;

                // TODO: Is this correct?
                warn!("Data Processing: SBC, correct?");
                (result, false, carry | carry2 | carry3, overflow)
            }
            0x7 => {
                info!("execute: `RSC ???`");
                let (result, carry) = operand1.overflowing_add(self.get_flag_c() as u32);
                let (result, carry2) = result.overflowing_sub(1);
                let (result, carry3) = operand2.overflowing_sub(result);
                let overflow = (operand2 ^ result) & 0x80000000 > 0;

                // TODO: Is this correct?
                warn!("Data Processing: RSC, correct?");
                (result, false, carry | carry2 | carry3, overflow)
            }
            0xC => {
                info!("execute: `ORR ???`");
                (operand1 | operand2, true, false, false)
            }
            0xD => {
                info!("execute: `MOV R{},#0x{:X}`", rd, operand2);
                (operand2, true, false, false)
            }
            0xE => {
                info!("execute: `BIC ???`");
                (operand1 & !(operand2), true, false, false)
            }
            0xF => {
                info!("execute: `MVN`");
                (!operand2, true, false, false)
            }
            _ => unreachable!(),
        };

        // Set flags
        // TODO: handle R15 as Rd
        // When Rd is R15 and the S flag is set the result of the operating
        // is placed in R15 and the SPSR corresponding to the current mode is moved to the CPSR
        // This allows state changes which atomically resotre both PC and CPSR. This
        // form of instruction should not be used in User mode.
        if set_condition || (op >= 0x8 && op <= 0xB) {
            if rd == 0xF {
                todo!("Data Processing with R15 as write register and S flag set")
            }

            self.set_flag_n(result & 0x80000000 == 0x80000000);
            self.set_flag_z(result == 0);

            if !logical {
                self.set_flag_c(carry);
                self.set_flag_v(overflow);
            } else {
                self.set_flag_c(shifter_carry);
            }
        }

        // If not test opcode, store result
        if op < 0x8 || op > 0xB {
            self.write_register(rd, result);
        }

        self.step_program_counter(4);
    }

    fn arm_shifted_offset(&self) {
        todo!("Implement");
    }

    fn arm_single_data_transfer(&mut self, opcode: u32) {
        let mut offset = (opcode & 0xFFF);
        let rd = (opcode >> 12) & 0xF;
        let rn = (opcode >> 16) & 0xF;

        let load = (opcode & (1 << 20)) != 0;
        let write_back = (opcode & (1 << 21)) != 0;
        let byte = (opcode & (1 << 22)) != 0;
        let add = (opcode & (1 << 23)) != 0;
        let pre_index = (opcode & (1 << 24)) != 0;
        let imm = (opcode & (1 << 25)) != 0;

        if rd == 15 && !load {
            todo!("Implement store with dest as r15");
        }

        // Shifted offset
        if imm {
            let rm = (offset & 0xF) as usize;
            let shift = (offset >> 4);

            todo!("Implement shifts in LDR/STR");
        }

        let base = if rn == 15 {
            self.read_register(15).wrapping_add(8)
        } else {
            self.read_register(rn as u8)
        };

        let offsetted_addr = if add {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        };

        if load {
            info!("execute: `LDR R{},???`", rd);
            let val = match (pre_index, byte) {
                (false, false) => self.read_u32(true, base),
                (false, true) => self.read_u8(true, base) as u32,
                (true, false) => self.read_u32(true, offsetted_addr),
                (true, true) => self.read_u8(true, offsetted_addr) as u32,
            };
            self.write_register(rd as u8, val);
        } else {
            info!("execute: `STR R{},???`", rd);
            let val = self.read_register(rd as u8);
            match (pre_index, byte) {
                (false, false) => self.write_u32(true, base, val),
                (false, true) => self.write_u8(true, base, val as u8),
                (true, false) => self.write_u32(true, offsetted_addr, val),
                (true, true) => self.write_u8(true, offsetted_addr, val as u8),
            };
        }

        // Write back to base register
        if write_back || !pre_index {
            self.write_register(rn as u8, offsetted_addr);
        }

        self.step_program_counter(4);
    }

    fn arm_msr(&mut self, opcode: u32) {
        let dest_spsr = (opcode & 0x400000) != 0;
        let rm = (opcode & 0xF) as u8;
        let rm_val = self.read_register(rm);

        if dest_spsr {
            info!("execute: `MSR SPSR,R{}`", rm);
            self.regs_spsr[self.get_mode() as usize] = rm_val;
        } else {
            info!("execute: `MSR CPSR,R{}`", rm);

            if self.get_mode() == MODE_USER {
                self.reg_cpsr = (self.reg_cpsr & 0x0FFFFFFF) | (rm_val & 0xF0000000);
            } else {
                self.reg_cpsr = rm_val;
            }
        }

        self.step_program_counter(4);
    }

    fn arm_branch_exchange(&mut self, opcode: u32) {
        let rm = (opcode & 0xF) as u8;
        let rm_val = self.read_register(rm);
        let thumb = (rm_val & 0x1) == 0x1;

        info!("execute: `BX R{}` => thumb={}", rm, thumb);

        self.set_thumb(thumb);
        match thumb {
            false => self.set_program_counter(rm_val),
            true => self.set_program_counter(rm_val & 0xFFFFFFFE),
        }
    }

    fn arm_block_data_transfer(&mut self, opcode: u32) {
        let rlist = (opcode & 0xFFFF);
        let rn = ((opcode >> 16) & 0xF) as u8;
        let load = (opcode & 0x100000) != 0;
        let write_back = (opcode & 0x200000) != 0;
        let psr = (opcode & 0x400000) != 0;
        let up = (opcode & 0x800000) != 0;
        let pre = (opcode & 0x1000000) != 0;

        let mut ptr = self.read_register(rn);

        debug!(
            "rn: {}, load: {}, wb: {}, psr: {}, up: {}, pre: {}, rlist: {:016b}, ptr: {:08X}",
            rn, load, write_back, psr, up, pre, rlist, ptr
        );

        if load {
            todo!("Load");
        }

        if psr && (rlist & 0x8000) != 0 {
            todo!("R15 set and S bit set");
        }

        for i in 0..16 {
            if (rlist & (1 << i)) != 0 {
                if pre {
                    match up {
                        false => ptr.wrapping_sub(1),
                        true => ptr.wrapping_add(1),
                    };
                }

                self.write_u32(true, ptr, self.read_register(i));

                if !pre {
                    match up {
                        false => ptr.wrapping_sub(1),
                        true => ptr.wrapping_add(1),
                    };
                }

                if write_back {
                    self.write_register(rn, ptr);
                }
            }
        }

        self.step_program_counter(4);
    }

    pub fn opcode_match(opcode: u32, mask_clr: u32, mask_set: u32) -> bool {
        (opcode & mask_set == mask_set) && ((!opcode) & mask_clr == mask_clr)
    }

    fn execute_arm(&mut self, opcode: u32) {
        let instr = ((opcode >> 20) & 0xFF) as u8;
        let cond = ((opcode >> 28) & 0xF) as u8;

        // Check conditional
        if !self.should_execute(cond) {
            info!("Skipped execution");
            self.step_program_counter(4);
            return;
        }

        match instr {
            0x00..=0x3F => {
                if Self::opcode_match(opcode, ARM_MASK_MUL_CLR, ARM_MASK_MUL_SET) {
                    todo!("Multiply");
                } else if Self::opcode_match(opcode, ARM_MASK_MUL_LONG_CLR, ARM_MASK_MUL_LONG_SET) {
                    todo!("Multiply Long");
                } else if Self::opcode_match(opcode, ARM_MASK_SNGL_SWP_CLR, ARM_MASK_SNGL_SWP_SET) {
                    todo!("Single Data Swap");
                } else if Self::opcode_match(opcode, ARM_MASK_BX_CLR, ARM_MASK_BX_SET) {
                    self.arm_branch_exchange(opcode);
                } else if Self::opcode_match(opcode, ARM_MASK_HW_REG_CLR, ARM_MASK_HW_REG_SET) {
                    todo!("Halfword Data Transfer: register offset");
                } else if Self::opcode_match(opcode, ARM_MASK_MRS_CLR, ARM_MASK_MRS_SET) {
                    todo!("MRS");
                } else if Self::opcode_match(opcode, ARM_MASK_MSR_CLR, ARM_MASK_MSR_SET) {
                    self.arm_msr(opcode);
                } else if Self::opcode_match(opcode, ARM_MASK_MSR_BITS_CLR, ARM_MASK_MSR_BITS_SET) {
                    todo!("MSR bits");
                } else {
                    self.arm_data_processing(opcode);
                }
            }
            0x40..=0x7F => {
                if Self::opcode_match(opcode, ARM_MASK_UNDEF_CLR, ARM_MASK_UNDEF_SET) {
                    todo!("Undefined instruction");
                } else {
                    self.arm_single_data_transfer(opcode);
                }
            }
            0x80..=0x9F => self.arm_block_data_transfer(opcode),
            0xA0..=0xBF => self.arm_branch(opcode),
            0xC0..=0xDF => todo!("Coprocessor data transfer"),
            0xE0..=0xEF => {
                if (opcode & 0x10) == 0 {
                    todo!("Coprocessor data operation");
                } else {
                    todo!("Coprocessor register transfer");
                }
            }
            0xF0..=0xFF => todo!("Software Interrupt"),
            _ => {
                error!(
                    "[ ARM ] Unknown opcode: {:07X}h ({:024b}b)",
                    opcode & 0x0FFFFFFF,
                    opcode & 0x0FFFFFFF
                );
                self.panic = true;
            }
        }
    }

    pub fn execute(&mut self, opcode: u32) {
        if self.is_thumb() {
            self.execute_thumb(opcode as u16);
        } else {
            self.execute_arm(opcode);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opcode_match_multiply() {
        let opcode: u32 = 0b00000000001100001111000010010001;
        assert!(CPU::opcode_match(
            opcode,
            ARM_MASK_MUL_CLR,
            ARM_MASK_MUL_SET
        ));

        assert!(!CPU::opcode_match(
            opcode,
            ARM_MASK_MUL_LONG_CLR,
            ARM_MASK_MUL_LONG_SET
        ));
    }

    #[test]
    fn arm_mov_imm() {
        let mut cpu = CPU::new();
        cpu.set_thumb(false);

        // MOV R0,#0x12
        let opcode: u32 = 0b1110_00_1_1101_0_0000_0000_000000010010;
        cpu.execute(opcode);

        assert!(cpu.read_register(0) == 0x12);
    }
}
