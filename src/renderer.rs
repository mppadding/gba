use sdl2::render::Texture;

pub struct RenderMessage {
    pub mode: u8,
    pub frame: bool,
}

/// Breaks 16 bit pixel color into 8 bit color components
/// Returns (r, g, b)
fn get_colors(pixel: u16) -> (u8, u8, u8) {
    // Pixel format = X BBBBB GGGGG RRRRR binary
    let red = (pixel & 0x1F) << 3;
    let green = ((pixel >> 5) & 0x1F) << 3;
    let blue = ((pixel >> 10) & 0x1F) << 3;

    (red as u8, green as u8, blue as u8)
}

/// Breaks 8 bit palette index into 8 bit color components
/// Returns (r, g, b)
fn get_palette_color(palette: &Vec<u8>, mode: u8, pixel: u8) -> (u8, u8, u8) {
    match mode {
        4 => {
            let addr = (pixel as usize) * 2;
            let color = ((palette[addr + 1] as u16) << 8) | palette[addr] as u16;
            get_colors(color)
        }
        _ => get_colors(0),
    }
}

pub fn draw_texture(msg: &RenderMessage, vram: &Vec<u8>, palette: &Vec<u8>, texture: &mut Texture) {
    //println!("Drawing in mode: {}", msg.mode);
    match msg.mode {
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
                            let addr = ((x + (y * 240)) * 2) as usize;
                            let pixel = ((vram[addr + 1] as u16) << 8) | vram[addr] as u16;

                            let (r, g, b) = get_colors(pixel);

                            buffer[offset] = r;
                            buffer[offset + 1] = g;
                            buffer[offset + 2] = b;
                        }
                    }
                })
                .expect("[SDL] Cannot fill texture");
        }
        4 => {
            let base = match msg.frame {
                false => 0x0000,
                true => 0xA000,
            };

            texture
                .with_lock(None, |buffer: &mut [u8], pitch: usize| {
                    for y in 0..160 {
                        for x in 0..240 {
                            let offset = y * pitch + x * 3;

                            let addr = base + (x + (y * 240));
                            let pixel = vram[addr];

                            let (r, g, b) = get_palette_color(palette, msg.mode, pixel);

                            buffer[offset] = r;
                            buffer[offset + 1] = g;
                            buffer[offset + 2] = b;
                        }
                    }
                })
                .expect("[SDL] Cannot fill texture");
        }
        5 => {
            texture
                .with_lock(None, |buffer: &mut [u8], pitch: usize| {
                    for y in 0..160 {
                        for x in 0..240 {
                            let offset = y * pitch + x * 3;

                            buffer[offset] = 0xFF;
                            buffer[offset + 1] = 0xFF;
                            buffer[offset + 2] = 0xFF;
                        }
                    }
                })
                .expect("[SDL] Cannot fill texture");
        }
        _ => panic!("Unknown LCD BG mode"),
    }
}

/*pub fn draw_texture(cpu: &mut CPU, texture: &mut Texture) {
    let mode = cpu.lcd.get_dispcnt_mode();
    match mode {
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
        4 => {
            let base = match cpu.lcd.get_dispcnt_frame() {
                false => 0x06000000,
                true => 0x0600A000,
            };

            texture
                .with_lock(None, |buffer: &mut [u8], pitch: usize| {
                    for y in 0..160 {
                        for x in 0..240 {
                            let offset = y * pitch + x * 3;

                            let addr = (base + (x + (y * 240))) as u32;
                            let pixel = cpu.read_u8(false, addr);

                            let (r, g, b) = get_palette_color(cpu, pixel);

                            buffer[offset] = r;
                            buffer[offset + 1] = g;
                            buffer[offset + 2] = b;
                        }
                    }
                })
                .expect("[SDL] Cannot fill texture");
        }
        5 => {}
        _ => panic!("Unknown LCD BG mode"),
    }
}*/
