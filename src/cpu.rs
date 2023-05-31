use std::sync::{Arc, Mutex};

use log::*;

use crate::{keypad::Keypad, lcd::LCD, serial::Serial};

const ROM_WRITING: bool = false;
const INTERNAL_PANIC: bool = false;

pub trait MMU {
    fn read_u8(&mut self, intern: bool, addr: u32) -> u8;
    fn read_u16(&mut self, intern: bool, addr: u32) -> u16;
    fn read_u32(&mut self, intern: bool, addr: u32) -> u32;
    fn write_u8(&mut self, intern: bool, addr: u32, val: u8);
    //fn write_u16(&mut self, intern: bool, addr: u32, val: u16);
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
    pub ram_palette: Arc<Mutex<Vec<u8>>>,
    pub ram_video: Arc<Mutex<Vec<u8>>>,
    pub ram_obj_attr: Arc<Mutex<Vec<u8>>>,
    pub panic: bool,
    pub rom: Vec<u8>,
    pub bios: Vec<u8>,
    pub mem_ptr: u32,

    // IO Registers
    pub dma: [u8; 4 * 3 * 4],
    pub timers: [u8; 2 * 2 * 4],
    pub io_waitcnt: u16,

    // IO -- Interrupt Control
    pub io_ime: u8,
    pub io_ie: u16,
    pub io_if: u16,

    // IO -- Keypad input
    pub keypad: Keypad,
    pub lcd: LCD,
    pub serial: Serial,

    pub halt: bool,
    pub io_bios_if: u16,

    pub cycle_count: usize,
}

impl MMU for CPU {
    fn read_u8(&mut self, intern: bool, addr: u32) -> u8 {
        let addr = addr & 0x0FFFFFFF;

        if intern {
            self.mem_ptr = addr;

            if addr < 0x00003FFF {
                panic!("In bios `{:08X}`", addr);
            }
        }

        let offset = (addr & 0x00FFFFFF) as usize;

        let addr = addr as usize;
        match addr {
            0x00000000..=0x00003FFF => self.bios[addr],
            0x02000000..=0x0203FFFF => self.ram_work1[offset],
            0x03000000..=0x03007FFF => self.ram_work2[offset],
            0x03FFFF00..=0x03FFFFFF => {
                let offset = (offset & 0x000000FF) | 0x00007F00;
                self.ram_work2[offset]
            }
            0x04000000..=0x040003FE => {
                if intern {
                    warn!("Read8 from IO register `{:08X}`", addr);
                }

                let io_addr = addr & 0x3FF;

                match io_addr {
                    0x00..=0x56 => self.lcd.registers[io_addr],
                    // Sound => 0x60..=0xA7
                    0x60..=0xA7 => {
                        if intern {
                            warn!("Read8 from Sound IO `{:08X}`", addr);
                        }
                        0
                    }
                    0xB0..=0xDE => {
                        let offset = (io_addr - 0xB0) as usize;
                        self.dma[offset]
                    }
                    0x134 => (self.serial.rcnt & 0xFF) as u8,
                    0x135 => ((self.serial.rcnt >> 8) & 0xFF) as u8,
                    _ => {
                        if intern {
                            panic!(
                                "Read8 from unimplemented IO register `{:08X}` (PC={:08X})",
                                addr,
                                self.get_program_counter()
                            )
                        }
                        0
                    }
                }
            }
            0x05000000..=0x050003FF => self.ram_palette.lock().unwrap()[offset],
            0x06000000..=0x06017FFF => self.ram_video.lock().unwrap()[offset],
            0x07000000..=0x070003FF => self.ram_obj_attr.lock().unwrap()[offset],
            0x08000000..=0x09FFFFFC | 0x0A000000..=0x0BFFFFFC | 0x0C000000..=0x0DFFFFFC => {
                self.rom[offset]
            }
            _ => {
                if INTERNAL_PANIC {
                    error!(
                        "[0x{:08X}] Panicked! Address out of range `{:08X}`",
                        self.get_program_counter(),
                        addr
                    );
                    self.panic = true;
                } else {
                    panic!(
                        "[0x{:08X}] Panicked! Address out of range `{:08X}`",
                        self.get_program_counter(),
                        addr
                    );
                }
                0
            }
        }
    }

    fn read_u16(&mut self, intern: bool, addr: u32) -> u16 {
        let addr = addr & 0x0FFFFFFF;

        if intern {
            self.mem_ptr = addr;

            if addr < 0x00003FFF {
                panic!("In bios `{:08X}`", addr);
            }
        }

        let offset = (addr & 0x00FFFFFF) as usize;

        match addr {
            0x00000000..=0x00003FFF => {
                //((self.bios[addr + 3] as u32) << 24)
                //    | ((self.bios[addr + 2] as u32) << 16)
                //    | ((self.bios[addr + 1] as u32) << 8)
                //    | (self.bios[addr] as u32)
                0
            }
            0x02000000..=0x0203FFFF => {
                ((self.ram_work1[offset + 1] as u16) << 8) | (self.ram_work1[offset] as u16)
            }
            0x03000000..=0x03007FFF => {
                ((self.ram_work2[offset + 1] as u16) << 8) | (self.ram_work2[offset] as u16)
            }
            0x04000000..=0x040003FE => {
                if intern {
                    warn!("Read16 from IO register `{:08X}`", addr);
                }

                let io_addr = addr & 0x3FF;

                match io_addr {
                    0x00 => self.lcd.get_dispcnt(),
                    _ => {
                        if intern {
                            panic!("Read16 from unimplemented IO register `{:08X}`", addr)
                        }
                        0
                    }
                }
            }
            0x05000000..=0x050003FF => {
                let palette = self.ram_palette.lock().unwrap();
                ((palette[offset + 1] as u16) << 8) | (palette[offset] as u16)
            }
            0x06000000..=0x06017FFF => {
                let vram = self.ram_video.lock().unwrap();
                ((vram[offset + 1] as u16) << 8) | (vram[offset] as u16)
            }
            0x07000000..=0x070003FF => {
                let oam = self.ram_obj_attr.lock().unwrap();
                ((oam[offset + 1] as u16) << 8) | (oam[offset] as u16)
            }
            0x08000000..=0x09FFFFFF | 0x0A000000..=0x0BFFFFFF | 0x0C000000..=0x0DFFFFFF => {
                ((self.rom[offset + 1] as u16) << 8) | (self.rom[offset] as u16)
            }
            _ => {
                if INTERNAL_PANIC {
                    error!(
                        "[0x{:08X}] Panicked! Address out of range `{:08X}`",
                        self.get_program_counter(),
                        addr
                    );
                    self.panic = true;
                } else {
                    panic!(
                        "[0x{:08X}] Panicked! Address out of range `{:08X}`",
                        self.get_program_counter(),
                        addr
                    );
                }
                0
            }
        }
    }

    fn read_u32(&mut self, intern: bool, addr: u32) -> u32 {
        let addr = addr & 0x0FFFFFFF;

        if intern {
            self.mem_ptr = addr;

            if addr < 0x00003FFF {
                panic!(
                    "[0x{:08X}] In bios `{:08X}`",
                    self.get_program_counter(),
                    addr
                );
            }
        }

        let offset = (addr & 0x00FFFFFF) as usize;

        match addr {
            0x00000000..=0x00003FFF => {
                ((self.bios[offset + 3] as u32) << 24)
                    | ((self.bios[offset + 2] as u32) << 16)
                    | ((self.bios[offset + 1] as u32) << 8)
                    | (self.bios[offset] as u32)
            }
            0x02000000..=0x0203FFFF => {
                ((self.ram_work1[offset + 3] as u32) << 24)
                    | ((self.ram_work1[offset + 2] as u32) << 16)
                    | ((self.ram_work1[offset + 1] as u32) << 8)
                    | (self.ram_work1[offset] as u32)
            }
            0x03007FF8 => self.io_bios_if as u32,
            0x03000000..=0x03007FFF => {
                ((self.ram_work2[offset + 3] as u32) << 24)
                    | ((self.ram_work2[offset + 2] as u32) << 16)
                    | ((self.ram_work2[offset + 1] as u32) << 8)
                    | (self.ram_work2[offset] as u32)
            }
            0x03FFFF00..=0x03FFFFFF => {
                let offset = (offset & 0x000000FF) | 0x00007F00;
                ((self.ram_work2[offset + 3] as u32) << 24)
                    | ((self.ram_work2[offset + 2] as u32) << 16)
                    | ((self.ram_work2[offset + 1] as u32) << 8)
                    | (self.ram_work2[offset] as u32)
            }
            0x04000000..=0x040003FE => {
                if intern {
                    warn!("Read32 from IO register `{:08X}`", addr);
                }

                let io_addr = addr & 0x3FF;

                match io_addr {
                    0x00..=0x54 => {
                        let offset = io_addr as usize;
                        ((self.lcd.registers[offset + 3] as u32) << 24)
                            | ((self.lcd.registers[offset + 2] as u32) << 16)
                            | ((self.lcd.registers[offset + 1] as u32) << 8)
                            | (self.lcd.registers[offset] as u32)
                    }
                    0x60..=0xA4 => {
                        warn!("Read32 from Sound IO `{:08X}`", addr);
                        0
                    }
                    0xB0..=0xDC => {
                        let offset = (io_addr - 0xB0) as usize;
                        ((self.dma[offset + 3] as u32) << 24)
                            | ((self.dma[offset + 2] as u32) << 16)
                            | ((self.dma[offset + 1] as u32) << 8)
                            | (self.dma[offset] as u32)
                    }
                    0xDE => {
                        let offset = (io_addr - 0xB0) as usize;
                        ((self.dma[offset + 1] as u32) << 8) | (self.dma[offset] as u32)
                    }
                    // Timers
                    0x100..=0x10E => {
                        let offset = (io_addr - 0x100) as usize;
                        ((self.timers[offset + 3] as u32) << 24)
                            | ((self.timers[offset + 2] as u32) << 16)
                            | ((self.timers[offset + 1] as u32) << 8)
                            | (self.timers[offset] as u32)
                    }
                    0x120..=0x12A => self.serial.read_u32(io_addr),
                    // Keypad
                    0x130 => {
                        warn!("Reading from keyinput: {:010b}", self.keypad.keyinput);
                        self.keypad.keyinput as u32
                    }
                    0x132 => self.keypad.keycnt as u32,
                    // Serial (2)
                    0x134..=0x158 => self.serial.read_u32(io_addr),
                    0x200 => {
                        let ie_bytes = self.io_ie.to_le_bytes();
                        let if_bytes = self.io_if.to_le_bytes();
                        ((if_bytes[1] as u32) << 24)
                            | ((if_bytes[0] as u32) << 16)
                            | ((ie_bytes[1] as u32) << 8)
                            | (ie_bytes[0] as u32)
                    }
                    0x202 => self.io_if as u32,
                    0x204 => self.io_waitcnt as u32,
                    0x208 => self.io_ime as u32,
                    _ => {
                        if intern {
                            panic!("Read32 from unimplemented IO register `{:08X}`", addr)
                        }
                        0
                    }
                }
            }
            0x05000000..=0x050003FF => {
                let palette = self.ram_palette.lock().unwrap();
                ((palette[offset + 3] as u32) << 24)
                    | ((palette[offset + 2] as u32) << 16)
                    | ((palette[offset + 1] as u32) << 8)
                    | (palette[offset] as u32)
            }
            0x06000000..=0x06017FFF => {
                let vram = self.ram_video.lock().unwrap();
                ((vram[offset + 3] as u32) << 24)
                    | ((vram[offset + 2] as u32) << 16)
                    | ((vram[offset + 1] as u32) << 8)
                    | (vram[offset] as u32)
            }
            0x07000000..=0x070003FF => {
                let oam = self.ram_obj_attr.lock().unwrap();
                ((oam[offset + 3] as u32) << 24)
                    | ((oam[offset + 2] as u32) << 16)
                    | ((oam[offset + 1] as u32) << 8)
                    | (oam[offset] as u32)
            }
            0x08000000..=0x09FFFFFC | 0x0A000000..=0x0BFFFFFC | 0x0C000000..=0x0DFFFFFC => {
                ((self.rom[offset + 3] as u32) << 24)
                    | ((self.rom[offset + 2] as u32) << 16)
                    | ((self.rom[offset + 1] as u32) << 8)
                    | (self.rom[offset] as u32)
            }
            _ => {
                if INTERNAL_PANIC {
                    error!(
                        "[0x{:08X}] Panicked! Address out of range `{:08X}`",
                        self.get_program_counter(),
                        addr
                    );
                    self.panic = true;
                } else {
                    panic!(
                        "[0x{:08X}] Panicked! Address out of range `{:08X}`",
                        self.get_program_counter(),
                        addr
                    );
                }
                0
            }
        }
    }

    fn write_u8(&mut self, intern: bool, addr: u32, val: u8) {
        let addr = addr & 0x0FFFFFFF;

        if intern {
            self.mem_ptr = addr;
        }

        let offset = (addr & 0x00FFFFFF) as usize;

        let addr = addr as usize;
        match addr {
            0x00000000..=0x00003FFF => {
                error!(
                    "Panicked! Cannot write8 to BIOS (`{:08X} => {:02X}`)",
                    addr, val
                );
                self.panic = true
            } //self.bios[addr] = val,
            0x02000000..=0x0203FFFF => self.ram_work1[offset] = val,
            0x03000000..=0x03007FFF => self.ram_work2[offset] = val,
            0x04000000..=0x040003FE => {
                warn!("Write8 to IO register `{:08X} = {:02X}`", addr, val);

                let io_addr = addr & 0x3FF;

                match io_addr {
                    0x00..=0x55 => self.lcd.registers[io_addr] = val,
                    0x60..=0xA7 => {
                        warn!("Write8 to Sound IO `{:08X}` => {:02X}", addr, val);
                    }
                    0xB0..=0xDE => {
                        let offset = (io_addr - 0xB0) as usize;
                        self.dma[offset] = val;
                    }
                    0x120..=0x12B => self.serial.write_u8(io_addr, val),
                    0x134..=0x159 => self.serial.write_u8(io_addr, val),
                    0x208 => self.io_ime = val,
                    _ => {
                        if intern {
                            panic!(
                                "Write8 to unimplemented IO register `{:08X}` => {:02X}",
                                addr, val
                            )
                        }
                    }
                }
            }
            0x05000000..=0x050003FF => self.ram_palette.lock().unwrap()[offset] = val,
            0x06000000..=0x06017FFF => self.ram_video.lock().unwrap()[offset] = val,
            0x07000000..=0x070003FF => self.ram_obj_attr.lock().unwrap()[offset] = val,
            0x08000000..=0x09FFFFFC | 0x0A000000..=0x0BFFFFFC | 0x0C000000..=0x0DFFFFFC => {
                if ROM_WRITING {
                    warn!("Write8 to ROM `{:08X} => {:02X}`", addr, val);
                    self.rom[offset] = val;
                } else {
                    error!("Panicked! Cannot Write8 to ROM @ `{:08X}`", addr);
                    self.panic = true;
                }
            }
            _ => {
                if INTERNAL_PANIC {
                    error!(
                        "[0x{:08X}] Panicked! Write8 Address out of range `{:08X}`",
                        self.get_program_counter(),
                        addr
                    );
                    self.panic = true;
                } else {
                    panic!(
                        "[0x{:08X}] Panicked! Write8 Address out of range `{:08X}`",
                        self.get_program_counter(),
                        addr
                    );
                }
            }
        }
    }

