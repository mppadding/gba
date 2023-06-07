use std::cmp;

use sdl2::{
    rect::{Point, Rect},
    render::{Canvas, Texture},
    video::Window,
};

#[derive(Debug)]
pub struct RenderMessage {
    pub dispcnt: u16,
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

struct ObjectAttributes {
    attr0: u16,
    attr1: u16,
    attr2: u16,
    fill: i16,
}

impl ObjectAttributes {
    fn get_size(&self) -> (usize, usize) {
        let shape = ((self.attr0 >> 14) & 0x3) as u8;
        let size = ((self.attr1 >> 14) & 0x3) as u8;

        match (shape, size) {
            (0, 0) => (8, 8),
            (0, 1) => (16, 16),
            (0, 2) => (32, 32),
            (0, 3) => (64, 64),
            //
            (1, 0) => (16, 8),
            (1, 1) => (32, 8),
            (1, 2) => (32, 16),
            (1, 3) => (64, 32),
            //
            (2, 0) => (8, 16),
            (2, 1) => (8, 32),
            (2, 2) => (16, 32),
            (2, 3) => (32, 64),
            (_, _) => panic!("Invalid Mode/shape for sprite `shape={shape:2b}, size={size:2b}`"),
        }
    }

    fn get_tile_id(&self) -> u16 {
        self.attr2 & 0x3FF
    }

    fn get_priority(&self) -> u8 {
        ((self.attr2 >> 10) & 0x3) as u8
    }

