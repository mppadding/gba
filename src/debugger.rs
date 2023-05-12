use std::cmp;

use log::debug;
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use tui_logger::{TuiLoggerSmartWidget, TuiWidgetState};

use crate::{
    cpu::{CPU, MMU},
    disassembler,
};

pub struct Debugger {
    pub opcode: u32,
}

impl Default for Debugger {
    fn default() -> Self {
        Self { opcode: 0 }
    }
}

fn format_stack(cpu: &mut CPU) -> String {
    let sp = cpu.read_register(13);

    let mut stack = String::new();

    for i in 0..20 {
        let addr = sp + (i * 4);
        stack.push_str(
            format!(
                "SP+{:2}│ {:08X}: {:08X}h\n",
                (i * 4),
                addr,
                cpu.read_u32(false, addr)
            )
            .as_str(),
        );
    }

    stack
}

fn format_rom(cpu: &mut CPU) -> String {
    let pc = cpu.get_program_counter() & 0xFFFFFFFE;

    let mut rom = String::new();

    for i in 0..20 {
        match cpu.is_thumb() {
            false => {
                let addr = pc + (i * 4);

                rom.push_str(format!("PC+{:2}│ ", (i * 4)).as_str());

                if !cpu.addr_valid(addr) {
                    rom.push_str("--------\n");
                } else {
                    rom.push_str(format!("{:08X}h\n", cpu.read_u32(false, addr)).as_str());
                }
            }
            true => {
                let addr = pc + (i * 2);
                rom.push_str(format!("PC+{:2}│ ", (i * 2)).as_str());

                if !cpu.addr_valid(addr) {
                    rom.push_str("----\n");
                } else {
                    rom.push_str(format!("{:04X}h\n", cpu.read_u32(false, addr) & 0xFFFF).as_str());
                }
            }
        }
    }

    rom
}

fn format_regs(cpu: &CPU) -> String {
    let mut regs = String::new();

    for i in 0..10 {
        regs.push_str(format!("   R{}│ {:08X}h\n", i, cpu.read_register(i)).as_str());
    }
    for i in 10..13 {
        regs.push_str(format!("  R{}│ {:08X}h\n", i, cpu.read_register(i)).as_str());
    }
    regs.push_str(format!("   SP│ {:08X}h\n", cpu.read_register(13)).as_str());
    regs.push_str(format!(" LINK│ {:08X}h\n", cpu.read_register(14)).as_str());
    regs.push_str(format!("   PC│ {:08X}h\n", cpu.get_program_counter()).as_str());
    regs.push_str(format!(" CPSR│ {:08X}h\n", cpu.reg_cpsr).as_str());
    regs.push_str(format!(" SPSR│ {:08X}h\n", cpu.regs_spsr[cpu.get_mode() as usize]).as_str());
    match cpu.get_mode() {
        0x0 => regs.push_str(" Mode│ User\n"),
        0x1 => regs.push_str(" Mode│ FIQ\n"),
        0x2 => regs.push_str(" Mode│ IRQ\n"),
        0x3 => regs.push_str(" Mode│ Supervisor\n"),
        0x7 => regs.push_str(" Mode│ Abort\n"),
        0xB => regs.push_str(" Mode│ Undefined\n"),
        0xF => regs.push_str(" Mode│ System\n"),
        _ => unreachable!("mode > 0xF"),
    }
    if cpu.is_thumb() {
        regs.push_str("State│ THUMB");
    } else {
        regs.push_str("State│ ARM");
    }

    regs
}

fn format_memory(cpu: &mut CPU) -> String {
    let mut fmt = String::new();

    let mem_ptr = cpu.mem_ptr & 0xFFFFFFF0;

    for y in 0..20 {
        let mem_ptr = mem_ptr.wrapping_add(y * 0x10);
        fmt.push_str(format!("{:08X}│", mem_ptr).as_str());

        for x in 0..4 {
            for nib in 0..4 {
                let mem_ptr = mem_ptr + (x * 4) + nib;

                if nib % 2 == 0 {
                    fmt.push_str(" ");
                }

                if !cpu.addr_valid(mem_ptr) {
                    fmt.push_str("--");
                } else {
                    let val = cpu.read_u8(false, mem_ptr);

                    fmt.push_str(format!("{:02X}", val).as_str())
                }
            }
        }

        fmt.push_str("\n");
    }

    fmt
}

