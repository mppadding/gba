use std::collections::HashSet;
use std::time::Instant;

use cpu::MMU;
use debugger::Debugger;
use log::warn;

use crate::cpu::CPU;
use crate::debugger::DebuggerEvent;

mod cpu;
mod debugger;
mod disassembler;
mod keypad;
mod lcd;
mod renderer;
mod sound;

use sdl2::event::Event as WinEvent;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;

fn main() {


    let mut cpu = CPU::new();
    cpu.reset();
    cpu.load_bios(&std::fs::read("bios/gba_bios.bin").unwrap());
    //let rom = std::fs::read("roms/pokemon_emerald.gba").unwrap();
    //let rom = std::fs::read("roms/super_dodgeball_advance.gba").unwrap();
    //let rom = std::fs::read("roms/super_mario_advance2.gba").unwrap();
    //let rom = std::fs::read("roms/super_mario_advance4.gba").unwrap();
    //let rom = std::fs::read("roms/mario_kart_super_circuit.gba").unwrap();
    //let rom = std::fs::read("roms/rgb_test.gba").unwrap();
    //let rom = std::fs::read("roms/tonc/first.gba").unwrap();
    let rom = std::fs::read("roms/tonc/irq_demo.gba").unwrap();
    cpu.load_rom(&rom.clone());

    let mut dbg = Debugger::new();
    Debugger::set_panic_hook();
    dbg.breakpoints = HashSet::from([
        //0x00000000, 0x13c, 0x080002e0,
        //0x080016BC
        // 0x03000188,
        //0x08000492,
    ]);

    let sdl_context = sdl2::init().expect("[SDL] Failed to create context");
    let video_subsystem = sdl_context
        .video()
        .expect("[SDL] Failed to get video subsystem");

    let window = video_subsystem
        .window("pGBA", 240, 160)
        .opengl()
        .position(0, 0)
        .build()
        .map_err(|e| e.to_string())
        .expect("[SDL] Failed to create window");

    let mut canvas = window
        .into_canvas()
        .build()
        .map_err(|e| e.to_string())
        .expect("[SDL] Failed to get canvas");

    let texture_creator = canvas.texture_creator();

    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGB24, 240, 160)
        .map_err(|e| e.to_string())
        .expect("[SDL] Cannot create texture");
    renderer::draw_texture(&mut cpu, &mut texture);

    let mut event_pump = sdl_context
        .event_pump()
        .expect("[SDL] Failed to get event pump");

    let mut prev = Instant::now();
    let mut frame_timer = Duration::from_micros(0);

    //cpu.io_ime = 0;
    //cpu.io_ie = cpu::IRQ_GAMEPAK | cpu::IRQ_DEBUG1;
    //cpu.io_bios_if = cpu::IRQ_GAMEPAK;

    //cpu.halt = true;
    dbg.draw(&cpu);

    let start = Instant::now();

    //dbg.free_run = false;
    //dbg.paused = true;
    //dbg.lockstep = true;

    let mut timer_scanline: usize = 0;
    let mut dt_cycles: usize = 0;

    'running: loop {
        let program_counter = cpu.get_program_counter();
        let opcode: u32 = if !cpu.addr_valid(program_counter) {
            if !cpu.panic {
                warn!("Panicked! PC at invalid address `{:08X}`", program_counter);
                cpu.panic = true;
            }
            dbg.opcode
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

        if !cpu.panic && (!dbg.paused || dbg.free_run) {
            if dbg.breakpoints.contains(&program_counter) {
                warn!("Breakpoint hit at `{:08X}`", program_counter);
                dbg.free_run = false;
                dbg.paused = true;
                dbg.lockstep = true;
            }
        }

        if cpu.panic || dbg.lockstep || !dbg.free_run {
            dbg.draw(&mut cpu);
        }

        for event in event_pump.poll_iter() {
            match event {
                // Keymap:
                // GBA => Keyboard
                // Shoulder Left => A
                // Shoulder Right => S
                // Up => Up
                // Left => Left
                // Right => Right
                // Down => Down
                //
                // B => Z
                // A => X
                //
                // Start => Enter
                // Select => Backspace
                WinEvent::Quit { .. }
                | WinEvent::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                WinEvent::KeyDown {
                    keycode: Some(keycode),
                    ..
                } => match keycode {
                    Keycode::X => cpu.keypad.press(keypad::BUTTON_A),
                    Keycode::Z => cpu.keypad.press(keypad::BUTTON_B),
                    Keycode::Backspace => cpu.keypad.press(keypad::BUTTON_SELECT),
                    Keycode::Return => cpu.keypad.press(keypad::BUTTON_START),
                    Keycode::Right => cpu.keypad.press(keypad::BUTTON_RIGHT),
                    Keycode::Left => cpu.keypad.press(keypad::BUTTON_LEFT),
                    Keycode::Up => cpu.keypad.press(keypad::BUTTON_UP),
                    Keycode::Down => cpu.keypad.press(keypad::BUTTON_DOWN),
                    Keycode::S => cpu.keypad.press(keypad::BUTTON_R),
                    Keycode::A => cpu.keypad.press(keypad::BUTTON_L),
                    _ => warn!("Press: {:?}", keycode),
                },
                WinEvent::KeyUp {
                    keycode: Some(keycode),
                    ..
                } => match keycode {
                    Keycode::X => cpu.keypad.release(keypad::BUTTON_A),
                    Keycode::Z => cpu.keypad.release(keypad::BUTTON_B),
                    Keycode::Backspace => cpu.keypad.release(keypad::BUTTON_SELECT),
                    Keycode::Return => cpu.keypad.release(keypad::BUTTON_START),
                    Keycode::Right => cpu.keypad.release(keypad::BUTTON_RIGHT),
                    Keycode::Left => cpu.keypad.release(keypad::BUTTON_LEFT),
                    Keycode::Up => cpu.keypad.release(keypad::BUTTON_UP),
                    Keycode::Down => cpu.keypad.release(keypad::BUTTON_DOWN),
                    Keycode::S => cpu.keypad.release(keypad::BUTTON_R),
                    Keycode::A => cpu.keypad.release(keypad::BUTTON_L),
                    _ => warn!("Release: {:?}", keycode),
                },
                _ => {}
            }
        }

        while event::poll(Duration::from_secs(0)).unwrap_or(false) {
            if let TermEvent::Key(key) = event::read().unwrap() {
                if key.code == KeyCode::F(1) {
                    dbg.input_mode = match dbg.input_mode {
                        InputMode::GAME => InputMode::DEBUGGER,
                        InputMode::DEBUGGER => InputMode::GAME,
                    };
                } else {
                    if dbg.input_mode == InputMode::DEBUGGER {
                        match key.code {
                            KeyCode::Char('q') => break 'running,
                            KeyCode::Enter => {
                                if cpu.panic {
                                    warn!(
                                    "CPU in panic mode, cannot step. Reset using `r` (stuck at `{}`)",
                                    dbg.instruction_counter
                                );
                                }

                                dbg.paused = false
                            }
                            KeyCode::Char('p') => {
                                dbg.state = match dbg.state {
                                    ViewState::RAM => ViewState::IO,
                                    ViewState::IO => ViewState::LOG,
                                    ViewState::LOG => ViewState::RAM,
                                };
                            }
                            KeyCode::Char('h') => dbg.free_run = !dbg.free_run,
                            KeyCode::Char('l') => dbg.lockstep = !dbg.lockstep,
                            KeyCode::F(2) => {
                                dbg.free_run = true;
                                dbg.input_mode = InputMode::GAME;
                            }
                            KeyCode::Char('r') => {
                                warn!("CPU Reset");
                                cpu.load_rom(&rom.clone());
                                cpu.reset();
                                dbg.reset();
                            }
                            KeyCode::Char('i') => {
                                dbg.lockstep = true;
                                dbg.paused = true;
                                dbg.free_run = false;
                                let can_trigger = cpu.can_irq_trigger(cpu::IRQ_DEBUG1);
                                warn!("DEBUG1 IRQ Triggered => {can_trigger}");

                                if can_trigger {
                                    cpu.trigger_irq(cpu::IRQ_DEBUG1);
                                }
                            }
                            _ => {
                                warn!("Key: {:?}", key);
                            }
                        }
                    }
                }
            }
        }

        if !cpu.panic && (!dbg.paused || dbg.free_run) {
            if let Some(num) = cpu.dma_check() {
                cpu.dma_run(num);
            }

            if !cpu.halt || (cpu.halt && cpu.get_mode() == cpu::MODE_IRQ) {
                let cycles = cpu.cycle_count;
                cpu.execute(opcode);

                dt_cycles = cpu.cycle_count - cycles;
                dbg.instruction_counter += 1;
            } else {
                cpu.cycle_count += 1;
                warn!("CPU Halted");
            }

            if !dbg.free_run {
                dbg.paused = true;
            }
        }

        // Hdraw => 960
        // HBlank => 272
        // scanline => 1232
        // Vdraw => 160*scanline => 197120
        // VBlank => 68*scanline => 83776
        // refresh => Vdraw+VBlank => 280896

        timer_scanline += dt_cycles;

        if timer_scanline >= 1232 {
            timer_scanline -= 1232;
            let vcount = cpu.lcd.increment_vcount();

            if vcount == 0 {
            } else if vcount == 160 {
                if cpu.can_irq_trigger(cpu::IRQ_VBLANK) {
                    //dbg.lockstep = true;
                    //dbg.paused = true;
                    //dbg.free_run = false;
                    warn!("VBLANK IRQ Triggered");

                    cpu.trigger_irq(cpu::IRQ_VBLANK);
                }
            }
        }

        //  Although the drawing time is only 960 cycles (240*4),
        //  the H-Blank flag is "0" for a total of 1006 cycles. (GBATEK)
        if timer_scanline <= 1006 {
            cpu.lcd.set_dispstat_hblank(false); // Hdraw
        } else {
            cpu.lcd.set_dispstat_hblank(true); // Hblank
            if cpu.can_irq_trigger(cpu::IRQ_HBLANK) {
                //dbg.lockstep = true;
                //dbg.paused = true;
                //dbg.free_run = false;
                warn!("HBlank IRQ Triggered");

                cpu.trigger_irq(cpu::IRQ_HBLANK);
            }
        }
    }

    let end = Instant::now();

    dbg.exit();
    let cps = (cpu.cycle_count as f64) / (end.duration_since(start).as_secs_f64());
    println!("{cps:.0} CPS, {:.3} MHz", cps / 1000000.0);
}