    fn get_palbank(&self) -> u8 {
        (self.attr2 >> 12) as u8
    }
}

fn get_obj_attr(oam: &Vec<u8>, n: usize) -> ObjectAttributes {
    let offset = n * 8;
    ObjectAttributes {
        attr0: ((oam[offset + 1] as u16) << 8) | (oam[offset + 0] as u16),
        attr1: ((oam[offset + 3] as u16) << 8) | (oam[offset + 2] as u16),
        attr2: ((oam[offset + 5] as u16) << 8) | (oam[offset + 4] as u16),
        fill: (((oam[offset + 7] as u16) << 8) | (oam[offset + 6] as u16)) as i16,
    }
}

fn get_obj_affine(oam: &Vec<u8>, n: usize) -> [ObjectAttributes; 4] {
    let offset = n * 4;

    [
        get_obj_attr(oam, offset + 0),
        get_obj_attr(oam, offset + 1),
        get_obj_attr(oam, offset + 2),
        get_obj_attr(oam, offset + 3),
    ]
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

fn get_sprite_palette_color(palette: &Vec<u8>, pixel: u8, color_4bit: bool) -> (u8, u8, u8, u8) {
    if pixel == 0 || (color_4bit && (pixel & 0xF) == 0) {
        return (0x00, 0x00, 0x00, 0x00);
    }

    let addr: usize = 0x200 + (pixel as usize) * 2;
    let color = ((palette[addr + 1] as u16) << 8) | palette[addr] as u16;
    get_colors(color)
}

/// Breaks 8 bit palette index into 8 bit color components
/// Returns (a, r, g, b)
fn get_palette_color(palette: &Vec<u8>, mode: u8, pixel: u8, color_4bit: bool) -> (u8, u8, u8, u8) {
    match mode {
        0 => {
            if pixel == 0 || (color_4bit && (pixel & 0xF) == 0) {
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

/// Draw tile in 4 bits per pixel color mode
/// x & y are in pixels, not tiles
fn draw_tile_4bpp(
    vram: &Vec<u8>,
    palette: &Vec<u8>,
    buffer: &mut [u8],
    x: usize,
    y: usize,
    cbb_bytes: usize,
    pitch: usize,
    tile: u16,
    sprite: bool,
) {
    let tile_id = (tile & 0x3FF) as usize;
    let horizontal_flip = (tile & 0x400) != 0;
    let vertical_flip = (tile & 0x800) != 0;
    let palbank = ((tile >> 8) & 0xF0) as u8;

    let base_tile = cbb_bytes + (tile_id * 32);

    // in 4-bit mode, single row is 4 bytes, for 8 rows total = 32 bytes

    let tile_width = 4;
    let tile_height = 8;

    for tile_y in 0..tile_height {
        let row_base = match vertical_flip {
            false => base_tile + (tile_y * tile_width),
            true => base_tile + (((tile_height - 1) - tile_y) * tile_width),
        };

        let row = &vram[row_base..row_base + tile_width];

        for tile_x in 0..tile_width {
            let y_offset = (y + tile_y) * pitch;
            let x_offset = match horizontal_flip {
                false => (x + (tile_x * 2)) * 4,
                true => (x + (((tile_width - 1) - tile_x) * 2)) * 4,
            };

            let offset = y_offset + x_offset;
            let (left_pal, right_pal) = match horizontal_flip {
                false => ((row[tile_x] & 0x0F) | palbank, (row[tile_x] >> 4) | palbank),
                true => ((row[tile_x] >> 4) | palbank, (row[tile_x] & 0x0F) | palbank),
            };

            let (left_color, right_color) = match sprite {
                false => (
                    get_palette_color(palette, 0, left_pal, true),
                    get_palette_color(palette, 0, right_pal, true),
                ),
                true => (
                    get_sprite_palette_color(palette, left_pal, true),
                    get_sprite_palette_color(palette, right_pal, true),
                ),
            };

            //if sprite == true && tile_y == 0 && tile_x == 0 && x == 0 && y == 0 {
            //    println!("sprite={sprite}, left_pal={left_pal}, right_pal={right_pal}, left_color={left_color:#?}, right_color={right_color:#?}");
            //}

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

/// Draw tile in 8 bits per pixel color mode
/// x & y are in pixels, not tiles
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

    for tile_y in 0..tile_height {
        // Get slice of row
        let row_base = match vertical_flip {
            false => base_tile + (tile_y * tile_width),
            true => base_tile + (((tile_height - 1) - tile_y) * tile_width),
        };
        let row = &vram[row_base..row_base + tile_width];

        for tile_x in 0..tile_width {
            let y_offset = (y + tile_y) * pitch;
            let x_offset = match horizontal_flip {
                false => (x + tile_x) * 4,
                true => (x + ((tile_width - 1) - tile_x)) * 4,
            };

            let offset = y_offset + x_offset;
            let color = get_palette_color(palette, 0, row[tile_x], false);

            buffer[offset] = color.0;
            buffer[offset + 1] = color.1;
            buffer[offset + 2] = color.2;
            buffer[offset + 3] = color.3;
        }
    }
}

fn get_tile_from_vram(
    vram: &Vec<u8>,
    x: u16,
    y: u16,
    dim_t: (u16, u16),
    screen_base_block: u32,
) -> u16 {
    let block_size = 32;

    let mut sbb_page = 0;

    let x_overflow = x >= block_size;
    let y_overflow = y >= block_size;

    let col = match x_overflow {
        false => x,
        true => x - block_size,
    };

    let row = match y_overflow {
        false => y * block_size,
        true => (y - block_size) * block_size,
    };

    if x_overflow {
        sbb_page += 1;
    }

    if y_overflow {
        sbb_page += 1;
        if dim_t.0 > 32 {
            sbb_page += 1;
        }
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

                    let x = (x as usize) * 8;
                    let y = (y as usize) * 8;

                    if color_4bit {
                        if tile_id < 512 {
                            draw_tile_4bpp(
                                vram, palette, buffer, x, y, cbb_bytes, pitch, tile, false,
                            );
                        }
                    } else {
                        if tile_id < 256 {
                            draw_tile_8bpp(vram, palette, buffer, x, y, cbb_bytes, pitch, tile);
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
                    let tile = get_tile_from_vram(
                        vram,
                        x,
                        y,
                        (width_tiles, height_tiles),
                        screen_base_block,
                    );

                    let x = (x as usize) * 8;
                    let y = (y as usize) * 8;

                    if color_4bit {
                        draw_tile_4bpp(vram, palette, buffer, x, y, cbb_bytes, pitch, tile, false);
                    } else {
                        draw_tile_8bpp(vram, palette, buffer, x, y, cbb_bytes, pitch, tile);
                    }
                }
            }
        })
        .expect("[SDL] Cannot fill texture");
}

fn draw_rect_argb(
    texture: &mut Texture,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    argb: u32,
) {
    let a = ((argb >> 24) & 0xFF) as u8;
    let r = ((argb >> 16) & 0xFF) as u8;
    let g = ((argb >> 8) & 0xFF) as u8;
    let b = ((argb >> 0) & 0xFF) as u8;

    texture
        .with_lock(None, |buffer: &mut [u8], pitch: usize| {
            for y in y..(y + height) {
                for x in x..(x + width) {
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
    match msg.dispcnt & 0x7 {
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

                    let (a, r, g, b) =
                        get_palette_color(palette, (msg.dispcnt & 0x7) as u8, pixel, false);

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
    match msg.dispcnt & 0x7 {
        0 => draw_mode0(msg, vram, palette, texture),
        1 => fill_texture_argb(texture, 240, 160, 0xFFFF0000),
        2 => fill_texture_argb(texture, 240, 160, 0xFF00FF00),
        3 => draw_mode3(texture, vram),
        4 => draw_mode4(texture, msg, vram, palette),
        5 => draw_mode5(texture, msg, vram),
        _ => panic!("Unknown LCD BG mode"),
    }
}

pub fn draw_and_render_sprites(
    canvas: &mut Canvas<Window>,
    texture: &mut Texture,
    msg: &RenderMessage,
    vram: &Vec<u8>,
    palette: &Vec<u8>,
    oam: &Vec<u8>,
) {
    let mapping_2d = msg.dispcnt & 0x40 == 0;

    // TODO: Priority
    for n in 0..128 {
        let obj = get_obj_attr(oam, n);

        let obj_mode = ((obj.attr0 >> 8) & 0x3) as u8;
        let gfx_mode = ((obj.attr0 >> 10) & 0x3) as u8;

        if obj_mode == 0b10 {
            continue;
        }

        if obj_mode == 0b01 || obj_mode == 0b11 {
            todo!("Todo, obj_mode={obj_mode:02b}");
        }

        if gfx_mode == 0b01 || obj_mode == 0b10 {
            todo!("Todo, gfx_mode={gfx_mode:02b}");
        }

        // Attr0
        let y = (obj.attr0 & 0xFF) as u32;
        let ys = (y as i32).wrapping_shl(24).wrapping_shr(24);
        let color_4bit = (obj.attr0 & 0x2000) == 0;
        let affine = (obj.attr0 & 0x100) != 0;

        // Attr1
        let x = (obj.attr1 & 0x1FF) as u32;
        let xs = (x as i32).wrapping_shl(23).wrapping_shr(23);
        let horizontal_flip = (obj.attr1 & 0x1000) != 0;
        let vertical_flip = (obj.attr1 & 0x2000) != 0;

        let (width, height) = obj.get_size();
        let (w_tiles, h_tiles) = (width / 8, height / 8);
        let prio = obj.get_priority();

        if !color_4bit {
            todo!("Implement 8bpp sprite");
        }

        texture
            .with_lock(None, |buffer: &mut [u8], pitch: usize| {
                let tile = obj.attr2 & 0xF3FF;

                for tile_y in 0..h_tiles {
                    for tile_x in 0..w_tiles {
                        let tile = match mapping_2d {
                            false => tile + ((tile_y * w_tiles) + tile_x) as u16,
                            true => tile + ((tile_y * 32) + tile_x) as u16,
                        };

                        draw_tile_4bpp(
                            vram,
                            palette,
                            buffer,
                            tile_x * 8,
                            tile_y * 8,
                            0x10000,
                            pitch,
                            tile,
                            true,
                        );
                    }
                }
            })
            .expect("[SDL] Cannot fill texture");

        // Check for wraparound on Y-axis, since max Y < screen height
        if y + height as u32 > 160 && y < 160 {
            let bot_height = (y + height as u32) - 160;
            let top_height = height as u32 - bot_height;

            let (dest_top, dest_bot) = match vertical_flip {
                false => (
                    Rect::new(x as i32, y as i32, width as u32, top_height),
                    Rect::new(xs, ys + top_height as i32, width as u32, bot_height),
                ),
                true => (
                    Rect::new(xs, ys + top_height as i32, width as u32, bot_height),
                    Rect::new(x as i32, y as i32, width as u32, top_height),
                ),
            };

            // Top part of sprite
            canvas
                .copy_ex(
                    &texture,
                    Rect::new(0, 0, width as u32, top_height),
                    dest_top,
                    0.0,
                    Point::new(0, 0),
                    horizontal_flip,
                    vertical_flip,
                )
                .expect("[SDL] Cannot copy sprite");

            // Bottom part of sprite
            canvas
                .copy_ex(
                    &texture,
                    Rect::new(0, top_height as i32, width as u32, bot_height),
                    dest_bot,
                    0.0,
                    Point::new(0, 0),
                    horizontal_flip,
                    vertical_flip,
                )
                .expect("[SDL] Cannot copy sprite");
        } else {
            if !affine {
                canvas
                    .copy_ex(
                        &texture,
                        Rect::new(0, 0, width as u32, height as u32),
                        Rect::new(xs, ys, width as u32, height as u32),
                        0.0,
                        Point::new(0, 0),
                        horizontal_flip,
                        vertical_flip,
                    )
                    .expect("[SDL] Cannot copy sprite");
            } else {
                println!("Implement affine sprites");
                canvas
                    .copy_ex(
                        &texture,
                        Rect::new(0, 0, width as u32, height as u32),
                        Rect::new(xs, ys, width as u32, height as u32),
                        0.0,
                        Point::new(0, 0),
                        false,
                        false,
                    )
                    .expect("[SDL] Cannot copy sprite");
            }
        }
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
