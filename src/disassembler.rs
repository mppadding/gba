use log::debug;

use crate::cpu::*;

pub fn disassemble_arm(opcode: u32, pc: u32) -> (String, String) {
    let instr = ((opcode >> 20) & 0xFF) as u8;
    let cond = ((opcode >> 28) & 0xF) as u8;

    match instr {
        0x00..=0x3F => {
            if CPU::opcode_match(opcode, ARM_MASK_MUL_CLR, ARM_MASK_MUL_SET) {
                ("Multiply".to_string(), "???".to_string())
            } else if CPU::opcode_match(opcode, ARM_MASK_MUL_LONG_CLR, ARM_MASK_MUL_LONG_SET) {
                ("Multiply Long".to_string(), "???".to_string())
            } else if CPU::opcode_match(opcode, ARM_MASK_SNGL_SWP_CLR, ARM_MASK_SNGL_SWP_SET) {
                ("Single Data Swap".to_string(), "???".to_string())
            } else if CPU::opcode_match(opcode, ARM_MASK_BX_CLR, ARM_MASK_BX_SET) {
                let rm = (opcode & 0xF) as u8;
                (
                    format!("BX R{}", rm),
                    "COND ---- ---- ---- ---- ---- ---- Rn__".to_string(),
                )
            } else if CPU::opcode_match(opcode, ARM_MASK_HW_REG_CLR, ARM_MASK_HW_REG_SET) {
                (
                    "Halfword Data Transfer: register offset".to_string(),
                    "???".to_string(),
                )
            } else if CPU::opcode_match(opcode, ARM_MASK_MRS_CLR, ARM_MASK_MRS_SET) {
                let bits = "COND ---- -P-- ---- Rd__  ---- ---- ----".to_string();
                ("MRS".to_string(), bits)
            } else if CPU::opcode_match(opcode, ARM_MASK_MSR_CLR, ARM_MASK_MSR_SET) {
                let dest_spsr = (opcode & 0x400000) != 0;
                let rm = (opcode & 0xF) as usize;

                let bits = "COND ---- -P-- ---- ---- ---- ---- Rm__".to_string();

                match dest_spsr {
                    false => (format!("MSR CPSR,R{}", rm), bits),
                    true => (format!("MSR SPSR,R{}", rm), bits),
                }
            } else if CPU::opcode_match(opcode, ARM_MASK_MSR_BITS_CLR, ARM_MASK_MSR_BITS_SET) {
                ("MSR bits".to_string(), "???".to_string())
            } else {
                let rd = ((opcode >> 12) & 0xF) as u8;
                let rn = ((opcode >> 16) & 0xF) as u8;
                let set_condition = (opcode & 0x100000) != 0;
                let op = ((opcode >> 21) & 0xF) as u8;
                let i = (opcode & 0x2000000) != 0;

                let s_str = match set_condition {
                    false => "",
                    true => "S",
                };

                let mnemonic = match op {
                    0x0 => format!("AND{} R{},R{},???", s_str, rd, rn),
                    0x1 => format!("EOR{} R{},R{},???", s_str, rd, rn),
                    0x2 => format!("SUB{} R{},R{},???", s_str, rd, rn),
                    0x3 => format!("RSB{} R{},R{},???", s_str, rd, rn),
                    0x4 => format!("ADD{} R{},R{},???", s_str, rd, rn),
                    0x5 => format!("ADC{} R{},R{},???", s_str, rd, rn),
                    0x6 => format!("SBC{} R{},R{},???", s_str, rd, rn),
                    0x7 => format!("RSC{} R{},R{},???", s_str, rd, rn),
                    0x8 => format!("TST{} R{},???", s_str, rn),
                    0x9 => format!("TEQ{} R{},???", s_str, rn),
                    0xA => format!("CMP{} R{},???", s_str, rn),
                    0xB => format!("CMN{} R{},???", s_str, rn),
                    0xC => format!("ORR{} R{},R{},???", s_str, rd, rn),
                    0xD => format!("MOV{} R{},???", s_str, rd),
                    0xE => format!("BIC{} R{},R{},???", s_str, rd, rn),
                    0xF => format!("MVN{} R{},???", s_str, rd),
                    _ => unreachable!(""),
                };
                let bits = "COND --IOpcodS Rn__ Rd__ Operand2______".to_string();

                (mnemonic, bits)
            }
        }
        0x40..=0x7F => {
            if CPU::opcode_match(opcode, ARM_MASK_UNDEF_CLR, ARM_MASK_UNDEF_SET) {
                ("???".to_string(), "???".to_string())
            } else {
                let mut offset = (opcode & 0xFFF);
                let rd = (opcode >> 12) & 0xF;
                let rn = (opcode >> 16) & 0xF;

                let load = (opcode & (1 << 20)) != 0;
                let write_back = (opcode & (1 << 21)) != 0;
                let byte = (opcode & (1 << 22)) != 0;
                let add = (opcode & (1 << 23)) != 0;
                let pre_index = (opcode & (1 << 24)) != 0;
                let imm = (opcode & (1 << 25)) != 0;

                let mut res = String::new();

                match load {
                    false => res.push_str("STR"),
                    true => res.push_str("LDR"),
                }

                if byte {
                    res.push_str("B");
                }

                if write_back {
                    res.push_str("T");
                }

                res.push_str(format!(" R{},", rd).as_str());

                if rn == 15 {
                    let val = match add {
                        false => pc.wrapping_sub(offset),
                        true => pc.wrapping_add(offset),
                    };

                    res.push_str(format!("0x{:X}", val + 8).as_str());
                } else {
                    res.push_str("???");
                }

                (res, "COND --IP UBWL _Rn_ _Rd_ ____Offset____".to_string())
            }
        }
        0x80..=0x9F => {
            let rlist = (opcode & 0xFFFF);
            let rn = ((opcode >> 16) & 0xF) as u8;
            let load = (opcode & 0x100000) != 0;
            let write_back = (opcode & 0x200000) != 0;
            let psr = (opcode & 0x400000) != 0;
            let up = (opcode & 0x800000) != 0;
            let pre = (opcode & 0x1000000) != 0;

            let mut mnemonic = match load {
                false => String::from("STM"),
                true => String::from("LDM"),
            };

            let mut mnemonic = match (load, pre, up) {
                (true, true, true) => String::from("LDMED"),
                (true, false, true) => String::from("LDMFD"),
                (true, true, false) => String::from("LDMEA"),
                (true, false, false) => String::from("LDMFA"),
                (false, true, true) => String::from("STMFA"),
                (false, false, true) => String::from("STMEA"),
                (false, true, false) => String::from("STMFD"),
                (false, false, false) => String::from("STMED"),
            };

            (
                mnemonic,
                "COND ---P USWL Rn__ Rlist______________".to_string(),
            )
        }
        0xA0..=0xBF => {
            let link = (opcode & 0x01000000) != 0;
            let mut offset = (opcode & 0xFFFFFF) << 2;

            // Sign extend
            if offset >= 0x02000000 {
                offset |= 0xFC000000;
            }

            // Current pc + 8 bytes for prefetch
            let target = pc + 8;
            let target = target.wrapping_add(offset);

            let res = match link {
                true => format!("BL 0x{:08X}", target),
                false => format!("B 0x{:08X}", target),
            };

            (res, "COND ---L ____________Offset___________".to_string())
        }
        0xC0..=0xDF => ("Coprocessor data transfer".to_string(), "???".to_string()),
        0xE0..=0xEF => {
            if (opcode & 0x10) == 0 {
                ("Coprocessor data operation".to_string(), "???".to_string())
            } else {
                (
                    "Coprocessor register transfer".to_string(),
                    "???".to_string(),
                )
            }
        }
        0xF0..=0xFF => ("Software Interrupt".to_string(), "???".to_string()),
        _ => ("???".to_string(), "???".to_string()),
    }
}

