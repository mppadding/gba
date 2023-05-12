use std::{io, thread, time::Duration};

use cpu::MMU;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use debugger::Debugger;
use log::{info, warn};
use ratatui::{backend::CrosstermBackend, Terminal};
use tui_logger::init_logger;

use crate::cpu::CPU;

mod cpu;
mod debugger;
mod disassembler;
mod mem;

type Error = Box<dyn std::error::Error>;
type ByteResult<T> = std::result::Result<T, Error>;

struct ByteArray {
    data: Vec<u8>,
}

impl ByteArray {
    fn get_u8(&self, addr: usize) -> ByteResult<u8> {
        if addr >= self.data.len() {
            return Err("Addr `{:X}` out of range".into());
        }

        Ok(self.data[addr])
    }
    fn get_u16(&self, addr: usize) -> ByteResult<u16> {
        if (addr + 1) >= self.data.len() {
            return Err("Addr `{:X}` out of range".into());
        }

        Ok(((self.data[addr + 1] as u16) << 8) | (self.data[addr] as u16))
    }
    fn get_u32(&self, addr: usize) -> ByteResult<u32> {
        if (addr + 3) >= self.data.len() {
            return Err("Addr `{:X}` out of range".into());
        }

        Ok(((self.data[addr + 3] as u32) << 24)
            | ((self.data[addr + 2] as u32) << 16)
            | ((self.data[addr + 1] as u32) << 8)
            | (self.data[addr] as u32))
    }
}

fn main() -> Result<(), io::Error> {
    init_logger(log::LevelFilter::Trace).unwrap();
    tui_logger::set_default_level(log::LevelFilter::Trace);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut dbg = Debugger::default();

    let mut cpu = CPU::new();
    cpu.reset();
    cpu.load_bios(&std::fs::read("bios/gba_bios.bin").unwrap());
    cpu.load_rom(&std::fs::read("roms/pokemon_emerald.gba").unwrap());
    //cpu.load_rom(&std::fs::read("roms/super_dodgeball_advance.gba").unwrap());
    //cpu.load_rom(&std::fs::read("roms/super_mario_advance2.gba").unwrap());
    //cpu.load_rom(&std::fs::read("roms/super_mario_advance4.gba").unwrap());
    //cpu.load_rom(&std::fs::read("roms/mario_kart_super_circuit.gba").unwrap());
    //cpu.load_rom(&std::fs::read("roms/rgb_test.gba").unwrap());

    let mut instruction_counter: isize = 0;

    let mut bkpt = 20;
    bkpt = 22;

    let (mut free_run, mut paused) = if bkpt > 0 {
        (true, false)
    } else {
        (false, true)
    };

    loop {
        let program_counter = cpu.get_program_counter();
        let opcode: u32 = if !cpu.addr_valid(program_counter) {
            if !cpu.panic {
                warn!("Panicked! PC at invalid address `{:08X}`", program_counter);
                cpu.panic = true;
            }
            0
        } else {
            match cpu.is_thumb() {
                true => {
                    let word = cpu.read_u32(false, program_counter & 0xFFFFFFFE);
                    let upper = (program_counter & 0x1) == 0x1;
                    match upper {
                        false => word & 0xFFFF,
                        true => (word >> 16) & 0xFFFF,
                    }
                }
                false => cpu.read_u32(false, program_counter),
            }
        };

        dbg.opcode = opcode;

        terminal.draw(|f| debugger::draw(f, &dbg, &mut cpu))?;

        if event::poll(Duration::from_secs(0)).unwrap_or(false) {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Enter => {
                        if cpu.panic {
                            warn!("CPU in panic mode, cannot step. Reset using `r`");
                        }

                        paused = false
                    }
                    KeyCode::Char('h') => free_run = !free_run,
                    KeyCode::Char('z') => panic!("Panic!"),
                    KeyCode::Char('r') => {
                        warn!("CPU Reset");
                        cpu.reset();
                        instruction_counter = 0;
                        free_run = false;
                        paused = true;
                    }
                    _ => {}
                }
            }
        }

        if !cpu.panic && (!paused || free_run) {
            cpu.execute(opcode);

            if !free_run {
                paused = true;
            }

            if bkpt == instruction_counter {
                free_run = false;
                paused = true;
            }

            instruction_counter += 1;
        }

        thread::sleep(Duration::from_millis(16));
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

//fn main() {
//    let rom = ByteArray {
//        data: std::fs::read("roms/pokemon_emerald.gba").unwrap(),
//    };
//
//    let mut cpu = CPU::new();
//    cpu.reset();
//
//    let mut instruction_counter: isize = 0;
//    let br = 2;
//    let step = true;
//    let mut buffer = String::new();
//    let stdin = io::stdin();
//
//    loop {
//        let program_counter = cpu.get_program_counter() as usize;
//        let opcode: u32 = match cpu.is_thumb() {
//            true => rom.get_u16(program_counter).unwrap() as u32,
//            false => rom.get_u32(program_counter).unwrap(),
//        };
//
//        print_opcode_binary(opcode);
//        print!("[{:08X}h] => {:08X}h ", program_counter, opcode);
//
//        cpu.execute(opcode);
//
//        if step {
//            println!("\n{:#?}", cpu);
//            println!("Enter to continue");
//            stdin.read_line(&mut buffer).expect("Failed to read stdin");
//        } else {
//            if instruction_counter == br {
//                println!("\n{:#?}", cpu);
//                panic!("Dumped");
//            }
//        }
//
//        instruction_counter += 1;
//    }
//}
