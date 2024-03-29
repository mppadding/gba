use std::cmp::Ordering;

use log::info;
use sdl2::{
    event::Event,
    keyboard::Keycode,
    pixels::PixelFormatEnum,
    rect::Rect,
    render::{Canvas, Texture, TextureCreator},
    video::{Window, WindowContext},
    EventPump,
};

use crate::{
    keypad,
    renderer::{self, RenderMessage},
};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Dump {
    Video,
    RAM,
    Palette,
    Object,
    Full,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum WindowEvent {
    Quit,
    ButtonPress(u16),
    ButtonRelease(u16),
    Pause(bool),
    NextVCount,
    Debug(u8),
    Dump(Dump),
    ForceRender,
}

pub struct GameWindow {
    canvas: Canvas<Window>,
    pub texture_creator: TextureCreator<WindowContext>,
    pub event_pump: EventPump,
    pub paused: bool,
}

impl GameWindow {
    pub fn new() -> Self {
        let sdl_context = sdl2::init().expect("[SDL] Failed to create context");
        let video_subsystem = sdl_context
            .video()
            .expect("[SDL] Failed to get video subsystem");

        let width = 240;
        let height = 160;

        //let width = 512;
        //let height = 512;

        let window = video_subsystem
            .window("pGBA", width, height)
            .opengl()
            .position(0, 0)
            .build()
            .map_err(|e| e.to_string())
            .expect("[SDL] Failed to create window");

        let canvas = window
            .into_canvas()
            .build()
            .map_err(|e| e.to_string())
            .expect("[SDL] Failed to get canvas");

        let texture_creator = canvas.texture_creator();

        let event_pump = sdl_context
            .event_pump()
            .expect("[SDL] Failed to get event pump");

        Self {
            canvas,
            texture_creator,
            event_pump,
            paused: false,
        }
    }

    pub fn draw(
        &mut self,
        msg: &mut RenderMessage,
        vram: &Vec<u8>,
        palette: &Vec<u8>,
        oam: &Vec<u8>,
    ) {
        self.canvas.clear();

        let mode = msg.dispcnt & 0x7;

        // (bg num, priority)
        let mut bg_order: [(u8, u8); 4] = [
            (0, (msg.backgrounds[0].control & 0b11) as u8),
            (1, (msg.backgrounds[1].control & 0b11) as u8),
            (2, (msg.backgrounds[2].control & 0b11) as u8),
            (3, (msg.backgrounds[3].control & 0b11) as u8),
        ];

        bg_order.sort_by(|a, b| {
            if a.1 > b.1 {
                Ordering::Greater
            } else if a.1 < b.1 {
                Ordering::Less
            } else {
                if a.0 > b.0 {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            }
        });

        /*
         * Draw BG 0
         */
        for (i, _) in bg_order {
            // Only draw if BG is on in DISPCNT
            if (msg.dispcnt & (0x0100 << i)) == 0 {
                continue;
            }

            // Not every background is enabled in every mode
            match (mode, i) {
                (0, _) => {}
                (1, 0) | (1, 1) | (1, 2) => {}
                (2, 2) | (2, 3) => {}
                (3, 2) => {}
                (4, 2) => {}
                (5, 2) => {}
                (_, _) => continue,
            }

            let (width, height) = renderer::get_texture_dimensions(msg, i as usize);
            msg.backgrounds[i as usize].width = width;
            msg.backgrounds[i as usize].height = height;

            let mut bg = self
                .texture_creator
                .create_texture_streaming(PixelFormatEnum::BGRA8888, width as u32, height as u32)
                .map_err(|e| e.to_string())
                .expect("[SDL] Cannot create texture");
            bg.set_blend_mode(sdl2::render::BlendMode::Blend);

            renderer::draw_background(&mut bg, msg, vram, palette, 0);
            renderer::render_background_to_canvas(
                &mut self.canvas,
                &bg,
                &msg.backgrounds[i as usize],
            );
        }

        // Sprites
        if msg.dispcnt & 0x1000 != 0 {
            let mut sprites = self
                .texture_creator
                .create_texture_streaming(PixelFormatEnum::BGRA8888, 128, 128)
                .map_err(|e| e.to_string())
                .expect("[SDL] Cannot create texture");
            sprites.set_blend_mode(sdl2::render::BlendMode::Blend);
            renderer::draw_and_render_sprites(
                &mut self.canvas,
                &mut sprites,
                msg,
                vram,
                palette,
                oam,
            );
        }

        // Render to canvas
        self.canvas.present();
    }

    pub fn update(&mut self) -> Option<Vec<WindowEvent>> {
        let mut events = Vec::new();
        for event in self.event_pump.poll_iter() {
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
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => return Some(vec![WindowEvent::Quit]),
                Event::KeyDown {
                    keycode: Some(keycode),
                    ..
                } => match keycode {
                    Keycode::X => events.push(WindowEvent::ButtonPress(keypad::BUTTON_A)),
                    Keycode::Z => events.push(WindowEvent::ButtonPress(keypad::BUTTON_B)),
                    Keycode::Backspace => {
                        events.push(WindowEvent::ButtonPress(keypad::BUTTON_SELECT))
                    }
                    Keycode::Return => events.push(WindowEvent::ButtonPress(keypad::BUTTON_START)),
                    Keycode::Right => events.push(WindowEvent::ButtonPress(keypad::BUTTON_RIGHT)),
                    Keycode::Left => events.push(WindowEvent::ButtonPress(keypad::BUTTON_LEFT)),
                    Keycode::Up => events.push(WindowEvent::ButtonPress(keypad::BUTTON_UP)),
                    Keycode::Down => events.push(WindowEvent::ButtonPress(keypad::BUTTON_DOWN)),
                    Keycode::S => events.push(WindowEvent::ButtonPress(keypad::BUTTON_R)),
                    Keycode::A => events.push(WindowEvent::ButtonPress(keypad::BUTTON_L)),
                    Keycode::P => events.push(WindowEvent::Dump(Dump::Palette)),
                    Keycode::N => events.push(WindowEvent::NextVCount),
                    Keycode::R => events.push(WindowEvent::ForceRender),
                    Keycode::F1 => events.push(WindowEvent::Debug(1)),
                    Keycode::F2 => events.push(WindowEvent::Debug(2)),
                    Keycode::F3 => events.push(WindowEvent::Debug(3)),
                    Keycode::V => events.push(WindowEvent::Dump(Dump::Video)),
                    _ => {}
                },
                Event::KeyUp {
                    keycode: Some(keycode),
                    ..
                } => match keycode {
                    Keycode::X => events.push(WindowEvent::ButtonRelease(keypad::BUTTON_A)),
                    Keycode::Z => events.push(WindowEvent::ButtonRelease(keypad::BUTTON_B)),
                    Keycode::Backspace => {
                        events.push(WindowEvent::ButtonRelease(keypad::BUTTON_SELECT))
                    }
                    Keycode::Return => {
                        events.push(WindowEvent::ButtonRelease(keypad::BUTTON_START))
                    }
                    Keycode::Right => events.push(WindowEvent::ButtonRelease(keypad::BUTTON_RIGHT)),
                    Keycode::Left => events.push(WindowEvent::ButtonRelease(keypad::BUTTON_LEFT)),
                    Keycode::Up => events.push(WindowEvent::ButtonRelease(keypad::BUTTON_UP)),
                    Keycode::Down => events.push(WindowEvent::ButtonRelease(keypad::BUTTON_DOWN)),
                    Keycode::S => events.push(WindowEvent::ButtonRelease(keypad::BUTTON_R)),
                    Keycode::A => events.push(WindowEvent::ButtonRelease(keypad::BUTTON_L)),
                    _ => {}
                },
                _ => {}
            }
        }

        match events.is_empty() {
            false => Some(events),
            true => None,
        }
    }
}
