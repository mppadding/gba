use log::warn;
use sdl2::render::Texture;

#[derive(Debug)]
pub struct RenderMessage {
    pub mode: u8,
    pub frame: bool,
    pub bg_control: u16,
    pub bg_offset: (u16, u16),
}

/// Breaks 16 bit pixel color into 8 bit color components
/// Returns (a, r, g, b)
fn get_colors(pixel: u16) -> (u8, u8, u8, u8) {
    // Pixel format = X BBBBB GGGGG RRRRR binary
    let red = ((pixel & 0x1F) * 255) / 31;
    let green = (((pixel >> 5) & 0x1F) * 255) / 31;
    let blue = (((pixel >> 10) & 0x1F) * 255) / 31;

    (0xFF, red as u8, green as u8, blue as u8)
}

/// Breaks 8 bit palette index into 8 bit color components
/// Returns (a, r, g, b)
fn get_palette_color(palette: &Vec<u8>, mode: u8, pixel: u8) -> (u8, u8, u8, u8) {
    match mode {
        0 => {
            if pixel == 0 {
                return (0x00, 0x00, 0x00, 0x00);
            }

            let addr = (pixel as usize) * 2;
            let color = ((palette[addr + 1] as u16) << 8) | palette[addr] as u16;
            get_colors(color)
        }
        4 => {
            let addr = (pixel as usize) * 2;
            let color = ((palette[addr + 1] as u16) << 8) | palette[addr] as u16;
            get_colors(color)
        }
        _ => get_colors(0),
    }
}

fn draw_tile(
    msg: &RenderMessage,
    vram: &Vec<u8>,
    palette: &Vec<u8>,
    oam: &Vec<u8>,
    buffer: &mut [u8],
    x: usize,
    y: usize,
    cbb_bytes: usize,
    pitch: usize,
    tile: u16,
) {
    let tile_id = (tile & 0x3FF) as usize;
    let horizontal_flip = (tile & 0x400) != 0;
    let vertical_flip = (tile & 0x800) != 0;
    let palbank = ((tile >> 8) & 0xF0) as u8;

    let base_tile = cbb_bytes + (tile_id * 32);

    // in 4-bit mode, single row is 4 bytes, for 8 rows total = 32 bytes

    for tile_y in 0..8 {
        let row_base = base_tile + (tile_y * 4);
        let row = &vram[row_base..row_base + 4];

        for tile_x in 0..4 {
            let y_offset = (tile_y + (y * 8)) * pitch;
            let offset = y_offset + ((tile_x * 2) + (x * 8)) * 4;
            let left_color = get_palette_color(palette, 0, (row[tile_x] & 0x0F) | palbank);
            let right_color = get_palette_color(palette, 0, (row[tile_x] >> 4) | palbank);

            buffer[offset] = left_color.0;
            buffer[offset + 1] = left_color.1;
            buffer[offset + 2] = left_color.2;
            buffer[offset + 3] = left_color.3;

            buffer[offset + 4] = right_color.0;
            buffer[offset + 5] = right_color.1;
            buffer[offset + 6] = right_color.2;
            buffer[offset + 7] = right_color.3;
        }
    }
}