    fn write_u32(&mut self, intern: bool, addr: u32, val: u32) {
        let addr = addr & 0x0FFFFFFF;

        if intern {
            self.mem_ptr = addr;

            if addr < 0x00003FFF {
                panic!("In bios `{:08X}`", addr);
            }
        }

        let offset = (addr & 0x00FFFFFF) as usize;
        let b3 = ((val >> 24) & 0xFF) as u8;
        let b2 = ((val >> 16) & 0xFF) as u8;
        let b1 = ((val >> 8) & 0xFF) as u8;
        let b0 = (val & 0xFF) as u8;

        match addr {
            0x02000000..=0x0203FFFF => {
                self.ram_work1[offset + 3] = b3;
                self.ram_work1[offset + 2] = b2;
                self.ram_work1[offset + 1] = b1;
                self.ram_work1[offset] = b0;
            }
            0x03007FF8 => {
                let mask = (val & 0xFFFF) as u16;
                self.io_bios_if &= !mask;
                if intern {
                    warn!("Clearing flags={mask:04X} in BIOS_IF");
                }
            }
            0x03000000..=0x03007FFF => {
                self.ram_work2[offset + 3] = b3;
                self.ram_work2[offset + 2] = b2;
                self.ram_work2[offset + 1] = b1;
                self.ram_work2[offset] = b0;
            }
            0x03FFFF00..=0x03FFFFFF => {
                let offset = (offset & 0x000000FF) | 0x00007F00;
                self.ram_work2[offset + 3] = b3;
                self.ram_work2[offset + 2] = b2;
                self.ram_work2[offset + 1] = b1;
                self.ram_work2[offset] = b0;
            }
            0x04000000..=0x040003FE => {
                if intern {
                    warn!("Write32 to IO register `{:08X}` => {:08X}", addr, val)
                }

                let io_addr = addr & 0x3FF;

                match io_addr {
                    0x00..=0x54 => {
                        info!(
                            "Write to LCD register `{:X}` at PC={:08X}",
                            offset,
                            self.get_program_counter()
                        );
                        self.lcd.registers[offset + 3] = ((val >> 24) & 0xFF) as u8;
                        self.lcd.registers[offset + 2] = ((val >> 16) & 0xFF) as u8;
                        self.lcd.registers[offset + 1] = ((val >> 8) & 0xFF) as u8;
                        self.lcd.registers[offset] = (val & 0xFF) as u8;
                    }
                    0x60..=0xA4 => {
                        warn!("Write32 to Sound IO `{:08X}` => {:08X}", addr, val);
                    }
                    0xB0..=0xDC => {
                        let offset = (io_addr - 0xB0) as usize;
                        self.dma[offset + 3] = ((val >> 24) & 0xFF) as u8;
                        self.dma[offset + 2] = ((val >> 16) & 0xFF) as u8;
                        self.dma[offset + 1] = ((val >> 8) & 0xFF) as u8;
                        self.dma[offset + 0] = ((val >> 0) & 0xFF) as u8;
                    }
                    0xDE => {
                        let offset = (io_addr - 0xB0) as usize;
                        self.dma[offset + 1] = ((val >> 8) & 0xFF) as u8;
                        self.dma[offset + 0] = ((val >> 0) & 0xFF) as u8;
                    }
                    // Timers
                    0x100..=0x10E => {
                        let offset = (io_addr - 0x100) as usize;
                        self.timers[offset + 3] = b3;
                        self.timers[offset + 2] = b2;
                        self.timers[offset + 1] = b1;
                        self.timers[offset] = b0;
                    }
                    // Serial (2)
                    0x120..=0x12A => self.serial.write_u32(io_addr, val),
                    0x134..=0x158 => self.serial.write_u32(io_addr, val),
                    0x200 => {
                        self.io_ie = (val & 0xFFFF) as u16;
                        warn!(
                            "Write32 to Interrupt Enable Register `{:08X}` => {:08X}",
                            addr, val
                        );
                    }
                    0x202 => {
                        let val = (val & 0xFFFF) as u16;
                        self.io_if &= !(val);
                        warn!(
                            "Write32 to Interrupt Request Flags Register `{:08X}` => {:08X}",
                            addr, val
                        );
                    }
                    0x204 => {
                        self.io_waitcnt = (val & 0xFFFF) as u16;
                        warn!(
                            "Write32 to GamePak Waitstate Control `{:08X}` => {:08X}",
                            addr, val
                        );
                    }
                    0x208 => {
                        self.io_ime = (val & 0xFF) as u8;
                        warn!(
                            "Write32 to Interrupt Master Enable Register `{:08X}` => {:08X}",
                            addr, val
                        );
                    }
                    _ => {
                        if intern {
                            panic!(
                                "Write32 to unimplemented IO register `{:08X}` => {:08X}",
                                addr, val
                            )
                        }
                    }
                }
            }
            0x05000000..=0x050003FF => {
                let mut palette = self.ram_palette.lock().unwrap();
                palette[offset + 3] = ((val >> 24) & 0xFF) as u8;
                palette[offset + 2] = ((val >> 16) & 0xFF) as u8;
                palette[offset + 1] = ((val >> 8) & 0xFF) as u8;
                palette[offset] = (val & 0xFF) as u8;
            }
            0x06000000..=0x06017FFF => {
                let mut vram = self.ram_video.lock().unwrap();
                vram[offset + 3] = b3;
                vram[offset + 2] = b2;
                vram[offset + 1] = b1;
                vram[offset] = b0;
            }
            0x07000000..=0x070003FF => {
                let mut oam = self.ram_obj_attr.lock().unwrap();
                oam[offset + 3] = ((val >> 24) & 0xFF) as u8;
                oam[offset + 2] = ((val >> 16) & 0xFF) as u8;
                oam[offset + 1] = ((val >> 8) & 0xFF) as u8;
                oam[offset] = (val & 0xFF) as u8;
            }
            0x08000000..=0x09FFFFFC | 0x0A000000..=0x0BFFFFFC | 0x0C000000..=0x0DFFFFFC => {
                if ROM_WRITING {
                    warn!("Write32 to ROM `{:08X} => {:08X}`", addr, val);
                    self.rom[offset + 3] = ((val >> 24) & 0xFF) as u8;
                    self.rom[offset + 2] = ((val >> 16) & 0xFF) as u8;
                    self.rom[offset + 1] = ((val >> 8) & 0xFF) as u8;
                    self.rom[offset] = (val & 0xFF) as u8;
                } else {
                    error!(
                        "Panicked! Cannot write32 to ROM (`{:08X} => {:08X}`)",
                        addr, val
                    );
                    self.panic = true;
                }
            }
            _ => {
                if INTERNAL_PANIC {
                    error!(
                        "[0x{:08X}] Panicked! Write32 Address out of range `{:08X}`",
                        self.get_program_counter(),
                        addr
                    );
                    self.panic = true;
                } else {
                    panic!(
                        "[0x{:08X}] Panicked! Write32 Address out of range `{:08X}`",
                        self.get_program_counter(),
                        addr
                    );
                }
            }
        }
    }

