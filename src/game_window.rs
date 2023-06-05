use log::info;
use sdl2::{
    event::Event,
    keyboard::Keycode,
    pixels::PixelFormatEnum,
    rect::Rect,
    render::{Canvas, TextureCreator},
    video::{Window, WindowContext},
    EventPump,
};

use crate::{
    keypad,
    renderer::{self, RenderMessage},
};

pub struct GameWindow {
    canvas: Canvas<Window>,
    pub texture_creator: TextureCreator<WindowContext>,
    pub event_pump: EventPump,
    pub paused: bool,
}

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

impl GameWindow {
    pub fn new() -> Self {
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

    pub fn draw(&mut self, msg: &RenderMessage, vram: &Vec<u8>, palette: &Vec<u8>, oam: &Vec<u8>) {
        self.canvas.clear();

        /*
         * Draw BG 0
         */
        let (bg0_w, bg0_h) = renderer::get_texture_dimensions(msg);
        let mut bg0 = self
            .texture_creator
            .create_texture_streaming(PixelFormatEnum::BGRA8888, bg0_w as u32, bg0_h as u32)
            .map_err(|e| e.to_string())
            .expect("[SDL] Cannot create texture");

        let (offset_x, offset_y) = msg.bg_offset;
        renderer::draw_background(&mut bg0, msg, vram, palette, 0);
        renderer::render_background_to_canvas(
            &mut self.canvas,
            &bg0,
            &renderer::BackgroundMessage {
                control: msg.bg_control,
                offset_x,
                offset_y,
                width: bg0_w,
                height: bg0_h,
            },
        );

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
                    Keycode::P => {
                        self.paused = !self.paused;
                        info!("LCD Paused={}", self.paused);
                        events.push(WindowEvent::Pause(self.paused));
                    }
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