pub fn draw_mode0(
    msg: &RenderMessage,
    vram: &Vec<u8>,
    palette: &Vec<u8>,
    oam: &Vec<u8>,
    texture: &mut Texture,
) {
    let priority = msg.bg_control & 0x3;
    let character_base_block = ((msg.bg_control >> 2) & 0x3) as u32;
    let mosaic = (msg.bg_control & 0x40) != 0x00;
    let color_4bit = (msg.bg_control & 0x80) == 0x0;
    let screen_base_block = ((msg.bg_control >> 8) & 0x1F) as u32;
    let screen_size = (msg.bg_control >> 14) & 0x3;

    let cbb_bytes = (character_base_block * 16 * 1024) as usize;
    let sbb_bytes = (screen_base_block * 2 * 1024) as usize;

    let (offset_x, offset_y) = msg.bg_offset;

    let (width, height) = match screen_size {
        0 => (256, 256),
        1 => (512, 256),
        2 => (256, 512),
        3 => (512, 512),
        _ => unreachable!(),
    };

    if width < (offset_x + 240) {
        todo!("X wrap around");
    }

    if height < (offset_y + 160) {
        todo!("Y wrap around");
    }

    let (offset_x, offset_y) = (offset_x / 8, offset_y / 8);

    println!("Draw mode0: prio={priority}, cbb={character_base_block}, mosaic={mosaic}, 4bit={color_4bit}, sbb={screen_base_block}, size={screen_size}, cbb_bytes={cbb_bytes:X}, sbb_bytes={sbb_bytes:X}");
    println!("\tOffset: ({offset_x}, {offset_y}), Internal Screen Dimension={width}x{height}");

    // Tiles are 8x8 pixels
    //
    // In 4 bit mode each tile has 32 bytes of memory. First 4 bytes for top most, etc
    // Each bytes is two pixels, lower nibble is left pixel, upper nibble is right pixel
    //
    // In 8 bit mode each tile has 64 bytes of memory. First 8 bytes for top most, etc
    // Each byte defines palette entry as per `get_palette_color`

    // Clear with black
    texture
        .with_lock(None, |buffer: &mut [u8], pitch: usize| {
            for y in 0..160 {
                for x in 0..240 {
                    let offset = y * pitch + x * 4;

                    buffer[offset] = 0xFF;
                    buffer[offset + 1] = 0x00;
                    buffer[offset + 2] = 0x00;
                    buffer[offset + 3] = 0x00;
                }
            }
        })
        .expect("[SDL] Cannot fill texture");

    if !color_4bit {
        println!("implement 8bpp tile_set drawing");
        return;
    }

    // Tiles are 8x8 pixels
    //
    // In 4 bit mode each tile has 32 bytes of memory. First 4 bytes for top most, etc
    // Each bytes is two pixels, lower nibble is left pixel, upper nibble is right pixel

    texture
        .with_lock(None, |buffer: &mut [u8], pitch: usize| {
            for y in 0..20 {
                for x in 0..30 {
                    // 32x32 per screenblock, a screen entry is 2 bytes wide.
                    let mut row = (y + offset_y) * 32;
                    let mut col = x + offset_x;
                    let mut sbb_page = 0;

                    let x_overflow = (x + offset_x) >= 32;
                    let y_overflow = (y + offset_y) >= 32;

                    if x_overflow {
                        col -= 32;
                        sbb_page += 1;
                    }

                    if y_overflow {
                        row -= 32;
                        sbb_page += 1;
                    }

                    let sbb_addr = (screen_base_block + sbb_page) as usize * 0x800;
                    let map_addr = sbb_addr + ((row + col) * 2) as usize;

                    let tile = ((vram[map_addr + 1] as u16) << 8) | vram[map_addr] as u16;
                    draw_tile(
                        msg, vram, palette, oam, buffer, x as usize, y as usize, cbb_bytes, pitch,
                        tile,
                    );
                }
            }
        })
        .expect("[SDL] Cannot fill texture");
}

