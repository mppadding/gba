use sdl2::render::Texture;

use crate::cpu::{CPU, MMU};

/// Breaks 16 bit pixel color into 8 bit color components
/// Returns (r, g, b)
fn get_colors(pixel: u16) -> (u8, u8, u8) {
    // Pixel format = X BBBBB GGGGG RRRRR binary
    let red = (pixel & 0x1F) << 3;
    let green = ((pixel >> 5) & 0x1F) << 3;
    let blue = ((pixel >> 10) & 0x1F) << 3;

    (red as u8, green as u8, blue as u8)
}

pub fn draw_texture(cpu: &mut CPU, texture: &mut Texture) {
    match cpu.lcd.get_dispcnt_mode() {
        0 => {
            texture
                .with_lock(None, |buffer: &mut [u8], pitch: usize| {
                    for y in 0..160 {
                        for x in 0..240 {
                            let offset = y * pitch + x * 3;

                            buffer[offset] = 0xFF;
                            buffer[offset + 1] = 0x00;
                            buffer[offset + 2] = 0x00;
                        }
                    }
                })
                .expect("[SDL] Cannot fill texture");
        }
        1 => {
            texture
                .with_lock(None, |buffer: &mut [u8], pitch: usize| {
                    for y in 0..160 {
                        for x in 0..240 {
                            let offset = y * pitch + x * 3;

                            buffer[offset] = 0x00;
                            buffer[offset + 1] = 0xFF;
                            buffer[offset + 2] = 0x00;
                        }
                    }
                })
                .expect("[SDL] Cannot fill texture");
        }
        2 => {
            texture
                .with_lock(None, |buffer: &mut [u8], pitch: usize| {
                    for y in 0..160 {
                        for x in 0..240 {
                            let offset = y * pitch + x * 3;

                            buffer[offset] = 0x00;
                            buffer[offset + 1] = 0x00;
                            buffer[offset + 2] = 0xFF;
                        }
                    }
                })
                .expect("[SDL] Cannot fill texture");
        }
        3 => {
            texture
                .with_lock(None, |buffer: &mut [u8], pitch: usize| {
                    for y in 0..160 {
                        for x in 0..240 {
                            let offset = y * pitch + x * 3;
                            let addr = (0x06000000 + ((x + (y * 240)) * 2)) as u32;
                            let pixel = cpu.read_u16(false, addr);

                            let (r, g, b) = get_colors(pixel);

                            buffer[offset] = r;
                            buffer[offset + 1] = g;
                            buffer[offset + 2] = b;
                        }
                    }
                })
                .expect("[SDL] Cannot fill texture");
        }
        4 => {}
        5 => {}
        _ => panic!("Unknown LCD BG mode"),
    }
}
