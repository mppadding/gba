use std::{
    backtrace::Backtrace,
    collections::HashMap,
    io::{self, Stdout},
    panic,
    time::Duration,
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::debug;
use log::error;
use log::info;
use log::warn;
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use tui_logger::{init_logger, TuiLoggerSmartWidget, TuiWidgetState};

use crate::{
    cpu::{CPU, MMU},
    disassembler, print_cpu_backtrace,
};

// To create a conditional:
//      let br = debugger::Breakpoint::AddressConditional(10, Box::new(|| {}));
pub enum Breakpoint {
    Default,
    Delay(u32, u32), // (Break after X, counter of hits)
    Conditional(Box<dyn Fn() -> bool>),
}

pub enum ViewState {
    RAM,
    IO,
    LOG,
}

#[derive(PartialEq, Eq)]
pub enum DebuggerEvent {
    None,
    Quit,
    Reset,
    Back,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum InputMode {
    GAME,
    DEBUGGER,
}

pub const MGBA_REG_DEBUG_ENABLE: usize = 0x04FFF780;
pub const MGBA_REG_DEBUG_FLAGS: usize = 0x04FFF700;
pub const MGBA_REG_DEBUG_STRING: usize = 0x04FFF600;

pub struct MgbaDebug {
    regs: [u8; 512],
}

impl MgbaDebug {
    pub fn new() -> Self {
        Self { regs: [0; 512] }
    }

    pub fn read_u8(&self, addr: usize) -> u8 {
        match addr {
            0x180 => (self.read_enable() & 0xFF) as u8,
            0x181 => (self.read_enable() >> 8) as u8,
            _ => self.regs[addr],
        }
    }

    pub fn write_u8(&mut self, addr: usize, val: u8) {
        match addr {
            0x100 | 0x101 => {
                self.regs[addr] = val;

                let flags = self.read_flags();
                if flags & 0x100 != 0 && self.read_enable() == 0x1DEA {
                    let s =
                        std::str::from_utf8(&self.regs[0..=0x100]).expect("invalid utf-8 sequence");

                    #[cfg(not(feature = "debugger"))]
                    match val & 0b111 {
                        0 => println!("[FATAL] {s}"),
                        1 => println!("[ERROR] {s}"),
                        2 => println!("[WARN ] {s}"),
                        3 => println!("[INFO ] {s}"),
                        4 => println!("[DEBUG] {s}"),
                        _ => {}
                    }

                    #[cfg(feature = "debugger")]
                    match val & 0b111 {
                        0 => error!("GBA FATAL: {s}"),
                        1 => error!("GBA: {s}"),
                        2 => warn!("GBA: {s}"),
                        3 => info!("GBA: {s}"),
                        4 => debug!("GBA: {s}"),
                        _ => {}
                    }

                    self.regs[0x101] = 0x00;
                }
            }
            _ => self.regs[addr] = val,
        }
    }

    fn read_flags(&self) -> u16 {
        ((self.regs[0x101] as u16) << 8) | self.regs[0x100] as u16
    }

    fn read_enable(&self) -> u16 {
        match (self.regs[0x180] == 0xDE) && (self.regs[0x181] == 0xC0) {
            false => 0x0000,
            true => 0x1DEA,
        }
    }
}

pub struct Debugger {
    pub opcode: u32,
    pub state: ViewState,
    pub breakpoints: HashMap<u32, Breakpoint>,
    pub instruction_counter: usize,
    pub free_run: bool,
    pub paused: bool,
    pub input_mode: InputMode,
    pub lockstep: bool,

    #[cfg(feature = "debugger")]
    pub terminal: Terminal<CrosstermBackend<Stdout>>,
}

#[cfg(not(feature = "debugger"))]
impl Debugger {
    pub fn new() -> Self {
        Self {
            opcode: 0,
            breakpoints: HashMap::default(),
            state: ViewState::RAM,
            instruction_counter: 0,
            free_run: true,
            paused: false,
            input_mode: InputMode::DEBUGGER,
            lockstep: false,
        }
    }

    pub fn draw(&mut self, _cpu: &CPU) {}
    pub fn update(&mut self, _cpu: &CPU) -> DebuggerEvent {
        DebuggerEvent::None
    }
    pub fn exit(&mut self) {}

    pub fn set_panic_hook() {}
}

#[cfg(feature = "debugger")]
impl Debugger {
    pub fn new() -> Self {
        init_logger(log::LevelFilter::Trace).unwrap();
        tui_logger::set_default_level(log::LevelFilter::Trace);

        enable_raw_mode().expect("[TERM] Failed to enable raw mode");
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .expect("[TERM] Failed to enter alternate screen & enable mouse capture");
        let backend = CrosstermBackend::new(stdout);

        Self {
            opcode: 0,
            breakpoints: HashMap::default(),
            state: ViewState::RAM,
            instruction_counter: 0,
            free_run: false,
            paused: true,
            input_mode: InputMode::DEBUGGER,
            lockstep: false,
            terminal: Terminal::new(backend).expect("[TERM] Failed to create terminal"),
        }
    }

    pub fn draw(&mut self, cpu: &mut CPU) {
        self.terminal
            .draw(|f| {
                let constraints = match self.state {
                    ViewState::RAM => [
                        Constraint::Percentage(14),
                        Constraint::Percentage(48),
                        Constraint::Percentage(100 - 48 - 14),
                    ],
                    ViewState::IO => [
                        Constraint::Percentage(14),
                        Constraint::Percentage(50),
                        Constraint::Percentage(100 - 54 - 14),
                    ],
                    ViewState::LOG => [
                        Constraint::Percentage(14),
                        Constraint::Percentage(0),
                        Constraint::Percentage(100 - 14),
                    ],
                };

                let verts = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(0)
                    .constraints(constraints)
                    .split(f.size());

                let hors1 = Layout::default()
                    .direction(Direction::Horizontal)
                    .margin(0)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                    .split(verts[0]);

                let block = Block::default().title("Opcode").borders(Borders::ALL);
                let opcode_text = match cpu.is_thumb() {
                    false => format_opcode_arm(self.opcode, cpu),
                    true => format_opcode_thumb(self.opcode as u16, cpu),
                };
                let text = Paragraph::new(opcode_text).block(block);
                f.render_widget(text, hors1[0]);

                let block = Block::default()
                    .title("Debugger State")
                    .borders(Borders::ALL);
                let text = Paragraph::new(format_debugger_state(
                    self.instruction_counter,
                    self.free_run,
                    self.lockstep,
                    self.input_mode,
                    cpu,
                ))
                .block(block);
                f.render_widget(text, hors1[1]);

                match self.state {
                    ViewState::RAM => draw_view_ram(f, cpu, verts[1]),
                    ViewState::IO => draw_view_io(f, cpu, verts[1]),
                    ViewState::LOG => {}
                }

                let mut tws =
                    TuiWidgetState::new().set_default_display_level(log::LevelFilter::Debug);

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
            })
            .expect("[TERM] Failed to draw to terminal");
    }

    pub fn exit(&mut self) {
        disable_raw_mode().expect("[TERM] Failed to disable raw mode");
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )
        .expect("[TERM] Failed to leave alternate screen & disable mouse capture");
        self.terminal
            .show_cursor()
            .expect("[TERM] Failed to show cursor");
    }

    pub fn update(&mut self, cpu: &CPU) -> DebuggerEvent {
        while event::poll(Duration::from_secs(0)).unwrap_or(false) {
            if let Event::Key(key) = event::read().unwrap() {
                if key.code == KeyCode::F(1) {
                    self.input_mode = match self.input_mode {
                        InputMode::GAME => InputMode::DEBUGGER,
                        InputMode::DEBUGGER => InputMode::GAME,
                    };
                } else {
                    if self.input_mode == InputMode::DEBUGGER {
                        match key.code {
                            KeyCode::Char('q') => return DebuggerEvent::Quit,
                            KeyCode::Enter => {
                                if cpu.panic {
                                    warn!(
                                    "CPU in panic mode, cannot step. Reset using `r` (stuck at `{}`)",
                                    self.instruction_counter
                                );
                                }

                                self.paused = false
                            }
                            KeyCode::Char('p') => {
                                self.state = match self.state {
                                    ViewState::RAM => ViewState::IO,
                                    ViewState::IO => ViewState::LOG,
                                    ViewState::LOG => ViewState::RAM,
                                };
                            }
                            KeyCode::Char('h') => self.free_run = !self.free_run,
                            KeyCode::Char('l') => self.lockstep = !self.lockstep,
                            KeyCode::F(2) => {
                                self.free_run = true;
                                self.input_mode = InputMode::GAME;
                            }
                            KeyCode::Char('r') => {
                                self.reset();
                                return DebuggerEvent::Reset;
                            }
                            KeyCode::Backspace => {
                                return DebuggerEvent::Back;
                            }
                            _ => {
                                warn!("Key: {:?}", key);
                            }
                        }
                    }
                }
            }
        }

        DebuggerEvent::None
    }

    pub fn set_panic_hook() {
        panic::set_hook(Box::new(|panic_info| {
            let bt = Backtrace::capture();
            enable_raw_mode().expect("[TERM] Failed to enable raw mode");
            let stdout = io::stdout();
            let backend = CrosstermBackend::new(stdout);
            let mut terminal = Terminal::new(backend).expect("[TERM] Failed to create terminal");

            disable_raw_mode().expect("[TERM] Failed to disable raw mode");
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )
            .expect("[TERM] Failed to leave alternate screen & disable mouse capture");
            terminal
                .show_cursor()
                .expect("[TERM] Failed to show cursor");

            println!("{panic_info}");
            println!("Backtrace:\n{bt}");
            print_cpu_backtrace();
        }));
    }

    pub fn should_break(&mut self, pc: u32) -> bool {
        if (!self.paused || self.free_run) && self.breakpoints.contains_key(&pc) {
            match self.breakpoints.get_mut(&pc).unwrap() {
                Breakpoint::Default => return true,
                Breakpoint::Delay(max, cur) => {
                    *cur += 1;
                    return cur > max;
                }
                Breakpoint::Conditional(f) => return f(),
            }
        }

        false
    }
}