    fn addr_valid(&self, addr: u32) -> bool {
        let addr = addr & 0x0FFFFFFF;

        match addr {
            (0x00000000..=0x00003FFF)
            | (0x02000000..=0x0203FFFF)
            | (0x03000000..=0x03007FFF)
            | (0x03FFFF00..=0x03FFFFFF)
            | (0x04000000..=0x040003FE)
            | (0x05000000..=0x050003FF)
            | (0x06000000..=0x06017FFF)
            | (0x07000000..=0x070003FF)
            | (0x08000000..=0x09FFFFFC)
            | (0x0A000000..=0x0BFFFFFC)
            | (0x0C000000..=0x0DFFFFFC) => true,
            _ => false,
        }
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
pub const MODE_IRQ: u8 = 0x2;
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

pub const IRQ_VBLANK: u16 = 1 << 0;
pub const IRQ_HBLANK: u16 = 1 << 1;
pub const IRQ_VCOUNT: u16 = 1 << 2;
pub const IRQ_TIM0: u16 = 1 << 3;
pub const IRQ_TIM1: u16 = 1 << 4;
pub const IRQ_TIM2: u16 = 1 << 5;
pub const IRQ_TIM3: u16 = 1 << 6;
pub const IRQ_SERIAL: u16 = 1 << 7;
pub const IRQ_DMA0: u16 = 1 << 8;
pub const IRQ_DMA1: u16 = 1 << 9;
pub const IRQ_DMA2: u16 = 1 << 10;
pub const IRQ_DMA3: u16 = 1 << 11;
pub const IRQ_KEYPAD: u16 = 1 << 12;
pub const IRQ_GAMEPAK: u16 = 1 << 13;
pub const IRQ_DEBUG1: u16 = 1 << 14;
pub const IRQ_DEBUG2: u16 = 1 << 15;

impl CPU {
    pub fn new(
        vram: &Arc<Mutex<Vec<u8>>>,
        palette: &Arc<Mutex<Vec<u8>>>,
        oam: &Arc<Mutex<Vec<u8>>>,
    ) -> Self {
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
            ram_palette: Arc::clone(palette),
            ram_video: Arc::clone(vram),
            ram_obj_attr: Arc::clone(oam),
            panic: false,
            rom: Vec::new(),
            bios: Vec::new(),
            dma: [0; 4 * 3 * 4],
            timers: [0; 2 * 2 * 4],
            io_waitcnt: 0,
            io_ie: 0,
            io_ime: 0,
            io_if: 0,
            keypad: Keypad::new(),
            lcd: LCD::new(),
            serial: Serial::new(),
            halt: false,
            io_bios_if: 0,
            cycle_count: 0,
        }
    }

    pub fn load_rom(&mut self, rom: &Vec<u8>) {
        self.rom = rom.to_vec();
    }

    pub fn load_bios(&mut self, bios: &Vec<u8>) {
        self.bios = bios.to_vec();
    }

    pub fn get_flag_n(&self) -> bool {
        self.reg_cpsr & FLAG_MASK_N > 0
    }

    /// Returns Mode (FIQ, IRQ etc)
    pub fn get_mode(&self) -> u8 {
        (self.reg_cpsr & 0xF) as u8
    }

    fn set_mode(&mut self, mode: u8) {
        self.reg_cpsr = (self.reg_cpsr & 0xFFFFFFE0) | 0x10 | (mode as u32);
    }

    fn set_flag_n(&mut self, set: bool) {
        match set {
            true => self.reg_cpsr |= FLAG_MASK_N,
            false => self.reg_cpsr &= !(FLAG_MASK_N),
        }
    }

    pub fn get_flag_z(&self) -> bool {
        self.reg_cpsr & FLAG_MASK_Z > 0
    }

    fn set_flag_z(&mut self, set: bool) {
        match set {
            true => self.reg_cpsr |= FLAG_MASK_Z,
            false => self.reg_cpsr &= !(FLAG_MASK_Z),
        }
    }

    pub fn get_flag_c(&self) -> bool {
        self.reg_cpsr & FLAG_MASK_C > 0
    }

    fn set_flag_c(&mut self, set: bool) {
        match set {
            true => self.reg_cpsr |= FLAG_MASK_C,
            false => self.reg_cpsr &= !(FLAG_MASK_C),
        }
    }

    pub fn get_flag_v(&self) -> bool {
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
        self.dma = [0; 48];

        self.set_program_counter(0x08000000);

        // Setup stack pointers
        self.regs_svc[0] = 0x03007FE0;
        self.regs_irq[0] = 0x03007FA0;
        self.registers[13] = 0x03007F00;

        // Clear panic flag
        self.panic = false;
        self.halt = false;

        // Clear cycle counter
        self.cycle_count = 0;
    }

    pub fn trigger_irq(&mut self, irq: u16) {
        self.io_if |= irq;
        self.halt = false;

        // Store current CPSR into SPSR[current_mode], store SPSR[IRQ] in CPSR
        self.regs_spsr[MODE_IRQ as usize] = self.reg_cpsr;
        warn!("IRQ: Stored CPSR `{:08X}` in SPSR[IRQ]", self.reg_cpsr);

        // Set IRQ mode, disable IRQs, Clear Thumb
        self.reg_cpsr = 0x80 | 0x10 | (MODE_IRQ as u32);
        warn!("IRQ: Set CPSR to `{:08X}`", self.reg_cpsr);

        self.write_register(14, self.get_program_counter() + 4);
        warn!("IRQ: Store return in LR `{:08X}`", self.read_register(14));
        self.set_program_counter(0x18);
        warn!("IRQ: Jump to 0x18");

        // IRQ Handler in BIOS:
        // 00000018  b      128h                ;IRQ vector: jump to actual BIOS handler
        // 00000128  stmfd  r13!,r0-r3,r12,r14  ;save registers to SP_irq
        // 0000012C  mov    r0,4000000h         ;ptr+4 to 03FFFFFC (mirror of 03007FFC)
        // 00000130  add    r14,r15,0h          ;retadr for USER handler $+8=138h
        // 00000134  ldr    r15,[r0,-4h]        ;jump to [03FFFFFC] USER handler
        // 00000138  ldmfd  r13!,r0-r3,r12,r14  ;restore registers from SP_irq
        // 0000013C  subs   r15,r14,4h          ;return from IRQ (PC=LR-4, CPSR=SPSR)
    }

    pub fn can_irq_trigger(&mut self, irq: u16) -> bool {
        let ime_enable = (self.io_ime & 0x1) == 0x1;
        let ie_enable = (self.io_ie & irq) == irq;
        let if_enable = (self.io_if & irq) == irq;
        let irq_enable = (self.reg_cpsr & 0x80) != 0x80;

        let bios_if_enable = (self.io_bios_if & irq) == irq;

        if !ie_enable || (!self.halt && (!irq_enable || !ime_enable)) {
            return false;
        }

        if self.halt && (self.io_bios_if != 0 && !bios_if_enable) {
            warn!("Halted (with BIOS IF), but IRQ `{irq:04X}` not in BIOS_IF");
            return false;
        }

        match irq {
            IRQ_VBLANK => self.lcd.is_vblank_irq_enabled(),
            IRQ_HBLANK => self.lcd.is_hblank_irq_enabled(),
            IRQ_VCOUNT => self.lcd.is_vcount_irq_enabled(),
            IRQ_TIM0 => false,
            IRQ_TIM1 => false,
            IRQ_TIM2 => false,
            IRQ_TIM3 => false,
            IRQ_SERIAL => false,
            IRQ_DMA0 => false,
            IRQ_DMA1 => false,
            IRQ_DMA2 => false,
            IRQ_DMA3 => false,
            IRQ_KEYPAD => self.keypad.is_irq_enabled(),
            IRQ_GAMEPAK => true, // Triggers on cart removal
            IRQ_DEBUG1 => true,
            IRQ_DEBUG2 => true,
            _ => false,
        }
    }

    pub fn read_register(&self, register: u8) -> u32 {
        let mode = self.get_mode();
        match (
            register,
            mode != MODE_SYSTEM && mode != MODE_USER,
            mode == MODE_FIQ,
        ) {
            (0..=7, _, _) | (8..=14, false, _) | (8..=12, true, false) => {
                self.registers[register as usize]
            }
            (15, _, _) => match self.is_thumb() {
                false => self.registers[15] & 0xFFFFFFFC,
                true => self.registers[15] & 0xFFFFFFFE,
            },
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
            (0..=7, _, _) | (8..=14, false, _) | (8..=12, true, false) => {
                self.registers[register as usize] = value
            }
            (15, _, _) => match self.is_thumb() {
                false => self.registers[15] = value & 0xFFFFFFFC,
                true => self.registers[15] = value & 0xFFFFFFFE,
            },
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
        self.write_register(15, addr);
    }

    fn step_program_counter(&mut self, steps: u32) {
        self.registers[15] += steps;
    }

    /// Memcopy `count` bytes from `src` to `dest`.
    /// Does not do bounds checking
    fn memcpy(&mut self, dest: u32, src: u32, count: u32) {
        for i in 0..count {
            let val = self.read_u8(true, src + i);
            self.write_u8(true, dest + i, val);
        }
    }

    /// Memfill `count` words from `val` to `dest`.
    /// Does not do bounds checking
    fn memfill32(&mut self, dest: u32, val: u32, words: u32) {
        for i in 0..words {
            self.write_u32(true, dest + (i * 4), val);
        }
    }

    /// Check DMAs and see if any need to run
    pub fn dma_check(&mut self) -> Option<u8> {
        for i in 0..4 {
            let control = {
                let reg = self.read_u32(false, 0x040000B0 + ((i * 12) + 8));

                (reg >> 16) & 0xFFFF
            };

            let timing = ((control >> 12) & 0x3) as u8;
            let enable = (control & 0x8000) != 0;

            // Skips if not enabled and if SOUND DMA is specified
            if !enable || (timing == 0x03 && (i == 1 || i == 2)) {
                continue;
            }

            match timing {
                0x00 => return Some(i as u8),
                0x01 => todo!("Check VBlank DMA start timing => DMA{}", i),
                0x02 => todo!("Check HBlank DMA start timing => DMA{}", i),
                0x03 => todo!("Check Special DMA start timing => DMA{}", i),
                _ => unreachable!(),
            }
        }

        None
    }

    pub fn dma_run(&mut self, num: u8) {
        let addr_base: u32 = 0x040000B0;
        let reg_offset = (num as u32) * 12;
        let src = self.read_u32(true, addr_base + reg_offset + 0);
        let dest = self.read_u32(true, addr_base + reg_offset + 4);
        let (count, control) = {
            let reg = self.read_u32(true, addr_base + reg_offset + 8);
            let cnt = reg & 0xFFFF;

            let cnt = match (cnt, num) {
                (0, 3) => 0x10000,
                (0, _) => 0x4000,
                (_, _) => cnt,
            };

            (cnt, (reg >> 16) & 0xFFFF)
        };

        let dest_ctrl = ((control >> 5) & 0x3) as u8; // 0=inc, 1=dec, 2=fixed, 3=inc+reload
        let src_ctrl = ((control >> 7) & 0x3) as u8; // 0=inc, 1=dec, 2=fixed, 3=prohib
        let repeat = (control & 0x200) != 0;
        let word = (control & 0x400) != 0;
        let drq = (control & 0x800) != 0;
        let timing = ((control >> 12) & 0x3) as u8;
        let irq = (control & 0x4000) != 0;
        let enable = (control & 0x8000) != 0;

        info!(
            "DMA: Start transfer from `{:08X}` to `{:08X}` with count={:X} (dest_ctrl={}, src_ctrl={}, repeat={}, word={}, drq={}, timing={}, irq={})",
            src, dest, count, dest_ctrl, src_ctrl, repeat, word, drq, timing, irq
        );

        let step: u32 = match word {
            false => 2,
            true => 4,
        };

        let mut src_ptr = src;
        let mut dest_ptr = dest;

        for _ in 0..count {
            // Read/Write to memory
            if word {
                let val = self.read_u32(true, src_ptr);
                self.write_u32(true, dest_ptr, val);
            } else {
                let low = self.read_u8(true, src_ptr);
                let high = self.read_u8(true, src_ptr + 1);

                self.write_u8(true, dest_ptr, low);
                self.write_u8(true, dest_ptr + 1, high);
            }

            // Fix pointers
            dest_ptr = match dest_ctrl {
                0 => dest_ptr + step,
                1 => dest_ptr - step,
                2 => dest_ptr,
                3 => {
                    todo!("Implement DMA dest increment+reload");
                    dest_ptr + step
                }
                _ => unreachable!(),
            };

            src_ptr = match src_ctrl {
                0 => src_ptr + step,
                1 => src_ptr - step,
                2 => src_ptr,
                3 => panic!("DMA Source Addr Control = 3 invalid"),
                _ => unreachable!(),
            };
        }

        if irq {
            todo!("Implement DMA finish IRQ");
        }

        if !repeat {
            // Clear enable bit
            self.dma[(reg_offset as usize) + 11] &= !(0x80);
        }

        info!(
            "DMA: Finish transfer from `{:08X}` to `{:08X}` with count={:X}",
            src, dest, count
        );
    }

    // SWI 0x01
    fn syscall_register_ram_reset(&mut self) {
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
            self.ram_palette.lock().unwrap().fill(0);
        }
        if (flags & 0x8) != 0 {
            self.ram_video.lock().unwrap().fill(0);
        }
        if (flags & 0x10) != 0 {
            self.ram_obj_attr.lock().unwrap().fill(0);
        }
        if (flags & 0x20) != 0 {
            // Clear SIO
        }
        if (flags & 0x40) != 0 {
            // Clear Sound registers
        }
        if (flags & 0x80) != 0 {
            self.lcd.reset();
            // Clear other registers
        }
    }

    // SWI 0x02
    fn syscall_halt(&mut self) {
        self.halt = true;
        warn!("HLE: executing syscall `Halt`");
    }

    // SWI 0x04
    fn syscall_intr_wait(&mut self) {
        let discard = (self.read_register(0) & 0x1) == 0x1;
        let r1 = (self.read_register(1) & 0xFFFF) as u16;

        warn!("HLE: executing syscall `IntrWait` with discard={discard}, irqs={r1:04X}");

        // r0 =>
        //      0=Return immediately if an old flag was already set
        //      1=Discard old flags, wait until a new flag becomes set
        //
        // r1 => Interrupt flag(s) to wait for (same format as IE/IF registers)

        if !discard {
            todo!("Implement IntrWait with discard=0");
        }

        self.io_ime |= 0x1;
        self.io_bios_if = r1;
        self.halt = true;
    }

    // SWI 0x05
    fn syscall_vblank_intr_wait(&mut self) {
        self.write_register(0, 1);
        self.write_register(1, 1);
        self.syscall_intr_wait();
    }

    // SWI 0x06
    fn syscall_div(&mut self) {
        let number = self.read_register(0) as i32;
        let div = self.read_register(1) as i32;

        info!("HLE: executing syscall `Div` with `{number}/{div}`");

        let res_div = number / div;
        let res_mod = number % div;
        let res_abs = res_div.abs();

        self.write_register(0, res_div as u32);
        self.write_register(1, res_mod as u32);
        self.write_register(2, res_abs as u32);
    }

    // SWI 0x07
    fn syscall_div_arm(&mut self) {
        let div = self.read_register(0) as i32;
        let number = self.read_register(1) as i32;

        info!("HLE: executing syscall `DivArm` with `{number}/{div}`");

        let res_div = number / div;
        let res_mod = number % div;
        let res_abs = res_div.abs();

        self.write_register(0, res_div as u32);
        self.write_register(1, res_mod as u32);
        self.write_register(2, res_abs as u32);
    }

    // SWI 0x0B
    fn syscall_cpu_set(&mut self) {
        let rs_val = self.read_register(0);
        let rd_val = self.read_register(1);
        let len_mode = self.read_register(2);

        let count = len_mode & 0x1FFFFF;
        let fill = (len_mode & 0x01000000) != 0;
        let word = (len_mode & 0x04000000) != 0;

        info!(
            "HLE: executing syscall `CpuSet` with `rd={:08X}, rs={:08X}, count={:X}, fill={}, word={}`",
            rd_val, rs_val, count, fill, word
        );

        if fill {
            if !word {
                todo!("Implement CpuSet::fill halfword");
            } else {
                let val = self.read_u32(true, rs_val);
                for i in 0..count {
                    let offset = i * 4;
                    self.write_u32(true, rd_val + offset, val);
                }
            }
        } else {
            if !word {
                todo!("Implement CpuSet::copy halfword");
            } else {
                for i in 0..count {
                    let offset = i * 4;
                    let val = self.read_u32(true, rs_val + offset);
                    self.write_u32(true, rd_val + offset, val);
                }
            }
        }
    }

    // SWI 0x0C
    fn syscall_cpu_fast_set(&mut self) {
        let src_addr = self.read_register(0);
        let dest_addr = self.read_register(1);
        let len_mode = self.read_register(2);

        let count = len_mode & 0x1FFFFF;
        let fill = (len_mode & 0x01000000) != 0;

        info!(
            "HLE: executing syscall `CpuFastSet` with `dest_addr={:08X}, src_addr={:08X}, count={:X}, fill={}`",
            dest_addr, src_addr, count, fill
        );

        if fill {
            if count % 8 != 0 {
                panic!("CpuFastSet: Count should be multiple of 8");
            }

            let val = self.read_u32(true, src_addr);

            self.memfill32(dest_addr, val, count);
        } else {
            todo!("Implement Copy CpuFastSet");
        }
    }

    fn bios_syscall(&mut self, syscall: u8) {
        match syscall {
            0x01 => self.syscall_register_ram_reset(),
            0x02 => self.syscall_halt(),
            0x04 => self.syscall_intr_wait(),
            0x05 => self.syscall_vblank_intr_wait(),
            0x06 => self.syscall_div(),
            0x07 => self.syscall_div_arm(),
            0x0B => self.syscall_cpu_set(),
            0x0C => self.syscall_cpu_fast_set(),
            _ => panic!("Unknown BIOS syscall `{:02X}h`", syscall),
        }
    }

    /// Format1
    fn thumb_move_shifted_register(&mut self, opcode: u16) {
        let rd = (opcode & 0x7) as u8;
        let rs = ((opcode >> 3) & 0x7) as u8;
        let rs_val = self.read_register(rs);

        let offset = ((opcode >> 6) & 0x1F) as u8;
        let op = ((opcode >> 11) & 0x3) as u8;

        let (res, carry) = match op {
            0x0 => {
                info!(
                    "[0x{:08X}] => execute: `LSL R{},R{},#{}`, #{}",
                    self.registers[15], rd, rs, offset, rs_val
                );
                let carry = ((rs_val >> (32 - offset)) & 0x1) != 0;
                (rs_val << offset, carry)
            }
            0x1 => {
                info!(
                    "[0x{:08X}] => execute: `LSR R{},R{},#{}`",
                    self.registers[15], rd, rs, offset
                );
                let carry = match offset {
                    0 => false,
                    _ => ((rs_val >> (offset - 1)) & 0x1) != 0,
                };
                (rs_val >> offset, carry)
            }
            0x2 => {
                info!(
                    "[0x{:08X}] => execute: `ASR R{},R{},#{}`",
                    self.registers[15], rd, rs, offset
                );
                let rs_signed = rs_val as i32;
                let carry = match offset {
                    0 => false,
                    _ => ((rs_val >> (offset - 1)) & 0x1) != 0,
                };
                ((rs_signed >> offset) as u32, carry)
            }
            _ => {
                error!("Invalid opcode for move_shifted_register {}", op);
                self.panic = true;
                (0, true)
            }
        };

        self.write_register(rd, res);

        self.set_flag_n((res & 0x80000000) != 0);
        self.set_flag_z(res == 0);
        if offset != 0 {
            self.set_flag_c(carry);
            //self.set_flag_v((rs_val & 0x80000000) != (res & 0x80000000));
        }

        self.step_program_counter(2);
        self.cycle_count += 1 + 1;
    }

    /// Format2
    fn thumb_add_subtract(&mut self, opcode: u16) {
        let rd = (opcode & 0x7) as u8;
        let rs = ((opcode >> 3) & 0x7) as u8;
        let rs_val = self.read_register(rs);
        let offset = ((opcode >> 6) & 0x7) as u8;
        let sub = (opcode & 0x200) != 0;
        let imm = (opcode & 0x400) != 0;

        let (result, n, z, c, v) = match (sub, imm) {
            (false, false) => {
                info!(
                    "[0x{:08X}] => execute: `ADD R{},R{},R{}`",
                    self.registers[15], rd, rs, offset
                );
                self.alu(ALU_ADD, rs_val, self.read_register(offset))
            }
            (false, true) => {
                info!(
                    "[0x{:08X}] => execute: `ADD R{},R{},#{}`",
                    self.registers[15], rd, rs, offset
                );
                self.alu(ALU_ADD, rs_val, offset as u32)
            }
            (true, false) => {
                info!(
                    "[0x{:08X}] => execute: `SUB R{},R{},R{}`",
                    self.registers[15], rd, rs, offset
                );
                self.alu(ALU_SUB, rs_val, self.read_register(offset))
            }
            (true, true) => {
                info!(
                    "[0x{:08X}] => execute: `SUB R{},R{},#{}`",
                    self.registers[15], rd, rs, offset
                );
                self.alu(ALU_SUB, rs_val, offset as u32)
            }
        };

        self.write_register(rd, result);
        self.set_flag_n(n);
        self.set_flag_z(z);
        self.set_flag_c(c);
        self.set_flag_v(v);

        self.step_program_counter(2);

        // ALU + reg offset, but no shift (immediate of 0)
        self.cycle_count += 1;
    }

    /// Format3
    fn thumb_mov_cmp_add_sub_imm(&mut self, opcode: u16) {
        let offset = (opcode & 0xFF) as u32;
        let rd = ((opcode >> 8) & 0x7) as u8;
        let op = ((opcode >> 11) & 0x3) as u8;
        let rd_val = self.read_register(rd);

        let (result, n, z, c, v) = match op {
            0b00 => {
                info!(
                    "[0x{:08X}] => execute: `MOV R{},#0x{:02X}`",
                    self.registers[15], rd, offset
                );
                self.alu(ALU_MOV, 0, offset)
            }
            0b01 => {
                info!(
                    "[0x{:08X}] => execute: `CMP R{},#0x{:02X}`",
                    self.registers[15], rd, offset
                );
                self.alu(ALU_CMP, rd_val, offset)
            }
            0b10 => {
                info!(
                    "[0x{:08X}] => execute: `ADD R{},#0x{:02X}`",
                    self.registers[15], rd, offset
                );
                self.alu(ALU_ADD, rd_val, offset)
            }
            0b11 => {
                info!(
                    "[0x{:08X}] => execute: `SUB R{},#0x{:02X}`",
                    self.registers[15], rd, offset
                );
                self.alu(ALU_SUB, rd_val, offset)
            }
            _ => unreachable!(""),
        };

        self.set_flag_n(n);
        self.set_flag_z(z);
        self.set_flag_c(c);
        self.set_flag_v(v);

        if op != 0b01 {
            self.write_register(rd, result);
        }

        self.step_program_counter(2);
        self.cycle_count += 1;
    }

    /// Format4
    fn thumb_alu(&mut self, opcode: u16) {
        let rd = (opcode & 0x7) as u8;
        let rd_val = self.read_register(rd);
        let rs = ((opcode >> 3) & 0x7) as u8;
        let rs_val = self.read_register(rs);

        let op = ((opcode >> 6) & 0xF) as u8;

        let (result, n, z, c, v) = match op {
            0x2 => {
                let (res, v) = rd_val.overflowing_shl(rs_val);
                let carry = match rs_val {
                    0 => false,
                    _ => (rd_val & (1 << (32 - rs_val))) != 0,
                };

                (res, (res & 0x80000000) != 0, res == 0, carry, v)
            } // LSL
            0x3 => {
                info!(
                    "[0x{:08X}] => execute: `LSR R{},R{}`",
                    self.registers[15], rd, rs
                );
                let op2 = 0b0000_0_01_1_0000 | ((rs as u16) << 8) | (rd as u16);
                let (rot_val, _) = self.alu_operand2_calc(false, op2);

                self.alu(ALU_MOV, rd_val, rot_val)
            }
            0x4 => {
                info!(
                    "[0x{:08X}] => execute: `ASR R{},R{}`",
                    self.registers[15], rd, rs
                );
                let op2 = 0b0000_0_10_1_0000 | ((rs as u16) << 8) | (rd as u16);
                let (rot_val, _) = self.alu_operand2_calc(false, op2);

                self.alu(ALU_MOV, rd_val, rot_val)
            }
            0x7 => {
                info!(
                    "[0x{:08X}] => execute: `ROR R{},R{}`",
                    self.registers[15], rd, rs
                );
                let op2 = 0b0000_0_11_1_0000 | ((rs as u16) << 8) | (rd as u16);
                let (rot_val, _) = self.alu_operand2_calc(false, op2);

                self.alu(ALU_MOV, rd_val, rot_val)
            }
            0x9 => {
                info!(
                    "[0x{:08X}] => execute: `NEG R{},R{}`",
                    self.registers[15], rd, rs
                );
                self.alu(ALU_RSB, rs_val, 0)
            } // NEG
            ALU_TST => self.alu_tst(rd_val, rs_val),
            0xD => {
                info!(
                    "[0x{:08X}] => execute: `MUL R{},R{}`",
                    self.registers[15], rd, rs
                );
                let res = rs_val.wrapping_mul(rd_val);
                (
                    res,
                    (res & 0x80000000) != 0,
                    res == 0,
                    self.get_flag_c(),
                    self.get_flag_v(),
                )
            } // MUL,
            _ => self.alu(op, rd_val, rs_val),
        };

        self.set_flag_n(n);
        self.set_flag_z(z);
        self.set_flag_c(c);
        self.set_flag_v(v);

        if op != ALU_TST && op != ALU_CMP && op != ALU_CMN {
            self.write_register(rd, result);
        }

        self.step_program_counter(2);
        self.cycle_count += match rd == 15 {
            false => 1,
            true => 1 + 2,
        };
    }

    /// Format5
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

        let cycles = match op {
            0x0 => {
                info!(
                    "[0x{:08X}] => execute: `ADD R{},R{}`",
                    self.registers[15], rd, rs
                );
                self.write_register(rd, rd_val.wrapping_add(rs_val));
                0
            }
            0x1 => {
                info!(
                    "[0x{:08X}] => execute: `CMP R{},R{}",
                    self.registers[15], rd, rs
                );

                let (_, n, z, c, v) = self.alu(ALU_CMP, rd_val, rs_val);
                self.set_flag_n(n);
                self.set_flag_n(z);
                self.set_flag_n(c);
                self.set_flag_n(v);
                0
            }
            0x2 => {
                info!(
                    "[0x{:08X}] => execute: `MOV R{},R{}",
                    self.registers[15], rd, rs
                );
                self.write_register(rd, rs_val);
                0
            }
            0x3 => {
                // TODO: Refactor
                let thumb = (rs_val & 0x1) == 0x1;

                info!(
                    "[0x{:08X}] => execute: `BX R{}` => thumb={}",
                    self.registers[15], rs, thumb
                );

                self.set_thumb(thumb);
                match thumb {
                    false => self.set_program_counter(rs_val),
                    true => self.set_program_counter(rs_val & 0xFFFFFFFE),
                }

                // Early return to prevent program_counter stepping
                self.cycle_count += 3;
                return;
            }
            _ => unreachable!("op > 0x3 (`{}`)", opcode),
        };

        if rd != 15 || op == 0x1 {
            self.step_program_counter(2);
        }

        self.cycle_count += cycles;
    }