fn format_opcode_thumb(dbg: &Debugger, cpu: &CPU) -> String {
    let mut fmt = String::new();

    fmt.push_str(format!("op: {:04X}h\n    ", dbg.opcode).as_str());

    for i in 0..4 {
        fmt.push_str(format!("{:04b} ", (dbg.opcode >> (12 - (4 * i))) & 0xF).as_str());
    }

    let (mnemonic, args) = disassembler::disassemble_thumb(dbg.opcode);
    fmt.push_str(format!("\n    {}\n    {}\n", args, mnemonic).as_str());

    fmt
}

fn format_opcode_arm(dbg: &Debugger, cpu: &CPU) -> String {
    let mut fmt = String::new();

    fmt.push_str(format!("op: {:08X}h\n    ", dbg.opcode).as_str());

    for i in 0..8 {
        fmt.push_str(format!("{:04b} ", (dbg.opcode >> (28 - (4 * i))) & 0xF).as_str());
    }

    let (mnemonic, args) = disassembler::disassemble_arm(dbg.opcode, cpu.get_program_counter());
    fmt.push_str(format!("\n    {}\n    {}\n", args, mnemonic).as_str());

    fmt
}

pub fn draw<B: Backend>(f: &mut Frame<B>, debugger: &Debugger, cpu: &mut CPU) {
    let verts = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints(
            [
                Constraint::Percentage(14),
                Constraint::Percentage(46),
                Constraint::Percentage(100 - 46 - 14),
            ]
            .as_ref(),
        )
        .split(f.size());

    let hors = Layout::default()
        .direction(Direction::Horizontal)
        .margin(0)
        .constraints(
            [
                Constraint::Percentage(15),
                Constraint::Percentage(23),
                Constraint::Percentage(42),
                Constraint::Percentage(100 - 42 - 23 - 15),
            ]
            .as_ref(),
        )
        .split(verts[1]);

    let block = Block::default().title("Opcode").borders(Borders::ALL);
    let opcode_text = match cpu.is_thumb() {
        false => format_opcode_arm(debugger, cpu),
        true => format_opcode_thumb(debugger, cpu),
    };
    let text = Paragraph::new(opcode_text).block(block);
    f.render_widget(text, verts[0]);

    let block = Block::default().title("Registers").borders(Borders::ALL);
    let text = Paragraph::new(format_regs(cpu)).block(block);
    f.render_widget(text, hors[0]);

    let block = Block::default().title("Stack").borders(Borders::ALL);
    let text = Paragraph::new(format_stack(cpu)).block(block);
    f.render_widget(text, hors[1]);

    let block = Block::default()
        .title(format!("Memory ptr={:08X}", cpu.mem_ptr))
        .borders(Borders::ALL);
    let text = Paragraph::new(format_memory(cpu)).block(block);
    f.render_widget(text, hors[2]);

    let block = Block::default().title("ROM").borders(Borders::ALL);
    let text = Paragraph::new(format_rom(cpu)).block(block);
    f.render_widget(text, hors[3]);

    let mut tws = TuiWidgetState::new().set_default_display_level(log::LevelFilter::Debug);

    let tui_sm = TuiLoggerSmartWidget::default()
        .style_error(Style::default().fg(Color::Red))
        .style_debug(Style::default().fg(Color::Green))
        .style_warn(Style::default().fg(Color::Yellow))
        .style_trace(Style::default().fg(Color::Magenta))
        .style_info(Style::default().fg(Color::Cyan))
        .output_target(true)
        .output_separator(' ')
        .state(&mut tws);
    f.render_widget(tui_sm, verts[2]);
}
