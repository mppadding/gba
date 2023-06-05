use std::cmp;

use log::warn;
use sdl2::{
    rect::Rect,
    render::{Canvas, Texture},
    video::Window,
};

#[derive(Debug)]
pub struct RenderMessage {
    pub mode: u8,
    pub frame: bool,
    pub bg_control: u16,
    pub bg_offset: (u16, u16),
}

#[derive(Debug)]
pub struct BackgroundMessage {
    pub control: u16,
    pub offset_x: u16,
    pub offset_y: u16,
    pub width: u16,
    pub height: u16,
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

fn draw_tile_4bpp(
    vram: &Vec<u8>,
    palette: &Vec<u8>,
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

    let tile_width = 4;
    let tile_height = 8;

    let x_base = x * 8;
    let y_base = y * 8;

    for tile_y in 0..tile_height {
        let row_base = match vertical_flip {
            false => base_tile + (tile_y * tile_width),
            true => base_tile + (((tile_height - 1) - tile_y) * tile_width),
        };

        let row = &vram[row_base..row_base + tile_width];

        for tile_x in 0..tile_width {
            let y_offset = (y_base + tile_y) * pitch;
            let x_offset = match horizontal_flip {
                false => (x_base + (tile_x * 2)) * 4,
                true => (x_base + (((tile_width - 1) - tile_x) * 2)) * 4,
            };

            let offset = y_offset + x_offset;
            let (left_pal, right_pal) = match horizontal_flip {
                false => ((row[tile_x] & 0x0F) | palbank, (row[tile_x] >> 4) | palbank),
                true => ((row[tile_x] >> 4) | palbank, (row[tile_x] & 0x0F) | palbank),
            };

            let left_color = get_palette_color(palette, 0, left_pal);
            let right_color = get_palette_color(palette, 0, right_pal);

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

fn draw_tile_8bpp(
    vram: &Vec<u8>,
    palette: &Vec<u8>,
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

    let base_tile = cbb_bytes + (tile_id * 64);

    // in 8-bit mode, single row is 8 bytes, for 8 rows total = 64 bytes

    let tile_width = 8;
    let tile_height = 8;

    let x_base = x * 8;
    let y_base = y * 8;

    for tile_y in 0..tile_height {
        // Get slice of row
        let row_base = match vertical_flip {
            false => base_tile + (tile_y * tile_width),
            true => base_tile + (((tile_height - 1) - tile_y) * tile_width),
        };
        let row = &vram[row_base..row_base + tile_width];

        for tile_x in 0..tile_width {
            let y_offset = (y_base + tile_y) * pitch;
            let x_offset = match horizontal_flip {
                false => (x_base + tile_x) * 4,
                true => (x_base + ((tile_width - 1) - tile_x)) * 4,
            };

            let offset = y_offset + x_offset;
            let color = get_palette_color(palette, 0, row[tile_x]);

            buffer[offset] = color.0;
            buffer[offset + 1] = color.1;
            buffer[offset + 2] = color.2;
            buffer[offset + 3] = color.3;
        }
    }
}

fn get_tile_from_vram(vram: &Vec<u8>, x: u16, y: u16, screen_base_block: u32) -> u16 {
    let block_size = 32;

    let mut row = y * block_size;
    let mut col = x;
    let mut sbb_page = 0;

    let x_overflow = x >= block_size;
    let y_overflow = y >= block_size;

    if x_overflow {
        col -= block_size;
        sbb_page += 1;
    }

    if y_overflow {
        row -= block_size;
        sbb_page += 1;
    }

    let sbb_addr = (screen_base_block + sbb_page) as usize * 0x800;
    let map_addr = sbb_addr + ((row + col) * 2) as usize;

    ((vram[map_addr + 1] as u16) << 8) | vram[map_addr] as u16
}

fn draw_tilemap(msg: &RenderMessage, vram: &Vec<u8>, palette: &Vec<u8>, texture: &mut Texture) {
    let character_base_block = ((msg.bg_control >> 2) & 0x3) as u32;
    let color_4bit = (msg.bg_control & 0x80) == 0x0;

    let cbb_bytes = (character_base_block * 16 * 1024) as usize;

    let palbank = 0;

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

    texture
        .with_lock(None, |buffer: &mut [u8], pitch: usize| {
            let max_y = match color_4bit {
                false => 8,
                true => 16,
            };

            for y in 0..max_y {
                for x in 0..8 {
                    let tile_id = ((y * 30) + x) & 0x3FF;
                    let tile = tile_id | palbank;

                    if color_4bit {
                        if tile_id < 512 {
                            draw_tile_4bpp(
                                vram, palette, buffer, x as usize, y as usize, cbb_bytes, pitch,
                                tile,
                            );
                        }
                    } else {
                        if tile_id < 256 {
                            draw_tile_8bpp(
                                vram, palette, buffer, x as usize, y as usize, cbb_bytes, pitch,
                                tile,
                            );
                        }
                    }
                }
            }
        })
        .expect("[SDL] Cannot fill texture");
}

pub fn draw_mode0(msg: &RenderMessage, vram: &Vec<u8>, palette: &Vec<u8>, texture: &mut Texture) {
    let character_base_block = ((msg.bg_control >> 2) & 0x3) as u32;
    let mosaic = (msg.bg_control & 0x40) != 0x00;
    let color_4bit = (msg.bg_control & 0x80) == 0x0;
    let screen_base_block = ((msg.bg_control >> 8) & 0x1F) as u32;
    let screen_size = (msg.bg_control >> 14) & 0x3;

    let cbb_bytes = (character_base_block * 16 * 1024) as usize;

    let (width, height) = match screen_size {
        0 => (256, 256),
        1 => (512, 256),
        2 => (256, 512),
        3 => (512, 512),
        _ => unreachable!(),
    };

    let (width_tiles, height_tiles) = (width / 8, height / 8);

    // Tiles are 8x8 pixels
    //
    // In 4 bit mode each tile has 32 bytes of memory. First 4 bytes for top most, etc
    // Each bytes is two pixels, lower nibble is left pixel, upper nibble is right pixel
    //
    // In 8 bit mode each tile has 64 bytes of memory. First 8 bytes for top most, etc
    // Each byte defines palette entry as per `get_palette_color`

    texture
        .with_lock(None, |buffer: &mut [u8], pitch: usize| {
            for y in 0..height_tiles {
                for x in 0..width_tiles {
                    let tile = get_tile_from_vram(vram, x, y, screen_base_block);

                    if color_4bit {
                        draw_tile_4bpp(
                            vram, palette, buffer, x as usize, y as usize, cbb_bytes, pitch, tile,
                        );
                    } else {
                        draw_tile_8bpp(
                            vram, palette, buffer, x as usize, y as usize, cbb_bytes, pitch, tile,
                        );
                    }
                }
            }
        })
        .expect("[SDL] Cannot fill texture");
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

/// Splits texture into 4 (src, dest) rects based on offset
///
/// Source rectangles (of texture):
///     1 | 2
///     --+--
///     3 | 4
///
/// Dest rectangles (of window):
///     4 | 3
///     --+--
///     2 | 1
///
/// The + indicates the offset coordinate
fn split_texture_rects(
    msg: &BackgroundMessage,
) -> (
    Option<(Rect, Rect)>,
    Option<(Rect, Rect)>,
    Option<(Rect, Rect)>,
    (Rect, Rect),
) {
    let offset_x = msg.offset_x % msg.width;
    let offset_y = msg.offset_y % msg.height;

    let width = msg.width as u32;
    let height = msg.height as u32;

    let win_width = 240;
    let win_height = 160;

    let w_right = width - (offset_x as u32);
    let h_bottom = height - (offset_y as u32);

    let offset_x = offset_x as i32;
    let offset_y = offset_y as i32;

    let w_right_window = cmp::min(w_right, win_width);
    let h_bottom_window = cmp::min(h_bottom, win_height);

    let r1 = match h_bottom_window < win_height && w_right_window < win_width {
        false => None,
        true => {
            let d_height = win_height - h_bottom_window;
            let d_width = win_width - w_right_window;

            Some((
                Rect::new(0, 0, d_width, d_height),
                Rect::new(
                    w_right_window as i32,
                    h_bottom_window as i32,
                    d_width,
                    d_height,
                ),
            ))
        }
    };

    let r2 = match h_bottom_window < win_height {
        false => None,
        true => {
            let d_height = win_height - h_bottom_window;
            Some((
                Rect::new(offset_x, 0, w_right_window, d_height),
                Rect::new(0, h_bottom_window as i32, w_right_window, d_height),
            ))
        }
    };

    let r3 = match w_right_window < win_width {
        false => None,
        true => {
            let d_width = win_width - w_right_window;
            Some((
                Rect::new(0, offset_y, d_width, h_bottom_window),
                Rect::new(w_right_window as i32, 0, d_width, h_bottom_window),
            ))
        }
    };

    let r4 = (
        Rect::new(offset_x, offset_y, w_right_window, h_bottom_window),
        Rect::new(0, 0, w_right_window, h_bottom_window),
    );

    (r1, r2, r3, r4)
}

pub fn render_background_to_canvas(
    canvas: &mut Canvas<Window>,
    background: &Texture,
    msg: &BackgroundMessage,
) {
    let (r1, r2, r3, r4) = split_texture_rects(msg);

    if let Some((src, dest)) = r1 {
        canvas
            .copy(background, Some(src), Some(dest))
            .expect("[SDL] Cannot copy background0");
    }

    if let Some((src, dest)) = r2 {
        canvas
            .copy(background, Some(src), Some(dest))
            .expect("[SDL] Cannot copy background0");
    }

    if let Some((src, dest)) = r3 {
        canvas
            .copy(background, Some(src), Some(dest))
            .expect("[SDL] Cannot copy background0");
    }

    canvas
        .copy(background, Some(r4.0), Some(r4.1))
        .expect("[SDL] Cannot copy background0");
}

pub fn get_texture_dimensions(msg: &RenderMessage) -> (u16, u16) {
    match msg.mode {
        0 => {
            let screen_size = (msg.bg_control >> 14) & 0x3;

            // Affinite? Gets textures up to 1024x1024

            match screen_size {
                0 => (256, 256),
                1 => (512, 256),
                2 => (256, 512),
                3 => (512, 512),
                _ => unreachable!(),
            }
        }
        1 => (240, 160),
        2 => (240, 160),
        3 | 4 | 5 => (240, 160),
        _ => unreachable!(),
    }
}

fn draw_mode3(texture: &mut Texture, vram: &Vec<u8>) {
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

fn draw_mode4(texture: &mut Texture, msg: &RenderMessage, vram: &Vec<u8>, palette: &Vec<u8>) {
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

                    let (a, r, g, b) = get_palette_color(palette, msg.mode, pixel);

                    buffer[offset] = a;
                    buffer[offset + 1] = r;
                    buffer[offset + 2] = g;
                    buffer[offset + 3] = b;
                }
            }
        })
        .expect("[SDL] Cannot fill texture");
}

fn draw_mode5(texture: &mut Texture, msg: &RenderMessage, vram: &Vec<u8>) {
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
                        // TODO: Change this to transparent when multiple backgrounds are a thing
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

pub fn draw_background(
    texture: &mut Texture,
    msg: &RenderMessage,
    vram: &Vec<u8>,
    palette: &Vec<u8>,
    bg: u8,
) {
    match msg.mode {
        0 => draw_mode0(msg, vram, palette, texture),
        1 => fill_texture_argb(texture, 240, 160, 0xFFFF0000),
        2 => fill_texture_argb(texture, 240, 160, 0xFF00FF00),
        3 => draw_mode3(texture, vram),
        4 => draw_mode4(texture, msg, vram, palette),
        5 => draw_mode5(texture, msg, vram),
        _ => panic!("Unknown LCD BG mode"),
    }
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