    /// Format6
    fn thumb_pc_relative_load(&mut self, opcode: u16) {
        let word = ((opcode & 0xFF) << 2) as u32;
        let rd = ((opcode >> 8) & 0x7) as u8;

        let addr = self
            .get_program_counter()
            .wrapping_add(4)
            .wrapping_add(word)
            & 0xFFFFFFFD;
        let val = self.read_u32(true, addr);

        info!(
            "[0x{:08X}] => execute: `LDR R{},[PC,#0x{:02X}]`, R{} => [0x{:08X}] => 0x{:X}",
            self.registers[15], rd, word, rd, addr, val
        );

        self.write_register(rd, val);

        self.step_program_counter(2);
        self.cycle_count += 3;
    }

    /// Format7
    fn thumb_load_store_register_offset(&mut self, opcode: u16) {
        let rd = (opcode & 0x7) as u8;
        let rb = ((opcode >> 3) & 0x7) as u8;
        let ro = ((opcode >> 6) & 0x7) as u8;

        let byte = (opcode & 0x400) != 0;
        let load = (opcode & 0x800) != 0;

        let ptr = self.read_register(rb).wrapping_add(self.read_register(ro));

        match (load, byte) {
            (false, false) => {
                self.write_u32(true, ptr, self.read_register(rd));
                info!(
                    "[0x{:08X}] => execute: `STR R{rd},[R{rb},R{ro}]`",
                    self.get_program_counter()
                );
            }
            (false, true) => {
                self.write_u8(true, ptr, (self.read_register(rd) & 0xFF) as u8);
                info!(
                    "[0x{:08X}] => execute: `STRB R{rd},[R{rb},R{ro}]`",
                    self.get_program_counter()
                );
            }
            (true, false) => {
                let val = self.read_u32(true, ptr);
                self.write_register(rd, val);
                info!(
                    "[0x{:08X}] => execute: `LDR R{rd},[R{rb},R{ro}]`",
                    self.get_program_counter()
                );
            }
            (true, true) => {
                let val = self.read_u8(true, ptr);
                self.write_register(rd, val as u32);
                info!(
                    "[0x{:08X}] => execute: `LDRB R{rd},[R{rb},R{ro}]`",
                    self.get_program_counter()
                );
            }
        }

        self.step_program_counter(2);

        self.cycle_count += match (load, rd == 15) {
            (false, _) => 2,
            (true, false) => 3,
            (true, true) => 3 + 2,
        };
    }

    /// Thumb Format8
    fn thumb_load_store_sign_extended_byte_halfword(&mut self, opcode: u16) {
        let rd = (opcode & 0x7) as u8;
        let rb = ((opcode >> 3) & 0x7) as u8;
        let ro = ((opcode >> 6) & 0x7) as u8;
        let sign_extended = (opcode & 0x400) != 0;
        let h_flag = (opcode & 0x800) != 0;

        let addr = self.read_register(rb) + self.read_register(ro);

        match (sign_extended, h_flag) {
            (false, false) => {
                let val = self.read_register(rd) & 0xFFFF;
                let addr_val = self.read_u32(true, addr);
                info!(
                    "[0x{:08X}] => execute: `STRH R{},[R{},R{}]`, [0x{:08X}] => 0x{:X}",
                    self.registers[15], rd, rb, ro, addr, val
                );
                self.write_u32(true, addr, (addr_val & 0xFFFF0000) | val);
            }
            (false, true) => {
                let val = self.read_u16(true, addr);
                info!(
                    "[0x{:08X}] => execute: `LDR R{},[R{},R{}]`, R{} => [0x{:08X}] => 0x{:X}",
                    self.registers[15], rd, rb, ro, rd, addr, val
                );
                self.write_register(rd, val as u32);
            }
            (true, false) => {
                let val = self.read_u32(true, addr) & 0xFF;
                let val = if (val & 0x80) != 0 {
                    val | (0xFFFFFF00)
                } else {
                    val
                };

                info!(
                    "[0x{:08X}] => execute: `LDSB R{},[R{},R{}]`, R{} => [0x{:08X}] => 0x{:X}",
                    self.registers[15], rd, rb, ro, rd, addr, val
                );
                self.write_register(rd, val);
            } //format!("LDSB R{},[R{},R{}]", rd, rb, ro),
            (true, true) => {
                let val = self.read_u32(true, addr) & 0xFFFF;
                let val = if (val & 0x8000) != 0 {
                    val | (0xFFFF0000)
                } else {
                    val
                };

                info!(
                    "[0x{:08X}] => execute: `LDSH R{},[R{},R{}]`, R{} => [0x{:08X}] => 0x{:X}",
                    self.registers[15], rd, rb, ro, rd, addr, val
                );
                self.write_register(rd, val);
            }
        }

        self.step_program_counter(2);
        self.cycle_count += match (sign_extended, h_flag, rd == 15) {
            (false, false, _) => 2, // STRH
            (_, _, false) => 3,     // LDR/LSB/LDSH, Rd != 15
            (_, _, true) => 3 + 2,  // LDR/LSB/LDSH, Rd == 15
        };
    }

    /// Format9
    fn thumb_load_store_immediate(&mut self, opcode: u16) {
        let rd = (opcode & 0x7) as u8;
        let rb = ((opcode >> 3) & 0x7) as u8;
        let offset = ((opcode >> 6) & 0x1F) as u32;
        let load = (opcode & 0x800) != 0;
        let byte = (opcode & 0x1000) != 0;

        // For word access, assembler places #imm >> 2 in offset.
        let offset = match byte {
            false => offset << 2,
            true => offset,
        };

        match (load, byte) {
            (false, false) => info!(
                "[0x{:08X}] => execute: `STR R{rd},[R{rb},#0x{offset:X}]`",
                self.get_program_counter()
            ),
            (false, true) => info!(
                "[0x{:08X}] => execute: `STRB R{rd},[R{rb},#0x{offset:X}]`",
                self.get_program_counter()
            ),
            (true, false) => info!(
                "[0x{:08X}] => execute: `LDR R{rd},[R{rb},#0x{offset:X}]`",
                self.get_program_counter()
            ),
            (true, true) => info!(
                "[0x{:08X}] => execute: `LDRB R{rd},[R{rb},#0x{offset:X}]`",
                self.get_program_counter()
            ),
        }

        self.operation_ldr_str(rd, rb, offset, load, false, true, true, byte, false);
        self.step_program_counter(2);
    }

    /// Format10
    fn thumb_load_store_halfword(&mut self, opcode: u16) {
        let rd = (opcode & 0x7) as u8;
        let rb = ((opcode >> 3) & 0x7) as u8;
        let offset = (((opcode >> 6) & 0x1F) << 1) as u32;
        let load = (opcode & 0x800) != 0;

        let addr = self.read_register(rb).wrapping_add(offset as u32);
        let addr_val = self.read_u32(true, addr);

        match load {
            false => {
                info!(
                    "[0x{:08X}] => execute: `STRH R{},[R{}, #0x{:02X}]`",
                    self.registers[15], rd, rb, offset
                );
                self.write_u32(
                    true,
                    addr,
                    (addr_val & 0xFFFF0000) | (self.read_register(rd) & 0xFFFF),
                )
            }
            true => {
                info!(
                    "[0x{:08X}] => execute: `LDRH R{},[R{}, #0x{:02X}]`",
                    self.registers[15], rd, rb, offset
                );
                self.write_register(rd, addr_val & 0xFFFF)
            }
        }

        self.step_program_counter(2);

        self.cycle_count += match (load, rd == 15) {
            (false, _) => 2,
            (true, false) => 3,
            (true, true) => 3 + 2,
        };
    }

    /// Format11
    fn thumb_sp_relative_load_store(&mut self, opcode: u16) {
        let imm = ((opcode & 0xFF) << 2) as u32;
        let rd = ((opcode >> 8) & 0x7) as u8;
        let load = (opcode & 0x800) != 0;

        self.operation_ldr_str(rd, 13, imm, load, false, true, true, false, false);

        let pc = self.get_program_counter();
        if load {
            info!("[0x{pc:08X}] => execute: `LDR R{rd},[SP,#0x{imm:X}]`");
        } else {
            info!("[0x{pc:08X}] => execute: `STR R{rd},[SP,#0x{imm:X}]`");
        }

        self.step_program_counter(2);
    }

    /// Format12
    fn thumb_load_address(&mut self, opcode: u16) {
        let imm = ((opcode & 0xFF) << 2) as u32;
        let rd = ((opcode >> 8) & 0x7) as u8;
        let sp = (opcode & 0x0800) != 0;

        let val = match sp {
            false => {
                info!(
                    "[0x{:08X}] => execute: `ADD R{},PC,#0x{:X}`",
                    self.registers[15], rd, imm
                );
                (self.get_program_counter() + 4) & 0xFFFFFFFD
            }
            true => {
                info!(
                    "[0x{:08X}] => execute: `ADD R{},SP,#0x{:X}`",
                    self.registers[15], rd, imm
                );
                self.read_register(13)
            }
        }
        .wrapping_add(imm);

        self.write_register(rd, val);

        self.step_program_counter(2);
        self.cycle_count += 1;
    }

    /// Format13
    fn thumb_offset_to_sp(&mut self, opcode: u16) {
        let imm = ((opcode & 0x7F) << 2) as u32;
        let neg = (opcode & 0x80) != 0;

        let sp_val = self.read_register(13);

        let res = match neg {
            false => {
                info!(
                    "[0x{:08X}] => execute: `ADD SP,#0x{:X}`",
                    self.registers[15], imm
                );
                sp_val.wrapping_add(imm)
            }
            true => {
                info!(
                    "[0x{:08X}] => execute: `SUB SP,#0x{:X}`",
                    self.registers[15], imm
                );
                sp_val.wrapping_sub(imm)
            }
        };

        self.write_register(13, res);
        self.step_program_counter(2);
        self.cycle_count += 1;
    }

    /// Format14
    fn thumb_push_pop(&mut self, opcode: u16) {
        let load = (opcode & 0x0800) != 0;
        let store_lr = (opcode & 0x0100) != 0;
        let r_list = (opcode & 0xFF) as u16;

        let r_list = match (load, store_lr) {
            (false, true) => {
                info!(
                    "[0x{:08X}] => execute: `PUSH {{{r_list:08b}, LR}}",
                    self.get_program_counter()
                );
                r_list | (1 << 14)
            }
            (true, true) => {
                info!(
                    "[0x{:08X}] => execute: `POP {{{r_list:08b}, PC}}",
                    self.get_program_counter()
                );
                r_list | (1 << 15)
            }
            (false, false) => {
                info!(
                    "[0x{:08X}] => execute: `PUSH {{{r_list:08b}}}",
                    self.get_program_counter()
                );
                r_list
            }
            (true, false) => {
                info!(
                    "[0x{:08X}] => execute: `POP {{{r_list:08b}}}",
                    self.get_program_counter()
                );
                r_list
            }
        };

        if load {
            // Pop => LDMIA R13!, {Rlist, R15?}
            self.operation_ldm_stm(13, r_list, load, true, false, true, false);
        } else {
            // Push => STMDB R13!, {Rlist, R14?}
            self.operation_ldm_stm(13, r_list, load, true, true, false, false);
        }

        if load && store_lr {
            // Rewritten PC from pop, do not step
        } else {
            self.step_program_counter(2);
        }
    }

    /// Format15
    fn thumb_multiple_load_store(&mut self, opcode: u16) {
        let r_list = (opcode & 0xFF) as u16;
        let r_base = ((opcode >> 8) & 0x7) as u8;
        let load = (opcode & 0x0800) != 0;

        match load {
            false => info!(
                "[0x{:08X}] => execute: `STMIA R{}!,{{{:08b}}}`",
                self.registers[15], r_base, r_list
            ),
            true => info!(
                "[0x{:08X}] => execute: `LDMIA R{}!,{{{:08b}}}`",
                self.registers[15], r_base, r_list
            ),
        }

        // STMIA Rb!,{Rlist} or LDMIA Rb!,{Rlist}
        let pre = false;
        let up = true;
        let s_bit = false;
        let wb = true;
        self.operation_ldm_stm(r_base, r_list, load, wb, pre, up, s_bit);

        self.step_program_counter(2);
    }

    /// Format16
    fn thumb_conditional_branch(&mut self, opcode: u16) {
        let offset = ((opcode & 0xFF) << 1) as u32;
        let cond = ((opcode >> 8) & 0xF) as u8;

        let offset = match (opcode & 0x80) == 0 {
            false => offset | 0xFFFFFE00,
            true => offset,
        };

        let str = match cond {
            0x0 => "BEQ",
            0x1 => "BNE",
            0x2 => "BCS",
            0x3 => "BCC",
            0x4 => "BMI",
            0x5 => "BPL",
            0x6 => "BVS",
            0x7 => "BVC",
            0x8 => "BHI",
            0x9 => "BLS",
            0xA => "BGE",
            0xB => "BLT",
            0xC => "BGT",
            0xD => "BLE",
            _ => "B???",
        };

        let addr = self
            .get_program_counter()
            .wrapping_add(4)
            .wrapping_add(offset);

        if self.should_execute(cond) {
            info!(
                "[0x{:08X}] => execute: `{} {:08X}` => Take",
                self.registers[15], str, addr
            );
            self.set_program_counter(addr);

            self.cycle_count += 3;
        } else {
            info!(
                "[0x{:08X}] => execute: `{} {:08X}` => Skip",
                self.registers[15], str, addr
            );
            self.step_program_counter(2);

            self.cycle_count += 1;
        }
    }

    /// Format17
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