pub fn disassemble_thumb(opcode: u32) -> (String, String) {
    let high = (opcode >> 8) as u8;

    match high {
        0x00..=0x17 => ("Move Shifted Register".to_string(), "???".to_string()), // Move shifted register
        0x18..=0x1F => {
            let rd = (opcode & 0x7) as u8;
            let rs = ((opcode >> 3) & 0x7) as u8;
            let offset = ((opcode >> 6) & 0x7) as u8;
            let sub = (opcode & 0x200) != 0;
            let imm = (opcode & 0x400) != 0;

            (
                match (sub, imm) {
                    (false, false) => format!("ADD R{},R{},R{}", rd, rs, offset),
                    (false, true) => format!("ADD R{},R{},#{}", rd, rs, offset),
                    (true, false) => format!("SUB R{},R{},R{}", rd, rs, offset),
                    (true, true) => format!("SUB R{},R{},#{}", rd, rs, offset),
                },
                "---- -IORn__Rs__Rd_".to_string(),
            )
        } // Add/subtract
        0x20..=0x3F => {
            let bits = "---O PRd_ Offset___".to_string();
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
        } // Move/compare/add/subtract immediate
        0x40..=0x43 => ("ALU operations".to_string(), "???".to_string()),        // ALU operations
        0x44..=0x47 => {
            let rd = opcode & 0x7;
            let rs = (opcode >> 3) & 0x7;
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

            (
                //"Hi register operations/branch exchange".to_string(),
                match op {
                    0b00 => format!("ADD R{},R{}", rd, rs),
                    0b01 => format!("CMP R{},R{}", rd, rs),
                    0b10 => format!("MOV R{},R{}", rd, rs),
                    0b11 => format!("BX R{},R{}", rd, rs),
                    _ => unreachable!(),
                },
                "---- --OP 12Rs__Rd_".to_string(),
            )
        } // Hi register operations/branch exchange
        0x48..=0x4F => {
            let bits = "---- -Rd_ Word_____".to_string();

            let word = (opcode & 0xFF) << 2;
            let rd = (opcode >> 8) & 0x7;

            (format!("LDR R{},[PC,#0x{:02X}]", rd, word), bits)
        }
        0x50 | 0x51 | 0x54 | 0x55 | 0x58 | 0x59 | 0x5C | 0x5D => (
            "Load/store with register offset".to_string(),
            "???".to_string(),
        ), // Load/store with register offset
        0x52 | 0x53 | 0x56 | 0x57 | 0x5A | 0x5B | 0x5E | 0x5F => (
            "Load/store sign-extended byte/halfword".to_string(),
            "???".to_string(),
        ), // Load/store sign-extended byte/halfword
        0x60..=0x7F => {
            let rd = (opcode & 0x7) as u8;
            let rb = ((opcode >> 3) & 0x7) as u8;
            let offset = ((opcode >> 6) & 0x1F) as u32;
            let load = (opcode & 0x800) != 0;
            let byte = (opcode & 0x1000) != 0;

            let bits = "---B LOffsetRb__Rd_";

            (
                match (load, byte) {
                    (false, false) => {
                        format!("STR R{},[R{},#{}]", rd, rb, offset)
                    }
                    (true, false) => {
                        format!("LDR R{},[R{},#{}]", rd, rb, offset)
                    }
                    (false, true) => {
                        format!("STRB R{},[R{},#{}]", rd, rb, offset)
                    }
                    (true, true) => {
                        format!("LDRB R{},[R{},#{}]", rd, rb, offset)
                    }
                },
                bits.to_string(),
            )
        }
        0x80..=0x8F => {
            let rd = (opcode & 0x7) as u8;
            let rb = ((opcode >> 3) & 0x7) as u8;
            let offset = (((opcode >> 6) & 0x1F) << 1) as u32;
            let load = (opcode & 0x800) != 0;

            let bits = "---- LOffset_Rb__Rd_".to_string();
            (
                match load {
                    false => format!("STRH R{},[R{}, #0x{:02X}]", rd, rb, offset),
                    true => format!("LDRH R{},[R{}, #0x{:02X}]", rd, rb, offset),
                },
                bits,
            ) // Load/store halfword
        }
        0x90..=0x9F => ("SP-relative load/store".to_string(), "???".to_string()), // SP-relative load/store
        0xA0..=0xAF => ("Load address".to_string(), "???".to_string()),           // Load address
        0xB0 => ("Add offset to stack pointer".to_string(), "???".to_string()), // Add offset to stack pointer
        0xB4 | 0xB5 => ("PUSH ???".to_string(), "---- L--R __RLIST__".to_string()), // Push/pop registers
        0xBC | 0xBD => ("POP ???".to_string(), "---- L--R __RLIST__".to_string()), // Push/pop registers
        0xC0..=0xCF => ("Multiple load/store".to_string(), "???".to_string()), // Multiple load/store
        0xD0..=0xDE => ("Conditional branch".to_string(), "???".to_string()),  // Conditional branch
        0xDF => (
            format!("SWI {:2X}", opcode & 0xFF),
            "---- ---- __Value8__".to_string(),
        ), // Software interrupt
        0xE0..=0xE7 => ("Unconditional branch,".to_string(), "???".to_string()), // Unconditional branch,
        0xF0..=0xFF => ("BL ???".to_string(), "---- H____Offset___".to_string()), // Long branch with link
        _ => ("???".to_string(), "???".to_string()),
    }
}
