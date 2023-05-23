use std::collections::HashSet;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Instant;

use cpu::MMU;
use debugger::Debugger;
use log::warn;

use crate::cpu::CPU;
use crate::debugger::DebuggerEvent;
use crate::game_window::{GameWindow, WindowEvent};
use crate::renderer::RenderMessage;

mod cpu;
mod debugger;
mod disassembler;
mod game_window;
mod keypad;
mod lcd;
mod renderer;
mod serial;
mod sound;

fn main() {
    let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
    let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));

    let mut cpu = CPU::new(&vram, &palette);
    cpu.reset();
    cpu.load_bios(&std::fs::read("bios/gba_bios.bin").unwrap());
    //let rom = std::fs::read("roms/pokemon_emerald.gba").unwrap();
    //let rom = std::fs::read("roms/super_dodgeball_advance.gba").unwrap();
    //let rom = std::fs::read("roms/super_mario_advance2.gba").unwrap();
    //let rom = std::fs::read("roms/super_mario_advance4.gba").unwrap();
    let rom = std::fs::read("roms/mario_kart_super_circuit.gba").unwrap();
    //let rom = std::fs::read("roms/rgb_test.gba").unwrap();
    //let rom = std::fs::read("roms/tonc/bm_modes.gba").unwrap();
    //let rom = std::fs::read("roms/tonc/irq_demo.gba").unwrap();
    //let rom = std::fs::read("roms/CPUTest.gba").unwrap();

    //let rom = std::fs::read("roms/tonc/key_demo.gba").unwrap(); // Works
    //let rom = std::fs::read("roms/tonc/pageflip.gba").unwrap(); // Works

    cpu.load_rom(&rom.clone());

    let mut dbg = Debugger::new();
    Debugger::set_panic_hook();
    dbg.breakpoints = HashSet::from([
        //0x00000000, 0x13c, 0x080002e0,
        //0x080016BC
        // 0x03000188,
        //0x08000492,
    ]);

    let (win_tx, win_rx) = mpsc::channel();
    let (game_tx, game_rx) = mpsc::channel();

    let window_handle = thread::spawn(move || {
        let mut window = GameWindow::new();

        let vram = Arc::clone(&vram);
        let palette = Arc::clone(&palette);

        loop {
            if let Some(events) = window.update() {
                win_tx.send(events).unwrap();
            }

            if let Ok(msg) = game_rx.try_recv() {
                if !window.paused {
                    window.draw(&msg, &vram.lock().unwrap(), &palette.lock().unwrap());
                }
            }
        }
    });

    dbg.draw(&cpu);

    let start = Instant::now();

    //dbg.free_run = false;
    //dbg.paused = true;
    //dbg.lockstep = true;

    let mut timer_scanline: usize = 0;
    let mut dt_cycles: usize = 0;
    let mut lcd_pause = false;

    'running: loop {
        let is_thumb = cpu.is_thumb();
        let program_counter = cpu.get_program_counter();
        let opcode: u32 = if !cpu.addr_valid(program_counter) {
            if !cpu.panic {
                warn!("Panicked! PC at invalid address `{:08X}`", program_counter);
                cpu.panic = true;
            }
            dbg.opcode
        } else {
            match is_thumb {
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

        match dbg.update(&mut cpu) {
            DebuggerEvent::None => {}
            DebuggerEvent::Quit => break 'running,
            DebuggerEvent::Reset => {
                warn!("CPU Reset");
                cpu.load_rom(&rom.clone());
                cpu.reset();
            }
        }

        if let Ok(events) = win_rx.try_recv() {
            for event in events {
                match event {
                    WindowEvent::Quit => break 'running,
                    WindowEvent::ButtonPress(button) => {
                        cpu.keypad.press(button);
                        println!(
                            "Press {button}, buttons:{:010b}",
                            cpu.keypad.keyinput & 0x3FF
                        );
                    }
                    WindowEvent::ButtonRelease(button) => {
                        cpu.keypad.release(button);
                        println!(
                            "Release {button}, buttons:{:010b}",
                            cpu.keypad.keyinput & 0x3FF
                        );
                    }
                    WindowEvent::Pause(paused) => lcd_pause = paused,
                    WindowEvent::NextVCount => timer_scanline = 1232,
                    _ => {}
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

        //frame_timer += dt;
        if !lcd_pause {
            timer_scanline += dt_cycles;
        }

        if timer_scanline >= 1232 {
            timer_scanline -= 1232;
            let vcount = cpu.lcd.increment_vcount();

            if vcount == 0 {
                game_tx
                    .send(RenderMessage {
                        mode: cpu.lcd.get_dispcnt_mode(),
                        frame: cpu.lcd.get_dispcnt_frame(),
                    })
                    .unwrap();
            } else if vcount == 160 {
                if dbg.free_run && cpu.can_irq_trigger(cpu::IRQ_VBLANK) {
                    //dbg.lockstep = true;
                    //dbg.paused = true;
                    //dbg.free_run = false;
                    warn!("VBLANK IRQ Triggered");

                    cpu.trigger_irq(cpu::IRQ_VBLANK);
                }
            }

            if dbg.free_run
                && cpu.lcd.get_dispstat_vcount_flag()
                && cpu.can_irq_trigger(cpu::IRQ_VCOUNT)
            {
                warn!("VCount IRQ Triggered");

                cpu.trigger_irq(cpu::IRQ_VCOUNT);
            }
        }

        //  Although the drawing time is only 960 cycles (240*4),
        //  the H-Blank flag is "0" for a total of 1006 cycles. (GBATEK)
        if timer_scanline <= 1006 {
            cpu.lcd.set_dispstat_hblank(false); // Hdraw
        } else {
            cpu.lcd.set_dispstat_hblank(true); // Hblank
            if dbg.free_run && cpu.can_irq_trigger(cpu::IRQ_HBLANK) {
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