impl Debugger {
    pub fn reset(&mut self) {
        self.instruction_counter = 0;
        self.free_run = false;
        self.paused = true;

        self.input_mode = InputMode::DEBUGGER;
    }
}

fn format_stack(cpu: &mut CPU) -> String {
    let sp = cpu.read_register(13);

    let mut stack = String::new();

    for i in 0..20 {
        let addr = sp + (i * 4);
        stack.push_str(format!("SP+{:2}│ {:08X}: ", (i * 4), addr).as_str());

        if !cpu.addr_valid(addr) {
            stack.push_str("--------\n")
        } else {
            stack.push_str(format!("{:08X}h\n", cpu.read_u32(false, addr)).as_str());
        }
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
    regs.push_str("Flags│ ");
    if cpu.get_flag_n() {
        regs.push_str("N");
    }
    if cpu.get_flag_z() {
        regs.push_str("Z");
    }
    if cpu.get_flag_c() {
        regs.push_str("C");
    }
    if cpu.get_flag_v() {
        regs.push_str("V");
    }
    regs.push_str("\n");

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

fn format_opcode_thumb(opcode: u16, cpu: &CPU) -> String {
    let mut fmt = String::new();

    fmt.push_str(format!("op: {:04X}h\n    ", opcode).as_str());

    for i in 0..4 {
        fmt.push_str(format!("{:04b} ", (opcode >> (12 - (4 * i))) & 0xF).as_str());
    }

    let (mnemonic, args) = disassembler::disassemble_thumb(opcode);
    fmt.push_str(format!("\n    {}\n    {}\n", args, mnemonic).as_str());

    fmt
}

fn format_opcode_arm(opcode: u32, cpu: &CPU) -> String {
    let mut fmt = String::new();

    fmt.push_str(format!("op: {:08X}h\n    ", opcode).as_str());

    for i in 0..8 {
        fmt.push_str(format!("{:04b} ", (opcode >> (28 - (4 * i))) & 0xF).as_str());
    }

    let (mnemonic, args) = disassembler::disassemble_arm(opcode, cpu.get_program_counter());
    fmt.push_str(format!("\n    {}\n    {}\n", args, mnemonic).as_str());

    fmt
}

fn format_debugger_state(
    instruction_counter: usize,
    free_run: bool,
    lockstep: bool,
    input_mode: InputMode,
    cpu: &CPU,
) -> String {
    let mut fmt = String::new();

    fmt.push_str(
        format!(
            "Instr Counter: {:8}    Cycle Count: {:8}\n",
            instruction_counter, cpu.cycle_count
        )
        .as_str(),
    );
    fmt.push_str(format!("     Free run: {}\t\t\n", free_run).as_str());
    fmt.push_str(format!("    Lock Step: {}\t\t\n", lockstep).as_str());
    match input_mode {
        InputMode::GAME => fmt.push_str("   Input Mode: Game\n"),
        InputMode::DEBUGGER => fmt.push_str("   Input Mode: Debugger\n"),
    }

    fmt
}

fn format_dma(cpu: &mut CPU) -> String {
    let mut fmt = String::new();

    for i in 0..4 {
        let addr = (0x040000B0 + (i * 12)) as u32;
        let cnt_ctrl = cpu.read_u32(false, addr + 8);
        fmt.push_str(format!("DMA{} SRC│ {:08X}h\n", i, cpu.read_u32(false, addr)).as_str());
        fmt.push_str(format!("DMA{} DST│ {:08X}h\n", i, cpu.read_u32(false, addr + 4)).as_str());
        fmt.push_str(format!("DMA{} CNT│     {:04X}h\n", i, cnt_ctrl & 0xFFFF).as_str());
        fmt.push_str(format!("DMA{} CTR│     {:04X}h\n", i, (cnt_ctrl >> 16) & 0xFFFF).as_str());
        fmt.push_str("        │\n");
    }
    fmt.push_str("        │\n");
    fmt.push_str("        │\n");

    fmt
}

fn format_interrupt(cpu: &mut CPU) -> String {
    let mut fmt = String::new();

    fmt.push_str(format!("      IE│ {:04X}h\n", cpu.io_ie).as_str());
    fmt.push_str(format!("      IF│ {:04X}h\n", cpu.io_if).as_str());
    fmt.push_str(format!("     IME│ {:04X}h\n", cpu.io_ime).as_str());
    fmt.push_str(format!("        │\n").as_str());
    fmt.push_str(format!(" WAITCNT│ {:04X}h\n", cpu.io_waitcnt).as_str());
    fmt.push_str(format!("        │\n").as_str());
    fmt.push_str(format!("TM0CNT_L│ ????h\n").as_str());
    fmt.push_str(format!("TM0CNT_H│ ????h\n").as_str());
    fmt.push_str(format!("TM1CNT_L│ ????h\n").as_str());
    fmt.push_str(format!("TM1CNT_H│ ????h\n").as_str());
    fmt.push_str(format!("TM2CNT_L│ ????h\n").as_str());
    fmt.push_str(format!("TM2CNT_H│ ????h\n").as_str());
    fmt.push_str(format!("TM3CNT_L│ ????h\n").as_str());
    fmt.push_str(format!("TM3CNT_H│ ????h\n").as_str());
    fmt.push_str(format!("        │\n").as_str());
    fmt.push_str(format!("KEYINPUT│ {:04X}h\n", cpu.keypad.keyinput).as_str());
    fmt.push_str(format!("  KEYCNT│ {:04X}h\n", cpu.keypad.keycnt).as_str());
    fmt.push_str(format!("        │\n").as_str());
    fmt.push_str(format!("    HALT│ {}\n", cpu.halt).as_str());
    fmt.push_str(format!(" BIOS_IF│ {:04X}h\n", cpu.io_bios_if).as_str());
    fmt.push_str(format!("        │\n").as_str());
    fmt.push_str(format!("        │\n").as_str());

    fmt
}

fn format_sound(cpu: &mut CPU) -> String {
    let mut fmt = String::new();

    fmt.push_str(format!("SOUND1CNT_L│     ????h\n").as_str());
    fmt.push_str(format!("SOUND1CNT_H│     ????h\n").as_str());
    fmt.push_str(format!("SOUND1CNT_X│     ????h\n").as_str());
    fmt.push_str(format!("SOUND2CNT_L│     ????h\n").as_str());
    fmt.push_str(format!("SOUND2CNT_H│     ????h\n").as_str());
    fmt.push_str(format!("SOUND3CNT_L│     ????h\n").as_str());
    fmt.push_str(format!("SOUND3CNT_H│     ????h\n").as_str());
    fmt.push_str(format!("SOUND3CNT_X│     ????h\n").as_str());
    fmt.push_str(format!("SOUND4CNT_L│     ????h\n").as_str());
    fmt.push_str(format!("SOUND4CNT_H│     ????h\n").as_str());
    fmt.push_str(format!(" SOUNDCNT_L│     ????h\n").as_str());
    fmt.push_str(format!(" SOUNDCNT_H│     ????h\n").as_str());
    fmt.push_str(format!(" SOUNDCNT_X│     ????h\n").as_str());
    fmt.push_str(format!("  SOUNDBIAS│     ????h\n").as_str());
    fmt.push_str(format!("           │\n").as_str());
    fmt.push_str(format!("   WAVE_RAM│     ????h\n").as_str());
    fmt.push_str(format!("     FIFO_A│ ????????h\n").as_str());
    fmt.push_str(format!("     FIFO_B│ ????????h\n").as_str());
    fmt.push_str(format!("           │\n").as_str());
    fmt.push_str(format!("           │\n").as_str());
    fmt.push_str(format!("           │\n").as_str());
    fmt.push_str(format!("           │\n").as_str());

    fmt
}

fn format_lcd(cpu: &mut CPU) -> String {
    let mut fmt = String::new();

    let bg0cnt = cpu.lcd.get_background_control(0);
    let bg1cnt = cpu.lcd.get_background_control(1);
    let bg2cnt = cpu.lcd.get_background_control(2);
    let bg3cnt = cpu.lcd.get_background_control(3);

    let (bg0off_x, bg0off_y) = cpu.lcd.get_background_offset(0);
    let (bg1off_x, bg1off_y) = cpu.lcd.get_background_offset(1);
    let (bg2off_x, bg2off_y) = cpu.lcd.get_background_offset(2);
    let (bg3off_x, bg3off_y) = cpu.lcd.get_background_offset(3);

    let (win0_h, win0_v) = cpu.lcd.get_window_dimensions(0);
    let (win1_h, win1_v) = cpu.lcd.get_window_dimensions(1);

    let winin = cpu.lcd.get_winin();
    let winout = cpu.lcd.get_winout();

    fmt.push_str(format!("    DISPCNT│       {:04X}h\n", cpu.lcd.get_dispcnt()).as_str());
    fmt.push_str(format!("   DISPSTAT│       {:04X}h\n", cpu.lcd.get_dispstat()).as_str());
    fmt.push_str(format!("     VCOUNT│        {:03}\n", cpu.lcd.get_vcount()).as_str());
    fmt.push_str(format!("     BG0CNT│       {:04X}h\n", bg0cnt).as_str());
    fmt.push_str(format!("     BG1CNT│       {:04X}h\n", bg1cnt).as_str());
    fmt.push_str(format!("     BG2CNT│       {:04X}h\n", bg2cnt).as_str());
    fmt.push_str(format!("     BG3CNT│       {:04X}h\n", bg3cnt).as_str());
    fmt.push_str(format!(" BG0 Off XY│ {:04X}h,{:04X}h\n", bg0off_x, bg0off_y).as_str());
    fmt.push_str(format!(" BG1 Off XY│ {:04X}h,{:04X}h\n", bg1off_x, bg1off_y).as_str());
    fmt.push_str(format!(" BG2 Off XY│ {:04X}h,{:04X}h\n", bg2off_x, bg2off_y).as_str());
    fmt.push_str(format!(" BG3 Off XY│ {:04X}h,{:04X}h\n", bg3off_x, bg3off_y).as_str());
    fmt.push_str(format!("  BG2 PA,PB│ ????h,????h\n").as_str());
    fmt.push_str(format!("  BG2 PC,PD│ ????h,????h\n").as_str());
    fmt.push_str(format!("       BG2X│   ????????h\n").as_str());
    fmt.push_str(format!("       BG2Y│   ????????h\n").as_str());
    fmt.push_str(format!("  BG3 PA,PB│ ????h,????h\n").as_str());
    fmt.push_str(format!("  BG3 PC,PD│ ????h,????h\n").as_str());
    fmt.push_str(format!("       BG3X│   ????????h\n").as_str());
    fmt.push_str(format!("       BG3Y│   ????????h\n").as_str());
    fmt.push_str(format!("    WIN0 HV│ {:04X}h,{:04X}h\n", win0_h, win0_v).as_str());
    fmt.push_str(format!("    WIN1 HV│ {:04X}h,{:04X}h\n", win1_h, win1_v).as_str());
    fmt.push_str(format!(" WIN IN,OUT│ {:04X}h,{:04X}h\n", winin, winout).as_str());

    fmt
}

fn format_serial(cpu: &mut CPU) -> String {
    let mut fmt = String::new();

    fmt.push_str(format!("  SIODATA32│ {:08X}h\n", cpu.serial.read_u32(0x120)).as_str());
    fmt.push_str(format!("  SIOMULTI0│     {:04X}h\n", cpu.serial.read_u16(0x120)).as_str());
    fmt.push_str(format!("  SIOMULTI1│     {:04X}h\n", cpu.serial.read_u16(0x122)).as_str());
    fmt.push_str(format!("  SIOMULTI2│     {:04X}h\n", cpu.serial.read_u16(0x124)).as_str());
    fmt.push_str(format!("  SIOMULTI3│     {:04X}h\n", cpu.serial.read_u16(0x126)).as_str());
    fmt.push_str(format!("     SIOCNT│     {:04X}h\n", cpu.serial.read_u16(0x128)).as_str());
    fmt.push_str(format!("SIOMLT_SEND│     {:04X}h\n", cpu.serial.read_u16(0x12A)).as_str());
    fmt.push_str(format!("   SIODATA8│     {:04X}h\n", cpu.serial.read_u16(0x12A)).as_str());
    fmt.push_str(format!("           │\n").as_str());
    fmt.push_str(format!("       RCNT│     {:04X}h\n", cpu.serial.rcnt).as_str());
    fmt.push_str(format!("     JOYCNT│     {:04X}h\n", cpu.serial.joy_cnt).as_str());
    fmt.push_str(format!("   JOY_RECV│ {:08X}h\n", cpu.serial.joy_recv).as_str());
    fmt.push_str(format!("  JOY_TRANS│ {:08X}h\n", cpu.serial.joy_trans).as_str());
    fmt.push_str(format!("    JOYSTAT│     {:04X}h\n", cpu.serial.joy_stat).as_str());
    fmt.push_str(format!("           │\n").as_str());
    fmt.push_str(format!("           │\n").as_str());
    fmt.push_str(format!("           │\n").as_str());
    fmt.push_str(format!("─LCD──────────────────\n").as_str());
    fmt.push_str(format!("     MOSAIC│     {:04X}h\n", cpu.lcd.get_mosaic()).as_str());
    fmt.push_str(format!("     BLDCNT│     {:04X}h\n", cpu.lcd.get_bldcnt()).as_str());
    fmt.push_str(format!("   BLDALPHA│     {:04X}h\n", cpu.lcd.get_bldalpha()).as_str());
    fmt.push_str(format!("       BLDY│     {:04X}h\n", cpu.lcd.get_bldy()).as_str());

    fmt
}

fn draw_view_ram<B: Backend>(f: &mut Frame<B>, cpu: &mut CPU, area: Rect) {
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
        .split(area);

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
}

fn draw_view_io<B: Backend>(f: &mut Frame<B>, cpu: &mut CPU, area: Rect) {
    let hors = Layout::default()
        .direction(Direction::Horizontal)
        .margin(0)
        .constraints(
            [
                Constraint::Percentage(17),
                Constraint::Percentage(14),
                Constraint::Percentage(19),
                Constraint::Percentage(19),
                Constraint::Percentage(100 - 19 - 19 - 14 - 17),
            ]
            .as_ref(),
        )
        .split(area);

    let block = Block::default().title("DMA").borders(Borders::ALL);
    let text = Paragraph::new(format_dma(cpu)).block(block);
    f.render_widget(text, hors[0]);

    let block = Block::default()
        .title("Int/Wait/Timer")
        .borders(Borders::ALL);
    let text = Paragraph::new(format_interrupt(cpu)).block(block);
    f.render_widget(text, hors[1]);

    let block = Block::default().title("Sound").borders(Borders::ALL);
    let text = Paragraph::new(format_sound(cpu)).block(block);
    f.render_widget(text, hors[2]);

    let block = Block::default().title("Serial").borders(Borders::ALL);
    let text = Paragraph::new(format_serial(cpu)).block(block);
    f.render_widget(text, hors[3]);

    let block = Block::default().title("LCD").borders(Borders::ALL);
    let text = Paragraph::new(format_lcd(cpu)).block(block);
    f.render_widget(text, hors[4]);
}