pub fn draw_texture(
    msg: &RenderMessage,
    vram: &Vec<u8>,
    palette: &Vec<u8>,
    oam: &Vec<u8>,
    texture: &mut Texture,
) {
    let mode = msg.mode;
    //let mode = 5;

    //println!("Drawing in mode: {}", msg.mode);
    warn!("Drawing in mode: {}", msg.mode);
    println!("Drawing for RenderMessage: {msg:#?}");
    //draw_tile_set(msg, vram, palette, oam, texture, true, 0);
    //return;

    match mode {
        0 => {
            draw_mode0(msg, vram, palette, oam, texture);
        }
        1 => {
            texture
                .with_lock(None, |buffer: &mut [u8], pitch: usize| {
                    for y in 0..160 {
                        for x in 0..240 {
                            let offset = y * pitch + x * 4;

                            buffer[offset] = 0xFF;
                            buffer[offset + 1] = 0xFF;
                            buffer[offset + 2] = 0x00;
                            buffer[offset + 3] = 0x00;
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
                            let offset = y * pitch + x * 4;

                            buffer[offset] = 0xFF;
                            buffer[offset + 1] = 0x00;
                            buffer[offset + 2] = 0x00;
                            buffer[offset + 3] = 0xFF;
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
                            let offset = y * pitch + x * 4;
                            let addr = ((x + (y * 240)) * 2) as usize;
                            let pixel = ((vram[addr + 1] as u16) << 8) | vram[addr] as u16;

                            let (a, r, g, b) = get_colors(pixel);

                            buffer[offset] = a;
                            buffer[offset + 1] = r;
                            buffer[offset + 2] = g;
                            buffer[offset + 3] = b;
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
                            let offset = y * pitch + x * 4;

                            let addr = base + (x + (y * 240));
                            let pixel = vram[addr];

                            let (a, r, g, b) = get_palette_color(palette, mode, pixel);

                            buffer[offset] = a;
                            buffer[offset + 1] = r;
                            buffer[offset + 2] = g;
                            buffer[offset + 3] = b;
                        }
                    }
                })
                .expect("[SDL] Cannot fill texture");
        }
        5 => {
            let base = match msg.frame {
                false => 0x0000,
                true => 0xA000,
            };

            texture
                .with_lock(None, |buffer: &mut [u8], pitch: usize| {
                    for y in 0..160 {
                        for x in 0..240 {
                            let offset = y * pitch + x * 4;

                            if y < 128 && x < 160 {
                                let addr = base + ((x + (y * 160)) * 2) as usize;
                                let pixel = ((vram[addr + 1] as u16) << 8) | vram[addr] as u16;

                                let (a, r, g, b) = get_colors(pixel);

                                buffer[offset] = a;
                                buffer[offset + 1] = r;
                                buffer[offset + 2] = g;
                                buffer[offset + 3] = b;
                            } else {
                                buffer[offset] = 0xFF;
                                buffer[offset + 1] = 0xFF;
                                buffer[offset + 2] = 0x00;
                                buffer[offset + 3] = 0xFF;
                            }
                        }
                    }
                })
                .expect("[SDL] Cannot fill texture");
        }
        _ => panic!("Unknown LCD BG mode"),
    }
}

fn fill_texture_argb(texture: &mut Texture, width: usize, height: usize, argb: u32) {
    let a = ((argb >> 24) & 0xFF) as u8;
    let r = ((argb >> 16) & 0xFF) as u8;
    let g = ((argb >> 8) & 0xFF) as u8;
    let b = ((argb >> 0) & 0xFF) as u8;

    texture
        .with_lock(None, |buffer: &mut [u8], pitch: usize| {
            for y in 0..height {
                for x in 0..width {
                    let offset = y * pitch + x * 4;

                    buffer[offset] = a;
                    buffer[offset + 1] = r;
                    buffer[offset + 2] = g;
                    buffer[offset + 3] = b;
                }
            }
        })
        .expect("[SDL] Cannot fill texture");
}

/// Fills 4 quadrant with colors
///     1 | 2
///     --+--
///     3 | 4
///
///     1 => Red
///     2 => Green
///     3 => Blue
///     4 => White
pub fn test_draw_background(texture: &mut Texture, width: usize, height: usize) {
    // Fill texture with black
    fill_texture_argb(texture, width, height, 0xFF000000);

    texture
        .with_lock(None, |buffer: &mut [u8], pitch: usize| {
            for y in 0..height {
                for x in 0..width {
                    let offset = y * pitch + x * 4;

                    let (r, g, b) = match (y >= (height / 2), x >= (width / 2)) {
                        (false, false) => (0xFF, 0, 0),
                        (false, true) => (0, 0xFF, 0),
                        (true, false) => (0, 0, 0xFF),
                        (true, true) => (0xFF, 0xFF, 0xFF),
                    };

                    buffer[offset] = 0xFF;
                    buffer[offset + 1] = r;
                    buffer[offset + 2] = g;
                    buffer[offset + 3] = b;
                }
            }
        })
        .expect("[SDL] Cannot fill texture");
}
