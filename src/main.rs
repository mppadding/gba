use std::backtrace::Backtrace;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::process::{exit, Command};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;
use std::{panic, thread};

use cpu::MMU;
use debugger::Debugger;
use log::warn;

use crate::backtrace::print_cpu_backtrace;
use crate::backtrace::PC_BACKTRACE;
use crate::cpu::CPU;
use crate::debugger::{Breakpoint, DebuggerEvent};
use crate::game_window::{Dump, GameWindow, WindowEvent};
use crate::renderer::{BackgroundMessage, RenderMessage};

mod backtrace;
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
    panic::set_hook(Box::new(|panic_info| {
        let bt = Backtrace::capture();

        println!("{panic_info}");
        println!("Backtrace:\n{bt}");
        print_cpu_backtrace();
    }));

    let vram = Arc::new(Mutex::new(vec![0; 96 * 1024]));
    let palette = Arc::new(Mutex::new(vec![0; 1 * 1024]));
    let oam = Arc::new(Mutex::new(vec![0; 1 * 1024]));

    let mut cpu = CPU::new(&vram, &palette, &oam);
    cpu.reset();
    cpu.load_bios(&std::fs::read("bios/gba_bios.bin").unwrap());

    /* Halt ROM */
    //let rom: Vec<u8> = vec![0xFE, 0xFF, 0xFF, 0xEA];

    /* Games */
    // Implement CpuSet::fill halfword
    //let rom = std::fs::read("roms/pokemon_emerald.gba").unwrap();
    //let rom = std::fs::read("roms/super_mario_advance2.gba").unwrap();
    //let rom = std::fs::read("roms/super_mario_advance4.gba").unwrap();
    //let rom = std::fs::read("roms/mario_kart_super_circuit.gba").unwrap();

    // Super Dodgeball Advance:
    //      - Next step: Get past title screen :D
    //      - ATLUS should fade-in
    let rom = std::fs::read("roms/super_dodgeball_advance.gba").unwrap();

    /* Test ROMs */
    //let rom = std::fs::read("roms/rgb_test.gba").unwrap();
    //let rom = std::fs::read("roms/CPUTest.gba").unwrap();

    //let rom = std::fs::read("../gba_suite/deploy/suite.gba").unwrap();

    // SWI_VSYNC:
    //      obj_mode=11
    //let rom = std::fs::read("roms/tonc/swi_vsync.gba").unwrap();

    // BIG_MAP:
    //let rom = std::fs::read("roms/tonc/bigmap.gba").unwrap();

    // BLD_DEMO:
    //      Requires GFX_MODE=01 for sprites
    //let rom = std::fs::read("roms/tonc/bld_demo.gba").unwrap();

    // DMA_DEMO:
    //      - 'not yet implemented: Check HBlank DMA start timing => DMA3', src/cpu.rs:1037:25
    //let rom = std::fs::read("roms/tonc/dma_demo.gba").unwrap();

    //let rom = std::fs::read("roms/tonc/swi_demo.gba").unwrap();

    //let rom = std::fs::read("roms/tonc/tmr_demo.gba").unwrap();
    //let rom = std::fs::read("roms/tonc/txt_obj.gba").unwrap();
    //let rom = std::fs::read("roms/tonc/txt_se1.gba").unwrap();
    //let rom = std::fs::read("roms/tonc/txt_se2.gba").unwrap();
    //let rom = std::fs::read("roms/tonc/win_demo.gba").unwrap();

    /* TONC */
    // IRQ_DEMO:
    //      - Crashes on press X => A button
    //          panicked at 'not yet implemented: Halfword Data Transfer: register offset', src/cpu.rs:3093:21
    //          Should swap between (red, descending) and (green, ascending)
    //      - Nothing happens on press Z => B button
    //          Should switch priorities of HBlank and VCount
    //      - No rendering of interrupts
    //      - Wrong rendering of text
    //let rom = std::fs::read("roms/tonc/irq_demo.gba").unwrap();

    // TTE_DEMO:
    //      - Requires SWI 15h
    //let rom = std::fs::read("roms/tonc/tte_demo.gba").unwrap();

    // TXT_BM:
    //let rom = std::fs::read("roms/tonc/txt_bm.gba").unwrap();

    // SBB_REG:
    //      - Sprite being rendered
    //          Apparently sprites are turned on in DISPCNT
    //          and OAM is not cleared, which means
    //          that every sprite will be rendered.
    //let rom = std::fs::read("roms/tonc/sbb_reg.gba").unwrap();

    /* Working ROMs */
    //let rom = std::fs::read("roms/tonc/key_demo.gba").unwrap();
    //let rom = std::fs::read("roms/tonc/pageflip.gba").unwrap();
    //let rom = std::fs::read("roms/tonc/bm_modes.gba").unwrap();
    //let rom = std::fs::read("roms/tonc/m3_demo.gba").unwrap();
    //let rom = std::fs::read("roms/tonc/brin_demo.gba").unwrap();
    //let rom = std::fs::read("roms/tonc/obj_demo.gba").unwrap();
    cpu.load_rom(&rom.clone());

    let mut dbg = Debugger::new();
    Debugger::set_panic_hook();
    dbg.breakpoints = HashMap::from([
        //(0x08000620, Breakpoint::Delay(3, 0)),
        //(0x08000624, Breakpoint::Delay(3, 0)),
        (0x080003B8, Breakpoint::Delay(13, 0)),
        //0x08000620, // m3_line in bottom left frame
        //0x08000624,
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

            if let Ok(mut msg) = game_rx.try_recv() {
                if !window.paused {
                    window.draw(
                        &mut msg,
                        &vram.lock().unwrap(),
                        &palette.lock().unwrap(),
                        &oam.lock().unwrap(),
                    );
                }
            }
        }
    });

    dbg.draw(&mut cpu);

    let start = Instant::now();

    //dbg.free_run = false;
    //dbg.paused = true;
    //dbg.lockstep = true;

    let mut timer_scanline: usize = 0;
    let mut dt_cycles: usize = 0;
    let mut lcd_pause = false;
    let mut previous_pc = 0x08000000;

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

        #[cfg(feature = "debugger")]
        if !cpu.panic && dbg.should_break(program_counter) {
            warn!("Breakpoint hit at `{:08X}`", program_counter);
            dbg.free_run = false;
            dbg.paused = true;
            dbg.lockstep = true;
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
            DebuggerEvent::Back => {
                warn!("Jumping 1 instruction back");
                cpu.set_program_counter(previous_pc);
            }
        }

        if let Ok(events) = win_rx.try_recv() {
            for event in events {
                match event {
                    WindowEvent::Quit => break 'running,
                    WindowEvent::ButtonPress(button) => {
                        cpu.keypad.press(button);
                        warn!(
                            "Press 0x{button:X}, buttons:{:010b}",
                            cpu.keypad.keyinput & 0x3FF
                        );
                    }
                    WindowEvent::ButtonRelease(button) => {
                        cpu.keypad.release(button);
                        warn!(
                            "Release 0x{button:X}, buttons:{:010b}",
                            cpu.keypad.keyinput & 0x3FF
                        );
                    }
                    WindowEvent::Pause(paused) => lcd_pause = paused,
                    WindowEvent::NextVCount => timer_scanline = 1232,
                    WindowEvent::ForceRender => {
                        timer_scanline = 1232;
                        cpu.lcd.set_vcount(227);
                    }
                    WindowEvent::Debug(1) => {
                        dbg.free_run = false;
                        dbg.paused = true;
                        dbg.lockstep = true;

                        warn!("Debug(1) pressed, trigger IRQ_VBLANK");
                        cpu.trigger_irq(cpu::IRQ_VBLANK);
                    }
                    WindowEvent::Debug(2) => {
                        dbg.free_run = false;
                        dbg.paused = true;
                        dbg.lockstep = true;

                        warn!("Debug(2) pressed, trigger IRQ_HBLANK");
                        cpu.trigger_irq(cpu::IRQ_HBLANK);
                    }
                    WindowEvent::Debug(3) => {
                        dbg.free_run = false;
                        dbg.paused = true;
                        dbg.lockstep = true;
                    }
                    WindowEvent::Dump(d) => {
                        let (file, data) = match d {
                            Dump::Video => ("vram", cpu.ram_video.lock().unwrap()),
                            Dump::Palette => ("pal", cpu.ram_palette.lock().unwrap()),
                            _ => todo!("Implement dump {d:#?}"),
                        };

                        println!("Dump {file}");

                        // Dump to path
                        let path = format!("dump/{file}");
                        let mut file = File::create(&path).expect("Could not create `{path}`");
                        file.write_all(&data).expect("Failed to write to `{path}`");

                        // Create hex file
                        Command::new("sh")
                            .arg("-c")
                            .arg(format!("xxd {path} >| {path}.hex"))
                            .output()
                            .expect("Failed to execute xxd");
                    }
                    _ => {
                        warn!("Unhandled WindowEvent `{event:#?}`");
                    }
                }
            }
        }

        if !cpu.panic && (!dbg.paused || dbg.free_run) {
            if let Some(num) = cpu.dma_check() {
                cpu.dma_run(num);
            }

            if !cpu.halt || (cpu.halt && cpu.get_mode() == cpu::MODE_IRQ) {
                #[cfg(feature = "backtrace")]
                {
                    let (asm, _) = match is_thumb {
                        false => disassembler::disassemble_arm(opcode, program_counter),
                        true => disassembler::disassemble_thumb(opcode as u16),
                    };
                    #[cfg(feature = "full-backtrace")]
                    let asm_reg = backtrace::replace_registers_in_string(&cpu, &asm);

                    #[cfg(not(feature = "full-backtrace"))]
                    let asm_reg = String::from("full-backtrace disabled");

                    // Unsafe due to static mut PC_BACKTRACE
                    unsafe {
                        if PC_BACKTRACE.len() == 32 {
                            PC_BACKTRACE.pop_back();
                        }

                        PC_BACKTRACE.push_front((program_counter, opcode, is_thumb, asm, asm_reg));
                    }
                }

                previous_pc = program_counter;

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
                        dispcnt: cpu.lcd.get_dispcnt(),
                        frame: cpu.lcd.get_dispcnt_frame(),
                        backgrounds: [
                            BackgroundMessage {
                                control: cpu.lcd.get_background_control(0),
                                offset: cpu.lcd.get_background_offset(0),
                                width: 0,
                                height: 0,
                            },
                            BackgroundMessage {
                                control: cpu.lcd.get_background_control(1),
                                offset: cpu.lcd.get_background_offset(1),
                                width: 0,
                                height: 0,
                            },
                            BackgroundMessage {
                                control: cpu.lcd.get_background_control(2),
                                offset: cpu.lcd.get_background_offset(2),
                                width: 0,
                                height: 0,
                            },
                            BackgroundMessage {
                                control: cpu.lcd.get_background_control(3),
                                offset: cpu.lcd.get_background_offset(3),
                                width: 0,
                                height: 0,
                            },
                        ],
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
                warn!("HBlank IRQ Triggered");

                cpu.trigger_irq(cpu::IRQ_HBLANK);
            }
        }

        if dbg.lockstep && !dbg.free_run {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    let end = Instant::now();

    dbg.exit();
    let cps = (cpu.cycle_count as f64) / (end.duration_since(start).as_secs_f64());
    println!("{cps:.0} CPS, {:.3} MHz", cps / 1000000.0);
}