        self.operation_swi(syscall);
        self.step_program_counter(2);
    }

    /// Format18
    fn thumb_unconditional_branch(&mut self, opcode: u16) {
        let offset = ((opcode & 0x7FF) << 1) as u32;
        let neg = (offset & 0x800) != 0;

        let extend = match neg {
            false => offset,
            true => 0xFFFFF000 | offset,
        };

        info!(
            "[0x{:08X}] => execute: `B {}`",
            self.registers[15], extend as i32
        );

        self.set_program_counter(
            self.get_program_counter()
                .wrapping_add(extend)
                .wrapping_add(4),
        );

        self.cycle_count += 3;
    }

    /// Format19
    fn thumb_long_branch_link(&mut self, opcode: u16) {
        let offset = (opcode & 0x7FF) as u32;
        let h = (opcode & 0x0800) != 0;

        match h {
            false => {
                let offset_sign_extend = match (offset & 0x400) != 0 {
                    false => offset << 12,
                    true => 0xFF800000 | (offset << 12),
                };

                //panic!("BL0, offset=0b{:b}, neg={}", offset, (offset & 0x400) != 0);

                self.write_register(
                    14,
                    (self.get_program_counter().wrapping_add(4)).wrapping_add(offset_sign_extend),
                );
                self.step_program_counter(2);
                self.cycle_count += 1;
            }
            true => {
                let next = self.get_program_counter() + 2;
                self.set_program_counter(self.read_register(14).wrapping_add(offset << 1));
                self.write_register(14, next | 1);
                debug!("Written `{:08X}` to LR", next | 1);
                self.cycle_count += 3;
            }
        }

        info!(
            "[0x{:08X}] => execute: `BL{} {}`",
            self.registers[15], h as u8, offset
        );
    }

    fn alu_operand2_calc(&self, imm: bool, op2: u16) -> (u32, bool) {
        if imm {
            let rotate = ((op2 >> 8) & 0xF) as u32;
            let val = (op2 & 0xFF) as u32;

            (val.rotate_right(rotate * 2), self.get_flag_c())
        } else {
            let rm = (op2 & 0xF) as u8;
            let rm_val = self.read_register(rm);
            let shift_type = (op2 >> 5) & 0x3;
            let shift_imm = (op2 & 0x10) == 0x0;

            let shift_amount = match shift_imm {
                false => {
                    let rs = ((op2 >> 8) & 0xF) as u8;
                    self.read_register(rs) & 0xFF
                }
                true => ((op2 >> 7) & 0x1F) as u32,
            };

            match shift_type {
                0b00 => {
                    let carry = match shift_amount {
                        0 => self.get_flag_c(),
                        1..=31 => (rm_val & (1 << (31 - shift_amount))) != 0,
                        _ => false,
                    };

                    (rm_val << shift_amount, carry)
                }
                0b01 => match shift_amount {
                    // 0 is encoded as shift 32 => 0, with carry of bit 31
                    0 => (0, (rm_val & (1 << 31)) != 0),
                    1..=31 => (
                        (rm_val >> shift_amount),
                        (rm_val & (1 << (shift_amount - 1))) != 0,
                    ),
                    _ => (0, false),
                },
                0b10 => match (shift_amount, (rm_val as i32) < 0) {
                    (1..=31, _) => (
                        ((rm_val as i32) >> shift_amount) as u32,
                        (rm_val & (1 << (shift_amount - 1))) != 0,
                    ),
                    (_, false) => (0x00000000, (rm_val & (1 << 31)) != 0),
                    (_, true) => (0xFFFFFFFF, (rm_val & (1 << 31)) != 0),
                },
                0b11 => match shift_amount {
                    0 => (
                        (rm_val >> 1) | ((self.get_flag_c() as u32) << 31),
                        (rm_val & 0x1) != 0,
                    ),
                    _ => (
                        rm_val.rotate_right(shift_amount),
                        (rm_val & (1 << (shift_amount - 1))) != 0,
                    ),
                },
                _ => unreachable!(),
            }
        }
    }

    /// Performs ALU operation
    /// Returns (result, N, Z, C, V)
    fn alu(&mut self, op: u8, operand1: u32, operand2: u32) -> (u32, bool, bool, bool, bool) {
        let op1_is_neg = (operand1 as i32) < 0;
        let op2_is_neg = (operand2 as i32) < 0;

        match op {
            ALU_TST => return self.alu_tst(operand1, operand2),
            ALU_CMP => {
                let (result, carry) = operand1.overflowing_sub(operand2);
                let signed = result as i32;
                return (
                    result,
                    signed < 0,
                    signed == 0,
                    !carry,
                    (op1_is_neg != op2_is_neg) && (op2_is_neg == (signed < 0)),
                );
            }
            ALU_CMN => {
                let (result, carry) = operand1.overflowing_add(operand2);
                let signed = result as i32;
                return (
                    result,
                    signed < 0,
                    signed == 0,
                    carry,
                    (op1_is_neg == op2_is_neg) && (op2_is_neg != (signed < 0)),
                );
            }
            _ => {}
        }

        let (result, carry, overflow) = match op {
            ALU_AND | ALU_TST => {
                if op == 0x0 {
                    //info!("ALU: `AND #{}, #{}`", operand1, operand2);
                } else {
                    //println!("ALU: `TST #{}, #{}`", operand1, operand2);
                }
                (operand1 & operand2, false, false)
            }
            ALU_EOR | ALU_TEQ => {
                if op == 0x1 {
                    //info!("ALU: `EOR #{}, #{}`", operand1, operand2);
                } else {
                    //info!("ALU: `TEQ #{}, #{}`", operand1, operand2);
                }
                (operand1 ^ operand2, false, false)
            }
            ALU_SUB | ALU_CMP => {
                if op == 0x2 {
                    //info!("ALU: `SUB #{}, #{}`", operand1, operand2);
                } else {
                    //info!("ALU: `CMP #{}, #{}`", operand1, operand2);
                }
                let (result, carry) = operand1.overflowing_sub(operand2);
                let overflow = (operand1 & 0x80000000) != (result & 0x80000000);
                (result, !carry, overflow)
            }
            ALU_RSB => {
                //info!("ALU: `RSB #{}, #{}`", operand1, operand2);
                let (result, carry) = operand2.overflowing_sub(operand1);
                let overflow = (operand2 ^ result) & 0x80000000 > 0;
                (result, !carry, overflow)
            }
            ALU_ADD | ALU_CMN => {
                if op == 0x4 {
                    //info!("ALU: `ADD #{}, #{}`", operand1, operand2);
                } else {
                    //info!("ALU: `CMN #{}, #{}`", operand1, operand2);
                }
                let (result, carry) = operand1.overflowing_add(operand2);
                let overflow = (operand1 ^ result) & 0x80000000 > 0;
                (result, carry, overflow)
            }
            ALU_ADC => {
                //info!(
                //    "ALU: `ADC #{}, #{}, C{}`",
                //    operand1,
                //    operand2,
                //    self.get_flag_c() as u8
                //);
                let (result, carry) = operand1.overflowing_add(self.get_flag_c() as u32);
                let (result, carry2) = result.overflowing_add(operand2);
                let overflow = (operand1 ^ result) & 0x80000000 > 0;
                (result, carry | carry2, overflow)
            }
            ALU_SBC => {
                //info!(
                //    "ALU: `SBC #{}, #{}, C{}`",
                //    operand1,
                //    operand2,
                //    self.get_flag_c() as u8
                //);
                let (result, carry) = operand2.overflowing_add(self.get_flag_c() as u32);
                let (result, carry2) = result.overflowing_sub(1);
                let (result, carry3) = operand1.overflowing_sub(result);
                let overflow = (operand1 ^ result) & 0x80000000 > 0;

                // TODO: Is this correct?
                warn!("Data Processing: SBC, correct?");
                (result, carry | carry2 | carry3, overflow)
            }
            ALU_RSC => {
                //info!(
                //    "ALU: `RSC #{}, #{}, C{}`",
                //    operand1,
                //    operand2,
                //    self.get_flag_c() as u8
                //);
                let (result, carry) = operand1.overflowing_add(self.get_flag_c() as u32);
                let (result, carry2) = result.overflowing_sub(1);
                let (result, carry3) = operand2.overflowing_sub(result);
                let overflow = (operand2 ^ result) & 0x80000000 > 0;

                // TODO: Is this correct?
                warn!("Data Processing: RSC, correct?");
                (result, carry | carry2 | carry3, overflow)
            }
            ALU_ORR => {
                //info!("ALU: `ORR #0x{:X}, #0x{:X}`", operand1, operand2);
                (operand1 | operand2, false, false)
            }
            ALU_MOV => {
                //info!("ALU: `MOV #0x{:X}`", operand2);
                (operand2, false, false)
            }
            ALU_BIC => {
                //info!("ALU: `BIC #{}, #{}`", operand1, operand2);
                (operand1 & !(operand2), false, false)
            }
            ALU_MVN => {
                //info!("ALU: `MVN #{}`", operand2);
                (!operand2, false, false)
            }
            _ => unreachable!(),
        };

        let negative = (result as i32) < 0;
        let zero = result == 0;
        (result, negative, zero, carry, overflow)
    }

    /// Returns (n, z)
    fn alu_tst(&mut self, operand1: u32, operand2: u32) -> (u32, bool, bool, bool, bool) {
        let result = operand1 & operand2;

        let n = (result as i32) < 0;
        let z = result == 0;
        let c = self.get_flag_c();
        let v = self.get_flag_v();

        (result, n, z, c, v)
    }

    fn execute_thumb(&mut self, opcode: u16) {
        let high = (opcode >> 8) as u8;

        match high {
            0x00..=0x17 => {
                self.thumb_move_shifted_register(opcode);
            } // Move shifted register
            0x18..=0x1F => {
                self.thumb_add_subtract(opcode);
            } // Add/subtract
            0x20..=0x3F => {
                self.thumb_mov_cmp_add_sub_imm(opcode);
            } // Move/compare/add/subtract immediate
            0x40..=0x43 => {
                self.thumb_alu(opcode);
            } // ALU operations
            0x44..=0x47 => self.thumb_hi_register_op_bx(opcode), // Hi register operations/branch exchange
            0x48..=0x4F => {
                self.thumb_pc_relative_load(opcode);
            } // PC-relative load
            0x50 | 0x51 | 0x54 | 0x55 | 0x58 | 0x59 | 0x5C | 0x5D => {
                self.thumb_load_store_register_offset(opcode);
            } // Load/store with register offset
            0x52 | 0x53 | 0x56 | 0x57 | 0x5A | 0x5B | 0x5E | 0x5F => {
                self.thumb_load_store_sign_extended_byte_halfword(opcode);
            } // Load/store sign-extended byte/halfword
            0x60..=0x7F => {
                self.thumb_load_store_immediate(opcode);
            } // load/store with immediate offset
            0x80..=0x8F => {
                self.thumb_load_store_halfword(opcode);
            } // Load/store halfword
            0x90..=0x9F => {
                self.thumb_sp_relative_load_store(opcode);
            } // SP-relative load/store
            0xA0..=0xAF => {
                self.thumb_load_address(opcode);
            } // Load address
            0xB0 => {
                self.thumb_offset_to_sp(opcode);
            } // Add offset to stack pointer
            0xB4 | 0xB5 | 0xBC | 0xBD => {
                self.thumb_push_pop(opcode);
            } // Push/pop registers
            0xC0..=0xCF => {
                self.thumb_multiple_load_store(opcode);
            } // Multiple load/store
            0xD0..=0xDE => {
                self.thumb_conditional_branch(opcode);
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
            info!(
                "[0x{:08X}] => execute: `BL 0x{:X}` => 0x{:08X}",
                self.registers[15], offset, target
            );
            self.write_register(14, self.get_program_counter() + 4);
        } else {
            info!(
                "[0x{:08X}] => execute: `B 0x{:X}` => 0x{:08X}",
                self.registers[15],
                offset + 8,
                target
            );
        }

        self.set_program_counter(target);
        self.cycle_count += 3;
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

        let (operand2, shifter_carry) = self.alu_operand2_calc(i, (opcode & 0xFFFF) as u16);
        let op2_str = String::new();

        let (result, logical, carry, overflow) = match op {
            0x0 | 0x8 => {
                if op == 0x0 {
                    info!(
                        "[0x{:08X}] => execute: `AND R{rd},???,{op2_str}`",
                        self.registers[15]
                    );
                } else {
                    info!("[0x{:08X}] => execute: `TST`", self.registers[15]);
                }
                (operand1 & operand2, true, false, false)
            }
            0x1 | 0x9 => {
                if op == 0x1 {
                    info!(
                        "[0x{:08X}] => execute: `EOR {},{}`",
                        self.registers[15], operand1, operand2
                    );
                } else {
                    info!(
                        "[0x{:08X}] => execute: `TEQ {},{}`",
                        self.registers[15], operand1, operand2
                    );
                }
                (operand1 ^ operand2, true, false, false)
            }
            0x2 | 0xA => {
                if op == 0x2 {
                    info!("[0x{:08X}] => execute: `SUB ???`", self.registers[15]);
                } else {
                    info!("[0x{:08X}] => execute: `CMP ???`", self.registers[15]);
                }
                let (result, carry) = operand1.overflowing_sub(operand2);
                let overflow = (operand1 ^ result) & 0x80000000 > 0;
                (result, false, carry, overflow)
            }
            0x3 => {
                info!("[0x{:08X}] => execute: `RSB ???`", self.registers[15]);
                let (result, carry) = operand2.overflowing_sub(operand1);
                let overflow = (operand2 ^ result) & 0x80000000 > 0;
                (result, false, carry, overflow)
            }
            0x4 | 0xB => {
                if op == 0x4 {
                    info!(
                        "[0x{:08X}] => execute: `ADD{} R{},R{},???`",
                        self.registers[15], s_str, rd, rn
                    );
                } else {
                    info!("[0x{:08X}] => execute: `CMN ???`", self.registers[15]);
                }
                let (result, carry) = operand1.overflowing_add(operand2);
                let overflow = (operand1 ^ result) & 0x80000000 > 0;
                (result, false, carry, overflow)
            }
            0x5 => {
                info!("[0x{:08X}] => execute: `ADC ???`", self.registers[15]);
                let (result, carry) = operand1.overflowing_add(self.get_flag_c() as u32);
                let (result, carry2) = result.overflowing_add(operand2);
                let overflow = (operand1 ^ result) & 0x80000000 > 0;
                (result, false, carry | carry2, overflow)
            }
            0x6 => {
                info!("[0x{:08X}] => execute: `SBC ???`", self.registers[15]);
                let (result, carry) = operand2.overflowing_add(self.get_flag_c() as u32);
                let (result, carry2) = result.overflowing_sub(1);
                let (result, carry3) = operand1.overflowing_sub(result);
                let overflow = (operand1 ^ result) & 0x80000000 > 0;

                // TODO: Is this correct?
                warn!("Data Processing: SBC, correct?");
                (result, false, carry | carry2 | carry3, overflow)
            }
            0x7 => {
                info!("[0x{:08X}] => execute: `RSC ???`", self.registers[15]);
                let (result, carry) = operand1.overflowing_add(self.get_flag_c() as u32);
                let (result, carry2) = result.overflowing_sub(1);
                let (result, carry3) = operand2.overflowing_sub(result);
                let overflow = (operand2 ^ result) & 0x80000000 > 0;

                // TODO: Is this correct?
                warn!("Data Processing: RSC, correct?");
                (result, false, carry | carry2 | carry3, overflow)
            }
            0xC => {
                info!(
                    "[0x{:08X}] => execute: `ORR R{},R{},#0x{:X}`",
                    self.registers[15], rd, rn, operand2
                );
                (operand1 | operand2, true, false, false)
            }
            0xD => {
                info!(
                    "[0x{:08X}] => execute: `MOV R{},#0x{:X}`",
                    self.registers[15], rd, operand2
                );
                (operand2, true, false, false)
            }
            0xE => {
                info!(
                    "[0x{:08X}] => execute: `BIC R{},R{},#0x{:X}`",
                    self.registers[15], rd, rn, operand2
                );
                (operand1 & !(operand2), true, false, false)
            }
            0xF => {
                info!("[0x{:08X}] => execute: `MVN`", self.registers[15]);
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
                if set_condition {
                    // Return from IRQ
                    if self.get_mode() == MODE_IRQ {
                        warn!("Exiting from IRQ");
                    }

                    self.reg_cpsr = self.regs_spsr[self.get_mode() as usize];
                }
            } else {
                self.set_flag_n(result & 0x80000000 == 0x80000000);
                self.set_flag_z(result == 0);

                if !logical {
                    self.set_flag_c(carry);
                    self.set_flag_v(overflow);
                } else {
                    self.set_flag_c(shifter_carry);
                }
            }
        }

        // If not test opcode, store result
        if op < 0x8 || op > 0xB {
            self.write_register(rd, result);
        }

        self.step_program_counter(4);
        self.cycle_count += match rd == 15 {
            false => 1,
            true => 1 + 2,
        };
    }

    fn arm_shifted_offset(&self) {
        todo!("Implement");
    }

    fn arm_single_data_transfer(&mut self, opcode: u32) {
        let offset = opcode & 0xFFF;
        let rd = ((opcode >> 12) & 0xF) as u8;
        let rb = ((opcode >> 16) & 0xF) as u8;

        let load = (opcode & (1 << 20)) != 0;
        let write_back = (opcode & (1 << 21)) != 0;
        let byte = (opcode & (1 << 22)) != 0;
        let up = (opcode & (1 << 23)) != 0;
        let pre = (opcode & (1 << 24)) != 0;
        let reg = (opcode & (1 << 25)) != 0;

        match (load, byte) {
            (false, false) => {
                info!(
                    "[0x{:08X}] => execute: `STR R{rd}[???] => Single Data Transfer`",
                    self.get_program_counter()
                )
            }
            (false, true) => {
                info!(
                    "[0x{:08X}] => execute: `STRB R{rd}[???] => Single Data Transfer`",
                    self.get_program_counter()
                )
            }
            (true, false) => {
                info!(
                    "[0x{:08X}] => execute: `LDR R{rd},[???] => Single Data Transfer`",
                    self.get_program_counter()
                )
            }
            (true, true) => {
                info!(
                    "[0x{:08X}] => execute: `LDRB R{rd},[???] => Single Data Transfer`",
                    self.get_program_counter()
                )
            }
        }

        self.operation_ldr_str(rd, rb, offset, load, write_back, pre, up, byte, reg);

        if rd != 15 {
            self.step_program_counter(4);
        }
    }

    fn arm_mrs(&mut self, opcode: u32) {
        let rd = ((opcode >> 12) & 0xF) as u8;
        let source_spsr = (opcode & 0x400000) != 0;

        let psr_val = match source_spsr {
            false => {
                info!(
                    "[0x{:08X}] => execute: `MRS R{rd},CPSR`",
                    self.get_program_counter()
                );
                self.reg_cpsr
            }
            true => {
                info!(
                    "[0x{:08X}] => execute: `MRS R{rd},SPSR`",
                    self.get_program_counter()
                );
                self.regs_spsr[self.get_mode() as usize]
            }
        };

        self.write_register(rd, psr_val);

        self.step_program_counter(4);
        self.cycle_count += 1;
    }

    fn arm_msr(&mut self, opcode: u32) {
        let dest_spsr = (opcode & 0x400000) != 0;
        let rm = (opcode & 0xF) as u8;
        let rm_val = self.read_register(rm);

        if dest_spsr {
            info!(
                "[0x{:08X}] => execute: `MSR SPSR,R{}`",
                self.registers[15], rm
            );
            self.regs_spsr[self.get_mode() as usize] = rm_val;
        } else {
            info!(
                "[0x{:08X}] => execute: `MSR CPSR,R{}`",
                self.registers[15], rm
            );

            if self.get_mode() == MODE_USER {
                self.reg_cpsr = (self.reg_cpsr & 0x0FFFFFFF) | (rm_val & 0xF0000000);
            } else {
                self.reg_cpsr = rm_val;
            }
        }

        self.step_program_counter(4);
        self.cycle_count += 1;
    }

    fn arm_branch_exchange(&mut self, opcode: u32) {
        let rm = (opcode & 0xF) as u8;
        let rm_val = self.read_register(rm);
        let thumb = (rm_val & 0x1) == 0x1;

        info!(
            "[0x{:08X}] => execute: `BX R{}` => thumb={}",
            self.registers[15], rm, thumb
        );

        self.set_thumb(thumb);
        match thumb {
            false => self.set_program_counter(rm_val),
            true => self.set_program_counter(rm_val & 0xFFFFFFFE),
        }

        self.cycle_count += 3;
    }

    fn arm_halfword_data_transfer_imm(&mut self, opcode: u32) {
        let offset = ((opcode >> 4) & 0xF0) | (opcode & 0xF);
        let h = (opcode & 0x20) != 0;
        let s = (opcode & 0x40) != 0;
        let load = (opcode & 0x100000) != 0;
        let write_back = (opcode & 0x200000) != 0;
        let up = (opcode & 0x800000) != 0;
        let pre = (opcode & 0x1000000) != 0;

        let rd = ((opcode >> 12) & 0xF) as u8;
        let rn = ((opcode >> 16) & 0xF) as u8;
        let mut base = match rn {
            15 => self.read_register(rn) + 8,
            _ => self.read_register(rn),
        };

        if pre {
            base = match up {
                false => base.wrapping_sub(offset),
                true => base.wrapping_add(offset),
            }
        }

        match (s, h) {
            (false, false) => {
                // SWP Instruction
                todo!("SWP");
            }
            (false, true) => {
                // Unsigned Halfwords
                match load {
                    false => {
                        info!(
                            "[0x{:08X}] => execute: `STRH R{rd},[R{rn},#0x{offset:X}]`",
                            self.get_program_counter()
                        );
                        let val = self.read_register(rd) & 0xFFFF;
                        self.write_u32(true, base, val);
                    }
                    true => {
                        info!(
                            "[0x{:08X}] => execute: `LDRH R{rd},[R{rn},#0x{offset:X}]`",
                            self.get_program_counter()
                        );
                        let val = self.read_u32(true, base) & 0xFFFF;
                        self.write_register(rd, val);
                    }
                }
            }
            (true, false) => {
                // Signed Byte
                info!(
                    "[0x{:08X}] => execute: `LDRSB R{rd},[R{rn},#0x{offset:X}]`",
                    self.get_program_counter()
                );

                let val = self.read_u8(true, base);

                // Sign extend
                let val = match (val as i8) > 0 {
                    false => 0xFFFFFF00 | (val as u32),
                    true => val as u32,
                };

                self.write_register(rd, val);
            }
            (true, true) => {
                // Signed Halfwords
                info!(
                    "[0x{:08X}] => execute: `LDRSH R{rd},[R{rn},#0x{offset:X}]`",
                    self.get_program_counter()
                );

                todo!("Signed Halfwords");
            }
        }

        if !pre {
            base = match up {
                false => base.wrapping_sub(offset),
                true => base.wrapping_add(offset),
            }
        }

        if write_back || !pre {
            self.write_register(rn, base);
        }

        self.step_program_counter(4);
        self.cycle_count += match (load, rd == 15) {
            (false, _) => 2,
            (true, false) => 3,
            (true, true) => 3 + 2,
        }
    }

    fn arm_block_data_transfer(&mut self, opcode: u32) {
        let r_list = (opcode & 0xFFFF) as u16;
        let r_base = ((opcode >> 16) & 0xF) as u8;
        let load = (opcode & 0x100000) != 0;
        let wb = (opcode & 0x200000) != 0;
        let s_bit = (opcode & 0x400000) != 0;
        let up = (opcode & 0x800000) != 0;
        let pre = (opcode & 0x1000000) != 0;

        self.operation_ldm_stm(r_base, r_list, load, wb, pre, up, s_bit);

        self.step_program_counter(4);
    }

    /// Performs SWI to `syscall`
    /// Updates cycle_count accordingly
    fn operation_swi(&mut self, syscall: u8) {
        info!(
            "[0x{:08X}] => execute: `SWI {:02X}`",
            self.registers[15], syscall
        );

        self.bios_syscall(syscall);
        self.cycle_count += 3;
    }

    /// Performs LDR or STR based on args
    /// Updates `self.cycle_count` accordingly
    fn operation_ldr_str(
        &mut self,
        r_dest: u8,
        r_base: u8,
        offset: u32,
        load: bool,
        wb: bool,
        pre: bool,
        up: bool,
        byte: bool,
        reg: bool,
    ) {
        if r_dest == 15 && !load {
            todo!("Implement store with dest as r15");
        }

        // Shifted offset
        if reg {
            let rm = (offset & 0xF) as usize;
            let shift = offset >> 4;

            todo!("Implement shifts in LDR/STR");
        }

        let base = if r_base == 15 {
            if self.is_thumb() {
                self.read_register(15) + 4
            } else {
                self.read_register(15) + 8
            }
        } else {
            self.read_register(r_base as u8)
        };

        let offsetted_addr = if up {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        };

        if load {
            let val = match (pre, byte) {
                (false, false) => self.read_u32(true, base),
                (false, true) => self.read_u8(true, base) as u32,
                (true, false) => self.read_u32(true, offsetted_addr),
                (true, true) => self.read_u8(true, offsetted_addr) as u32,
            };
            self.write_register(r_dest as u8, val);
        } else {
            let val = self.read_register(r_dest as u8);
            match (pre, byte) {
                (false, false) => self.write_u32(true, base, val),
                (false, true) => self.write_u8(true, base, val as u8),
                (true, false) => self.write_u32(true, offsetted_addr, val),
                (true, true) => self.write_u8(true, offsetted_addr, val as u8),
            };
        }

        // Write back to base register
        if wb || !pre {
            self.write_register(r_base as u8, offsetted_addr);
        }

        self.cycle_count += match (load, r_dest == 15) {
            (false, _) => 2,
            (true, false) => 3,
            (true, true) => 3 + 2,
        }
    }

    /// Performs LDRH or STRH based on args
    /// Updates `self.cycle_count` accordingly
    fn operation_ldrh_strh(&mut self) {
        todo!("Implement");
    }

    /// Performs LDM or STM based on args
    /// Updates `self.cycle_count` accordingly
    fn operation_ldm_stm(
        &mut self,
        r_base: u8,
        r_list: u16,
        load: bool,
        wb: bool,
        pre: bool,
        up: bool,
        s_bit: bool,
    ) {
        if s_bit {
            todo!("S bit set (r15={})", (r_list & 0x8000) != 0);
        }

        let num_reg = u16::count_ones(r_list);

        let base = self.read_register(r_base);
        let final_rb = match up {
            false => base - (num_reg * 4),
            true => base + (num_reg * 4),
        };

        let mut ptr = match (up, pre) {
            (false, false) => base - (num_reg * 4) + 4, // Post-Decrement
            (false, true) => base - (num_reg * 4),      // Pre-Decrement
            (true, false) => base,                      // Post-Increment
            (true, true) => base + 4,                   // Pre-Increment
        };

        for i in 0..16 {
            if (r_list & (1 << i)) != 0 {
                match load {
                    false => {
                        let val = self.read_register(i);
                        self.write_u32(false, ptr, val);
                    }
                    true => {
                        let val = self.read_u32(false, ptr);
                        self.write_register(i, val);
                    }
                }

                ptr += 4;
            }
        }

        if wb {
            self.write_register(r_base, final_rb);
        }

        // Update cycle count
        self.cycle_count += match load {
            false => 1,
            true => 2,
        };

        self.cycle_count += num_reg as usize;

        if load && ((r_list & (1 << 15)) != 0) {
            self.cycle_count += 2;
        }
    }

    pub fn opcode_match(opcode: u32, mask_clr: u32, mask_set: u32) -> bool {
        (opcode & mask_set == mask_set) && ((!opcode) & mask_clr == mask_clr)
    }

    fn arm_multiply(&mut self, opcode: u32) {
        let rm = (opcode & 0xF) as u8;
        let rs = ((opcode >> 8) & 0xF) as u8;
        let rn = ((opcode >> 12) & 0xF) as u8;
        let rd = ((opcode >> 16) & 0xF) as u8;

        let set_condition = (opcode & 0x100000) != 0;
        let accumulate = (opcode & 0x200000) != 0;

        let rm_val = self.read_register(rm);
        let rs_val = self.read_register(rs);
        let rn_val = self.read_register(rn);

        let result = match accumulate {
            false => rm_val.wrapping_mul(rs_val),
            true => rm_val.wrapping_mul(rs_val).wrapping_add(rn_val),
        };

        self.write_register(rd, result);

        if set_condition {
            self.set_flag_n((result as i32) < 0);
            self.set_flag_z(result == 0);
        }

        self.step_program_counter(4);
        let rs_sign = rs_val as i32;
        let m_cycles = if -(1 << 8) <= rs_sign && rs_sign < (1 << 8) {
            1
        } else if -(1 << 16) <= rs_sign && rs_sign < (1 << 16) {
            2
        } else if -(1 << 24) <= rs_sign && rs_sign < (1 << 24) {
            3
        } else {
            4
        };

        self.cycle_count += match accumulate {
            false => 1 + m_cycles,
            true => 2 + m_cycles,
        };
    }

    fn execute_arm(&mut self, opcode: u32) {
        let instr = ((opcode >> 20) & 0xFF) as u8;
        let cond = ((opcode >> 28) & 0xF) as u8;

        // Check conditional
        if !self.should_execute(cond) {
            info!("Skipped execution");
            self.step_program_counter(4);

            self.cycle_count += 1;
            return;
        }

        match instr {
            0x00..=0x3F => {
                if Self::opcode_match(opcode, ARM_MASK_MUL_CLR, ARM_MASK_MUL_SET) {
                    self.arm_multiply(opcode);
                } else if Self::opcode_match(opcode, ARM_MASK_MUL_LONG_CLR, ARM_MASK_MUL_LONG_SET) {
                    todo!("Multiply Long");
                } else if Self::opcode_match(opcode, ARM_MASK_SNGL_SWP_CLR, ARM_MASK_SNGL_SWP_SET) {
                    todo!("Single Data Swap");
                } else if Self::opcode_match(opcode, ARM_MASK_BX_CLR, ARM_MASK_BX_SET) {
                    self.arm_branch_exchange(opcode);
                } else if Self::opcode_match(opcode, ARM_MASK_HW_REG_CLR, ARM_MASK_HW_REG_SET) {
                    todo!("Halfword Data Transfer: register offset");
                } else if Self::opcode_match(opcode, ARM_MASK_HW_IMM_CLR, ARM_MASK_HW_IMM_SET) {
                    self.arm_halfword_data_transfer_imm(opcode);
                } else if Self::opcode_match(opcode, ARM_MASK_MRS_CLR, ARM_MASK_MRS_SET) {
                    self.arm_mrs(opcode);
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
            0xC0..=0xDF => panic!(
                "Coprocessor data transfer @ {:08X}",
                self.get_program_counter()
            ),
            0xE0..=0xEF => {
                if (opcode & 0x10) == 0 {
                    panic!(
                        "Coprocessor data operation @ {:08X}",
                        self.get_program_counter()
                    );
                } else {
                    panic!(
                        "Coprocessor register transfer @ {:08X}",
                        self.get_program_counter()
                    );
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
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);
        cpu.set_thumb(false);

        // MOV R0,#0x12
        let opcode: u32 = 0b1110_00_1_1101_0_0000_0000_000000010010;
        cpu.execute_arm(opcode);

        assert!(cpu.read_register(0) == 0x12);
    }

    // Thumb Format1
    #[test]
    fn thumb_move_shifted_register() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        let rs = 0b101;
        let rs_signed = 0x97000000;
        let offset = 2;

        let opcode_lsl = 0b0000000000001000 | (offset << 6);
        let opcode_lsr = 0b0000100000001000 | (offset << 6);
        let opcode_asr = 0b0001000000001000 | (offset << 6);

        cpu.reg_cpsr = 0x00000000;
        cpu.write_register(1, rs);
        cpu.execute_thumb(opcode_lsl);
        assert_eq!(cpu.read_register(0), rs << offset);
        assert_eq!(cpu.get_flag_n(), false);
        assert_eq!(cpu.get_flag_z(), false);
        assert_eq!(cpu.get_flag_c(), false);
        assert_eq!(cpu.get_flag_v(), false);

        // Shift to zero
        cpu.reg_cpsr = 0x00000000;
        cpu.write_register(1, 0x2);
        cpu.execute_thumb(opcode_lsr);
        assert_eq!(cpu.read_register(0), 0x2 >> offset);
        assert_eq!(cpu.get_flag_n(), false);
        assert_eq!(cpu.get_flag_z(), true);
        assert_eq!(cpu.get_flag_c(), true);
        assert_eq!(cpu.get_flag_v(), false);

        cpu.reg_cpsr = 0x00000000;
        cpu.write_register(1, rs);
        cpu.execute_thumb(opcode_lsr);
        assert_eq!(cpu.read_register(0), rs >> offset);
        assert_eq!(cpu.get_flag_n(), false);
        assert_eq!(cpu.get_flag_z(), false);
        assert_eq!(cpu.get_flag_c(), false);
        assert_eq!(cpu.get_flag_v(), false);

        cpu.reg_cpsr = 0x00000000;
        cpu.write_register(1, rs);
        cpu.execute_thumb(opcode_asr);
        assert_eq!(cpu.read_register(0), rs >> offset);
        assert_eq!(cpu.get_flag_n(), false);
        assert_eq!(cpu.get_flag_z(), false);
        assert_eq!(cpu.get_flag_c(), false);
        assert_eq!(cpu.get_flag_v(), false);

        // ASR signed
        cpu.reg_cpsr = 0x00000000;
        cpu.write_register(1, rs_signed);
        cpu.execute_thumb(opcode_asr);
        assert_eq!(cpu.read_register(0), ((rs_signed as i32) >> 2) as u32);
        assert_eq!(cpu.get_flag_n(), true);
        assert_eq!(cpu.get_flag_z(), false);
        assert_eq!(cpu.get_flag_c(), false);
        assert_eq!(cpu.get_flag_v(), false);
    }

    /// Thumb Format2
    #[test]
    fn thumb_add_subtract() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        let rd = 0;
        let rs = 8;
        let rn = 4;
        let imm: u16 = 2 << 6;

        // Rd=0, Rs=1, Rn=2, Imm = 2
        let opcode_reg_add: u16 = 0b0001100010001000;
        let opcode_imm_add: u16 = 0b0001110000001000 | imm;
        let opcode_reg_sub: u16 = 0b0001101010001000;
        let opcode_imm_sub: u16 = 0b0001111010001000 | imm;

        cpu.write_register(0, rd);
        cpu.write_register(1, rs);
        cpu.write_register(2, rn);
        cpu.execute_thumb(opcode_reg_add);
        assert!(cpu.read_register(0) == 12);
        assert!(cpu.read_register(1) == rs);
        assert!(cpu.read_register(2) == rn);

        cpu.write_register(0, rd);
        cpu.write_register(1, rs);
        cpu.write_register(2, rn);
        cpu.execute_thumb(opcode_imm_add);
        assert!(cpu.read_register(0) == 10);
        assert!(cpu.read_register(1) == rs);
        assert!(cpu.read_register(2) == rn);

        cpu.write_register(0, rd);
        cpu.write_register(1, rs);
        cpu.write_register(2, rn);
        cpu.execute_thumb(opcode_reg_sub);
        assert!(cpu.read_register(0) == 4);
        assert!(cpu.read_register(1) == rs);
        assert!(cpu.read_register(2) == rn);

        cpu.write_register(0, rd);
        cpu.write_register(1, rs);
        cpu.write_register(2, rn);
        cpu.execute_thumb(opcode_imm_sub);
        assert!(cpu.read_register(0) == 6);
        assert!(cpu.read_register(1) == rs);
        assert!(cpu.read_register(2) == rn);

        // Status checks Negative
        cpu.write_register(1, 0x80000001);
        cpu.write_register(2, 1);
        cpu.reg_cpsr = 0x0;
        cpu.execute_thumb(opcode_reg_add);
        assert_eq!(cpu.get_flag_n(), true);
        assert_eq!(cpu.get_flag_z(), false);
        assert_eq!(cpu.get_flag_c(), false);
        assert_eq!(cpu.get_flag_v(), false);

        // Status checks Zero
        cpu.write_register(1, 0);
        cpu.write_register(2, 0);
        cpu.reg_cpsr = 0x0;
        cpu.execute_thumb(opcode_reg_add);
        assert_eq!(cpu.get_flag_n(), false);
        assert_eq!(cpu.get_flag_z(), true);
        assert_eq!(cpu.get_flag_c(), false);
        assert_eq!(cpu.get_flag_v(), false);

        // Carry & Overflow
        cpu.write_register(1, 0xFFFFFFFF);
        cpu.write_register(2, 1);
        cpu.reg_cpsr = 0x0;
        cpu.execute_thumb(opcode_reg_add);
        assert_eq!(cpu.get_flag_c(), true);
        assert_eq!(cpu.get_flag_v(), true);
    }

    /// Thumb Format3
    #[test]
    fn thumb_mov_cmp_add_sub_imm() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        // Rd => 0, Offset => 8
        let offset = 8;
        let rd = 24;

        let opcode_mov = 0x2000;
        let opcode_cmp = 0x2800 | offset;
        let opcode_add = 0x3000;
        let opcode_sub = 0x3800 | offset;

        // MOV
        cpu.write_register(0, rd);
        cpu.reg_cpsr = 0;
        cpu.execute_thumb(opcode_mov | 8);
        assert_eq!(cpu.read_register(0), 8);
        assert!(!cpu.get_flag_n());
        assert!(!cpu.get_flag_z());
        assert!(!cpu.get_flag_c());
        assert!(!cpu.get_flag_v());

        // MOV zero
        cpu.write_register(0, rd);
        cpu.reg_cpsr = 0;
        cpu.execute_thumb(opcode_mov | 0);
        assert_eq!(cpu.read_register(0), 0);
        assert!(!cpu.get_flag_n());
        assert!(cpu.get_flag_z());
        assert!(!cpu.get_flag_c());
        assert!(!cpu.get_flag_v());

        // CMP

        // ADD
        cpu.write_register(0, rd);
        cpu.reg_cpsr = 0;
        cpu.execute_thumb(opcode_add | 8);
        assert_eq!(cpu.read_register(0), rd + 8);
        assert!(!cpu.get_flag_n());
        assert!(!cpu.get_flag_z());
        assert!(!cpu.get_flag_c());
        assert!(!cpu.get_flag_v());

        // ADD zero
        cpu.write_register(0, 0);
        cpu.reg_cpsr = 0;
        cpu.execute_thumb(opcode_add | 0);
        assert_eq!(cpu.read_register(0), 0);
        assert!(!cpu.get_flag_n());
        assert!(cpu.get_flag_z());
        assert!(!cpu.get_flag_c());
        assert!(!cpu.get_flag_v());

        // ADD negative & overflow
        cpu.write_register(0, 0x7FFFFFFF);
        cpu.reg_cpsr = 0;
        cpu.execute_thumb(opcode_add | 1);
        assert_eq!(cpu.read_register(0), 0x80000000);
        assert!(cpu.get_flag_n());
        assert!(!cpu.get_flag_z());
        assert!(!cpu.get_flag_c());
        assert!(cpu.get_flag_v());

        // ADD zero, carry
        cpu.write_register(0, 0xFFFFFFFF);
        cpu.reg_cpsr = 0;
        cpu.execute_thumb(opcode_add | 1);
        assert_eq!(cpu.read_register(0), 0x0);
        assert!(cpu.get_flag_z());
        assert!(cpu.get_flag_c());

        // SUB
    }

    /// Thumb Format4
    #[ignore]
    #[test]
    fn thumb_alu() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        // TST R0, R1 (equal)
        let opcode_tst_equal = 0b0100_00_1000_001_000;
        cpu.write_register(0, 0x10);
        cpu.write_register(1, 0x10);
        cpu.thumb_alu(opcode_tst_equal);
        assert!(!cpu.get_flag_n());
        assert!(cpu.get_flag_z());
        assert!(!cpu.get_flag_c());
        assert!(!cpu.get_flag_v());
    }

    #[test]
    fn alu() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        // TST
        let (_, n, z, _, _) = cpu.alu(ALU_TST, 0x10, 0x10);
        assert!(!n);
        assert!(!z);

        let (_, n, z, _, _) = cpu.alu(ALU_TST, 0x10, 0x20);
        assert!(!n);
        assert!(z);

        let (_, n, z, _, _) = cpu.alu(ALU_TST, 0x30, 0x10);
        assert!(!n);
        assert!(!z);

        // TODO: TEQ
        let (_, n, z, _, _) = cpu.alu(ALU_TEQ, 0x10, 0x10);
        assert!(!n);
        assert!(z);

        let (_, n, z, _, _) = cpu.alu(ALU_TEQ, 0x10, 0x20);
        assert!(!n);
        assert!(!z);

        let (_, n, z, _, _) = cpu.alu(ALU_TEQ, 0x10, 0x30);
        assert!(!n);
        assert!(!z);

        // CMP
        let (_, n, z, c, v) = cpu.alu(ALU_CMP, 5, 10);
        assert!(n);
        assert!(!z);
        assert!(!c);
        assert!(!v);

        let (_, n, z, c, v) = cpu.alu(ALU_CMP, 10, 5);
        assert!(!n);
        assert!(!z);
        assert!(c);
        assert!(!v);

        let (_, n, z, c, v) = cpu.alu(ALU_CMP, 5, 5);
        assert!(!n);
        assert!(z);
        assert!(c);
        assert!(!v);

        // CMN
        let (_, n, z, c, v) = cpu.alu(ALU_CMN, 5, 10);
        assert!(!n);
        assert!(!z);
        assert!(!c);
        assert!(!v);

        let (_, n, z, c, v) = cpu.alu(ALU_CMN, 10, 5);
        assert!(!n);
        assert!(!z);
        assert!(!c);
        assert!(!v);

        let (_, n, z, c, v) = cpu.alu(ALU_CMN, 5, 5);
        assert!(!n);
        assert!(!z);
        assert!(!c);
        assert!(!v);

        let op1 = -5;
        let (_, n, z, c, v) = cpu.alu(ALU_CMN, op1 as u32, 5);
        assert!(!n);
        assert!(z);
        assert!(c);
        assert!(!v);

        let op1 = -5;
        let (_, n, z, c, v) = cpu.alu(ALU_CMN, op1 as u32, op1 as u32);
        assert!(n);
        assert!(!z);
        assert!(c);
        assert!(!v);

        cpu.set_flag_n(true);
        cpu.set_flag_z(true);
        cpu.set_flag_c(true);
        cpu.set_flag_v(true);
        let (_, n, z, c, v) = cpu.alu(ALU_CMP, 0x3E, 0x00);
        assert!(!n);
        assert!(!z);
        assert!(c);
        assert!(!v);
    }

    /// Thumb Format5
    #[test]
    fn thumb_hi_register_op_bx() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        let opcode_bx_low = 0b0100011100000000;
        let opcode_bx_high = 0b0100011101000000;

        // BX Rs (0)
        let addr_thumb = 0x4D500001;

        cpu.write_register(0, addr_thumb);
        cpu.execute_thumb(opcode_bx_low);
        assert_eq!(cpu.get_program_counter(), addr_thumb & 0xFFFFFFFE);
        assert!(cpu.is_thumb());

        let addr_arm = 0x4D500000;
        cpu.write_register(0, addr_arm);
        cpu.execute_thumb(opcode_bx_low);
        assert_eq!(cpu.get_program_counter(), addr_arm & 0xFFFFFFFE);
        assert!(!cpu.is_thumb());

        // BX Hs (8)
        let addr_thumb = 0x4D500001;

        cpu.write_register(8, addr_thumb);
        cpu.execute_thumb(opcode_bx_high);
        assert_eq!(cpu.get_program_counter(), addr_thumb & 0xFFFFFFFE);
        assert!(cpu.is_thumb());

        let addr_arm = 0x4D500000;
        cpu.write_register(8, addr_arm);
        cpu.execute_thumb(opcode_bx_high);
        assert_eq!(cpu.get_program_counter(), addr_arm & 0xFFFFFFFE);
        assert!(!cpu.is_thumb());
    }

    /// Thumb Format6
    #[test]
    fn thumb_pc_relative_load() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        let offset = 8;
        let opcode = 0x4800 | ((offset as u16) >> 2);
        let val = 0xDEADBEEF;

        cpu.reg_cpsr = 0x4D504D50;
        cpu.set_program_counter(0x02000000);
        cpu.write_u32(false, 0x02000000 + offset + 4, val);
        cpu.execute_thumb(opcode);
        assert_eq!(cpu.read_u32(false, 0x02000000 + offset + 4), val);
        assert_eq!(cpu.read_register(0), val);
        assert_eq!(cpu.reg_cpsr, 0x4D504D50);
    }

    /// Thumb Format7
    #[test]
    fn thumb_load_store_register_offset() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        // Rd => 0, Rb => 1, Ro => 2
        let rb = 0x03000000;
        let rd = 0xDEADBEEF;
        let ro = 0x8;
        let addr = rb + ro;

        let opcode_str = 0x5088;
        let opcode_strb = 0x5488;
        let opcode_ldr = 0x5888;
        let opcode_ldrb = 0x5C88;

        // STR
        cpu.write_register(0, rd);
        cpu.write_register(1, rb);
        cpu.write_register(2, ro);
        cpu.execute_thumb(opcode_str);
        assert_eq!(cpu.read_u32(false, addr), rd);
        cpu.write_u32(false, addr, 0);

        // STRB
        cpu.write_register(0, rd);
        cpu.write_register(1, rb);
        cpu.write_register(2, ro);
        cpu.execute_thumb(opcode_strb);
        assert_eq!(cpu.read_u8(false, addr), (rd & 0xFF) as u8);

        // LDR
        cpu.write_register(0, 0);
        cpu.write_register(1, rb);
        cpu.write_register(2, ro);
        cpu.write_u32(false, addr, rd);
        cpu.execute_thumb(opcode_ldr);
        assert_eq!(cpu.read_register(0), rd);

        // LDRB
        cpu.write_register(0, 0);
        cpu.write_register(1, rb);
        cpu.write_register(2, ro);
        cpu.write_u8(false, addr, (rd & 0xFF) as u8);
        cpu.execute_thumb(opcode_ldrb);
        assert_eq!(cpu.read_register(0), rd & 0xFF);
    }

    /// Thumb Format8
    #[test]
    fn thumb_load_store_sign_extended_byte_halfword() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        // Rd => 0
        // Ro => 1
        // Rb => 2
        let rd = 0xDEADBEEF;
        let rd_sbh = 0b1001000110100_10101010_11001101;
        let ro = 0x04;
        let rb = 0x03000000;

        let opcode_strh = 0x5250;
        let opcode_ldrh = 0x5A50;
        let opcode_ldsb = 0x5650;
        let opcode_ldsh = 0x5E50;

        // STRH R0,[R2,R1]
        cpu.write_register(0, rd);
        cpu.write_register(1, ro);
        cpu.write_register(2, rb);
        cpu.execute_thumb(opcode_strh);
        assert_eq!(cpu.read_u32(false, rb + ro), rd & 0xFFFF);

        // LDRH R0,[R2,R1]
        cpu.write_register(0, 0);
        cpu.write_register(1, ro);
        cpu.write_register(2, rb);
        cpu.write_u32(false, rb + ro, rd);
        cpu.execute_thumb(opcode_ldrh);
        assert_eq!(cpu.read_register(0), rd & 0xFFFF);

        // LDSB R0,[R2,R1]
        cpu.write_register(0, 0);
        cpu.write_register(1, ro);
        cpu.write_register(2, rb);
        cpu.write_u32(false, rb + ro, rd_sbh);
        cpu.execute_thumb(opcode_ldsb);
        assert_eq!(cpu.read_register(0), (rd_sbh & 0xFF) | 0xFFFFFF00);

        // LDSH R0,[R2,R1]
        cpu.write_register(0, 0);
        cpu.write_register(1, ro);
        cpu.write_register(2, rb);
        cpu.write_u32(false, rb + ro, rd_sbh);
        cpu.execute_thumb(opcode_ldsh);
        assert_eq!(cpu.read_register(0), (rd_sbh & 0xFFFF) | 0xFFFF0000);
    }

    /// Thumb Format9
    #[test]
    fn thumb_load_store_immediate() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        let rd: u32 = 2;
        let rb: u32 = 0x02000000;
        let offset: u16 = 8;
        let val: u32 = 0xdeadbeef;
        let val8: u8 = 0x4D;

        // Rd=0, Rb=1, Offset=10
        let opcode_str: u16 = 0b0110000000001000 | ((offset >> 2) << 6);
        let opcode_ldr: u16 = 0b0110100000001000 | ((offset >> 2) << 6);
        let opcode_strb: u16 = 0b0111000000001000 | (offset << 6);
        let opcode_ldrb: u16 = 0b0111100000001000 | (offset << 6);

        cpu.write_register(0, rd);
        cpu.write_register(1, rb);
        cpu.write_u32(false, rb + offset as u32, val);
        cpu.execute_thumb(opcode_str);
        assert!(cpu.read_u32(false, rb + offset as u32) == rd);
        assert!(cpu.read_register(0) == rd);
        assert!(cpu.read_register(1) == rb);

        cpu.write_register(0, rd);
        cpu.write_register(1, rb);
        cpu.write_u32(false, rb + offset as u32, val);
        cpu.execute_thumb(opcode_ldr);
        assert!(cpu.read_register(0) == val);
        assert!(cpu.read_register(1) == rb);

        cpu.write_register(0, rd);
        cpu.write_register(1, rb);
        cpu.write_u8(false, rb + offset as u32, val8);
        cpu.execute_thumb(opcode_strb);
        assert!(cpu.read_u8(false, rb + offset as u32) == (rd as u8));
        assert!(cpu.read_register(0) == rd);
        assert!(cpu.read_register(1) == rb);

        cpu.write_register(0, rd);
        cpu.write_register(1, rb);
        cpu.write_u8(false, rb + offset as u32, val8);
        cpu.execute_thumb(opcode_ldrb);
        assert!(cpu.read_u8(false, rb + offset as u32) == val8);
        assert!(cpu.read_register(0) == (val8 as u32));
        assert!(cpu.read_register(1) == rb);
    }

    /// Thumb Format10
    #[test]
    fn thumb_load_store_halfword() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        // Rd => 0, Rb => 1, Imm => 8
        let rd = 0xDEADBEEF;
        let rb = 0x03000000;
        let imm = 8;
        let addr = rb + (imm as u32);

        let opcode_strh = 0x8008 | ((imm >> 1) << 6);
        let opcode_ldrh = 0x8808 | ((imm >> 1) << 6);

        // STRH
        cpu.write_register(0, rd);
        cpu.write_register(1, rb);
        cpu.execute_thumb(opcode_strh);
        assert_eq!(cpu.read_u32(false, addr), rd & 0xFFFF);

        // LDRH
        cpu.write_register(0, 0);
        cpu.write_register(1, rb);
        cpu.write_u32(false, addr, rd);
        cpu.execute_thumb(opcode_ldrh);
        assert_eq!(cpu.read_register(0), rd & 0xFFFF);
    }

    /// Thumb Format11
    #[test]
    fn thumb_sp_relative_load_store() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        let rd = 0xDEADBEEF;
        let sp = 0x03000000;
        let offset = 8;
        let opcode_str = 0x9000 | (offset >> 2);
        let opcode_ldr = 0x9800 | (offset >> 2);

        let offset = offset as u32;

        // STR
        cpu.write_register(13, sp);
        cpu.write_register(0, rd);
        cpu.execute_thumb(opcode_str);
        assert_eq!(cpu.read_u32(false, sp + offset), rd);

        // LDR
        cpu.write_register(13, sp);
        cpu.write_register(0, 0);
        cpu.execute_thumb(opcode_ldr);
        assert_eq!(cpu.read_register(0), rd);
    }

    // Thumb Format12
    #[test]
    fn thumb_load_address() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        // rd=0, word8=16
        let val = 16;
        let opcode_pc = 0xA000 | (val >> 2);
        let opcode_sp = 0xA800 | (val >> 2);

        let val = val as u32;
        // PC is offset at 4
        cpu.set_program_counter(0x0000);

        cpu.write_register(0, 0x0);
        cpu.execute_thumb(opcode_pc);
        assert_eq!(cpu.read_register(0), (val + 4));

        // Bit 1 is forced to 0 when using PC
        cpu.set_program_counter(0x0002);
        cpu.write_register(0, 0x0);
        cpu.execute_thumb(opcode_pc);
        assert_eq!(cpu.read_register(0), (val + 4));

        // SP check
        cpu.write_register(13, 0x1000);
        cpu.write_register(0, 0x0);
        cpu.execute_thumb(opcode_sp);
        assert_eq!(cpu.read_register(0), (0x1000 + val));
    }

    /// Thumb Format13
    #[test]
    fn thumb_offset_to_sp() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        let imm = 16;
        let opcode_add = 0xB000 | (imm >> 2);
        let opcode_sub = 0xB080 | (imm >> 2);

        let cpsr = 0xDEADBEEF;
        cpu.reg_cpsr = cpsr;

        cpu.write_register(13, 0);
        cpu.execute_thumb(opcode_add);
        assert_eq!(cpu.read_register(13), 16);
        assert_eq!(cpu.reg_cpsr, cpsr);

        cpu.write_register(13, 16);
        cpu.execute_thumb(opcode_sub);
        assert_eq!(cpu.read_register(13), 0);
        assert_eq!(cpu.reg_cpsr, cpsr);
    }

    // Thumb Format14
    #[test]
    fn thumb_push_pop() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        let base = 0x03000000;

        // Push/pop R0 & R2
        let opcode_push = 0b1011010000000000 | (1 << 0) | (1 << 2);
        let opcode_push_lr = 0b1011010100000000 | (1 << 0) | (1 << 2);
        let opcode_pop = 0b1011110000000000 | (1 << 0) | (1 << 2);
        let opcode_pop_lr = 0b1011110100000000 | (1 << 0) | (1 << 2);

        let r0 = 0x0123;
        let r1 = 0x4567;
        let r2 = 0x89ab;
        let rl = 0xdeadbeef;
        cpu.write_register(0, r0);
        cpu.write_register(1, r1);
        cpu.write_register(2, r2);
        cpu.write_register(14, rl);

        // Push {R0, R2}
        cpu.write_register(13, base | 16);
        cpu.execute_thumb(opcode_push);
        assert_eq!(cpu.read_register(13), base | 8);
        assert_eq!(cpu.read_u32(false, base | 8), r0);
        assert_eq!(cpu.read_u32(false, base | 12), r2);

        // Pop {R2, R0}
        cpu.write_register(0, 0);
        cpu.write_register(1, 0);
        cpu.write_register(2, 0);
        cpu.execute_thumb(opcode_pop);
        assert_eq!(cpu.read_register(0), r0);
        assert_eq!(cpu.read_register(1), 0);
        assert_eq!(cpu.read_register(2), r2);

        //// Push {R0, R2, LR}
        cpu.write_register(0, r0);
        cpu.write_register(1, r1);
        cpu.write_register(2, r2);
        cpu.write_register(14, rl);

        cpu.write_u32(false, base, 0);
        cpu.write_u32(false, base | 4, 0);
        cpu.write_u32(false, base | 8, 0);
        cpu.write_u32(false, base | 12, 0);
        cpu.write_u32(false, base | 16, 0);
        cpu.write_register(13, base | 16);
        cpu.execute_thumb(opcode_push_lr);
        assert_eq!(cpu.read_register(13), base + 4);
        assert_eq!(cpu.read_u32(false, base + 4), r0);
        assert_eq!(cpu.read_u32(false, base + 8), r2);
        assert_eq!(cpu.read_u32(false, base + 12), rl);

        //// Pop {R2, R0, PC}
        cpu.write_register(0, 0);
        cpu.write_register(1, 0);
        cpu.write_register(2, 0);
        cpu.set_program_counter(0);
        cpu.execute_thumb(opcode_pop_lr);
        assert_eq!(cpu.read_register(0), r0);
        assert_eq!(cpu.read_register(1), 0);
        assert_eq!(cpu.read_register(2), r2);
        //assert_eq!(cpu.get_program_counter(), rl);
    }

    /// Thumb Format15
    #[test]
    fn thumb_multiple_load_store() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        // STMIA/LDMIA R3!,{R0, R2}
        let rb = 0x03000000;
        let r0 = 0x1234;
        let r2 = 0x9abc;
        let opcode_store = 0xC300 | (1 << 0) | (1 << 2);
        let opcode_load = 0xCB00 | (1 << 0) | (1 << 2);

        // STMIA R3!,{R0, R2}
        cpu.write_register(0, r0);
        cpu.write_register(2, r2);
        cpu.write_register(3, rb);
        cpu.execute_thumb(opcode_store);
        assert_eq!(cpu.read_u32(false, rb), r0);
        assert_eq!(cpu.read_u32(false, rb + 4), r2);
        assert_eq!(cpu.read_register(3), rb + 8);

        // STMIA R3!,{R0, R2}
        cpu.write_register(0, 0);
        cpu.write_register(2, 0);
        cpu.write_register(3, rb);
        cpu.execute_thumb(opcode_load);
        assert_eq!(cpu.read_register(0), r0);
        assert_eq!(cpu.read_register(2), r2);
        assert_eq!(cpu.read_register(3), rb + 8);
    }

    /// Thumb Format16
    #[test]
    fn thumb_conditional_branch() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);
        cpu.set_thumb(true);

        let opcode_bne = 0xD1FC;
        let pc = 0x08000194;

        // Take negative
        cpu.set_flag_z(false);
        cpu.set_program_counter(pc);
        cpu.execute_thumb(opcode_bne);
        assert_eq!(cpu.get_program_counter(), pc - 8 + 4);

        // Skip negative
        cpu.set_flag_z(true);
        cpu.set_program_counter(pc);
        cpu.execute_thumb(opcode_bne);
        assert_eq!(cpu.get_program_counter(), pc + 2);

        let opcode_bne = 0xD104;

        // Take positive
        cpu.set_flag_z(false);
        cpu.set_program_counter(pc);
        cpu.execute_thumb(opcode_bne);
        assert_eq!(cpu.get_program_counter(), pc + 8 + 4);

        // Skip positive
        cpu.set_flag_z(true);
        cpu.set_program_counter(pc);
        cpu.execute_thumb(opcode_bne);
        assert_eq!(cpu.get_program_counter(), pc + 2);
    }

    /// Thumb Format17
    #[ignore]
    #[test]
    fn thumb_swi() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);
    }

    /// Thumb Format18
    #[test]
    fn thumb_unconditional_branch() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        // Forward
        let offset = 8;
        let opcode = 0xE000 | (offset >> 1);

        cpu.set_program_counter(0);
        cpu.execute_thumb(opcode);
        assert_eq!(cpu.get_program_counter(), (offset as u32) + 4);

        // Backward (-8)
        let offset = 0xFF8;
        let opcode = 0xE000 | (offset >> 1);

        let pc = 1000;
        cpu.set_program_counter(pc);
        cpu.execute_thumb(opcode);
        assert_eq!(cpu.get_program_counter(), pc + 4 - 8);
    }

    /// Thumb Format19
    #[test]
    fn thumb_long_branch_link() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        // Forward
        let addr: u32 = 0x8;
        let offset = (addr >> 1) & 0x3FFFFF;
        let offset_high = ((offset >> 11) & 0x3FF) as u16;
        let offset_low = (offset & 0x3FF) as u16;

        let opcode_high = 0xF000 | offset_high;
        let opcode_low = 0xF800 | offset_low;

        let lr_val = 4 + ((offset_high as u32) << 12);

        // BL0
        cpu.set_program_counter(0);
        cpu.execute_thumb(opcode_high);
        assert_eq!(cpu.read_register(14), lr_val);
        assert_eq!(cpu.get_program_counter(), 2);

        // BL1
        cpu.execute_thumb(opcode_low);
        assert_eq!(
            cpu.get_program_counter(),
            lr_val + ((offset_low as u32) << 1)
        );
        assert_eq!(cpu.read_register(14), 4 | 1);
        assert_eq!(cpu.get_program_counter(), 4 + addr);

        // Backward (-8)
        let addr: u32 = 0x7FFFF8;
        let offset = (addr >> 1) & 0x3FFFFF;
        let offset_high = ((offset >> 11) & 0x7FF) as u16;
        let offset_low = (offset & 0x7FF) as u16;

        let opcode_high = 0xF000 | offset_high;
        let opcode_low = 0xF800 | offset_low;
        let pc = 1000;

        // BL0
        cpu.set_program_counter(pc);
        cpu.execute_thumb(opcode_high);
        assert_eq!(cpu.get_program_counter(), pc + 2);

        // BL1
        cpu.execute_thumb(opcode_low);
        assert_eq!(cpu.read_register(14), (pc + 4) | 1);
        assert_eq!(cpu.get_program_counter(), pc + 4 - 8);
    }

    /// Tests post-increment load
    #[test]
    fn operation_ldmia() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        let wb = true;
        let load = true;
        let pre = false;
        let up = true;

        let r_base = 13;
        let r_list = (1 << 2) | (1 << 0);

        let base_ptr = 0x03000000;

        let r0 = 123;
        let r2 = 789;

        // Set up base_ptr
        cpu.write_register(r_base, base_ptr);

        // Write stack
        cpu.write_u32(false, base_ptr + 0, r0);
        cpu.write_u32(false, base_ptr + 4, r2);

        cpu.operation_ldm_stm(r_base, r_list, load, wb, pre, up, false);
        assert_eq!(cpu.read_register(r_base), base_ptr + 8);
        assert_eq!(cpu.read_register(0), r0);
        assert_eq!(cpu.read_register(2), r2);
        assert_eq!(cpu.cycle_count, 2 + 2);
    }

    /// Tests pre-decrement store
    #[test]
    fn operation_stmdb() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        let wb = true;
        let load = false;
        let pre = true;
        let up = false;

        let r_base = 13;
        let r_list = (1 << 2) | (1 << 0);

        let base_ptr = 0x03000010;

        let r0 = 123;
        let r2 = 789;

        // Set up base_ptr
        cpu.write_register(r_base, base_ptr);
        cpu.write_register(0, r0);
        cpu.write_register(2, r2);

        // Write stack
        cpu.write_u32(false, base_ptr + 0, r0);
        cpu.write_u32(false, base_ptr + 4, r2);

        cpu.operation_ldm_stm(r_base, r_list, load, wb, pre, up, false);
        assert_eq!(cpu.read_register(r_base), base_ptr - 8);
        assert_eq!(cpu.read_u32(false, base_ptr - 8), r0);
        assert_eq!(cpu.read_u32(false, base_ptr - 4), r2);
        assert_eq!(cpu.cycle_count, 1 + 2);
    }

    #[test]
    fn syscall_div() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        cpu.write_register(0, (-1234 as i32) as u32);
        cpu.write_register(1, (10 as i32) as u32);
        cpu.syscall_div();
        assert_eq!(cpu.read_register(0) as i32, -123);
        assert_eq!(cpu.read_register(1) as i32, -4);
        assert_eq!(cpu.read_register(2), 123);
    }

    #[test]
    fn memcpy() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        let src: u32 = 0x02000000;
        let dest: u32 = 0x03000000;
        let val: u32 = 0xDEADBEEF;

        cpu.write_u32(false, src, val);
        cpu.memcpy(dest, src, 4);
        assert!(cpu.read_u32(false, dest) == val);
    }

    #[test]
    fn memfill32() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        let dest: u32 = 0x03000000;
        let val: u32 = 0xDEADBEEF;

        cpu.memfill32(dest, val, 2);
        assert!(cpu.read_u32(false, dest) == val);
        assert!(cpu.read_u32(false, dest + 4) == val);
        assert!(cpu.read_u32(false, dest + 8) != val);
    }

    /// Halt runs until an enabled interrupt is triggered
    #[test]
    fn syscall_halt() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        // Enable VBlank interrupt
        cpu.io_ie |= IRQ_VBLANK;

        // Enable VBlank, HBlank irq in LCD
        cpu.lcd.set_dispstat(0x0018);

        cpu.syscall_halt();
        assert!(cpu.halt);

        assert!(!cpu.can_irq_trigger(IRQ_HBLANK));
        assert!(cpu.can_irq_trigger(IRQ_VBLANK));

        cpu.trigger_irq(IRQ_VBLANK);
        assert!(!cpu.halt);
    }

    /// Halt runs until an eanbled interrupt is triggered
    #[test]
    fn syscall_vblank_intr_wait() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        assert!(!cpu.can_irq_trigger(IRQ_HBLANK));
        assert!(!cpu.can_irq_trigger(IRQ_VBLANK));

        // Enable VBlank interrupt
        cpu.io_ie |= IRQ_VBLANK;

        assert!(!cpu.can_irq_trigger(IRQ_HBLANK));
        assert!(!cpu.can_irq_trigger(IRQ_VBLANK));

        // Enable VBlank, HBlank irq in LCD
        cpu.lcd.set_dispstat(0x0018);

        assert!(!cpu.can_irq_trigger(IRQ_HBLANK));
        assert!(!cpu.can_irq_trigger(IRQ_VBLANK));

        cpu.syscall_vblank_intr_wait();
        assert!(cpu.halt);

        assert!(!cpu.can_irq_trigger(IRQ_HBLANK));
        assert!(cpu.can_irq_trigger(IRQ_VBLANK));

        cpu.trigger_irq(IRQ_VBLANK);
        assert!(!cpu.halt);
    }

    #[test]
    fn alu_operand2_calc_imm() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        let imm: u32 = 0x4D;
        let rot: u32 = 0x3;

        let (result, _) = cpu.alu_operand2_calc(true, ((rot << 8) | imm) as u16);
        assert_eq!(result, imm.rotate_right(rot * 2));
    }

    #[test]
    fn alu_operand2_calc_shift_imm() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        let rm_val = 0x4D;
        cpu.write_register(0, rm_val);

        let shift_amount = 5;
        let op_lsl = 0b00101_00_0_0000 | (shift_amount << 7);
        let op_lsr = 0b00101_01_0_0000 | (shift_amount << 7);
        let op_asr = 0b00101_10_0_0000 | (shift_amount << 7);
        let op_ror = 0b00101_11_0_0000 | (shift_amount << 7);

        // LSL R0,5
        let (result, _) = cpu.alu_operand2_calc(false, op_lsl);
        assert_eq!(result, rm_val << shift_amount);

        // LSR R0,5
        let (result, _) = cpu.alu_operand2_calc(false, op_lsr);
        assert_eq!(result, rm_val >> shift_amount);

        // ASR R0,5
        let (result, _) = cpu.alu_operand2_calc(false, op_asr);
        assert_eq!(result as i32, (rm_val as i32) >> shift_amount);

        // ROR R0,5
        let (result, _) = cpu.alu_operand2_calc(false, op_ror);
        assert_eq!(result, rm_val.rotate_right(shift_amount as u32));
    }

    #[test]
    fn alu_operand2_calc_shift_reg() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        let rm_val = 0x4D;
        let shift_amount = 5;

        cpu.write_register(0, rm_val);
        cpu.write_register(1, shift_amount);

        let op_lsl = 0b0001_0_00_1_0000;
        let op_lsr = 0b0001_0_01_1_0000;
        let op_asr = 0b0001_0_10_1_0000;
        let op_ror = 0b0001_0_11_1_0000;

        // LSL R0,R1
        let (result, _) = cpu.alu_operand2_calc(false, op_lsl);
        assert_eq!(result, rm_val << shift_amount);

        // LSR R0,R1
        let (result, _) = cpu.alu_operand2_calc(false, op_lsr);
        assert_eq!(result, rm_val >> shift_amount);

        // ASR R0,R1
        let (result, _) = cpu.alu_operand2_calc(false, op_asr);
        assert_eq!(result as i32, (rm_val as i32) >> shift_amount);

        // ROR R0,R1
        let (result, _) = cpu.alu_operand2_calc(false, op_ror);
        assert_eq!(result, rm_val.rotate_right(shift_amount as u32));
    }

    #[test]
    fn arm_multiply() {
        let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
        let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));
        let mut cpu = CPU::new(&vram, &palette, &oam);

        let rd = 0;
        let rm = 1;
        let rs = 2;
        let rn = 3;

        let opcode_mul = 0xE0100090
            | ((rd as u32) << 16)
            | ((rn as u32) << 12)
            | ((rs as u32) << 8)
            | (rm as u32);
        let opcode_mla = 0xE0300090
            | ((rd as u32) << 16)
            | ((rn as u32) << 12)
            | ((rs as u32) << 8)
            | (rm as u32);

        // MUL Rd, Rm, Rs (-10 * 20 => -200)
        cpu.write_register(rd, 0);
        cpu.write_register(rm, 0xFFFFFFF6);
        cpu.write_register(rs, 0x00000014);
        cpu.write_register(rn, 1);
        cpu.execute_arm(opcode_mul);
        assert_eq!(cpu.read_register(rd), 0xFFFFFF38);

        // MLA Rd, Rm, Rs, Rn (-10 * 20 + 1 => -199);
        cpu.write_register(rd, 0);
        cpu.write_register(rm, 0xFFFFFFF6);
        cpu.write_register(rs, 0x00000014);
        cpu.write_register(rn, 1);
        cpu.execute_arm(opcode_mla);
        assert_eq!(cpu.read_register(rd), 0xFFFFFF39);
    }
}
