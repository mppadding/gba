#[derive(Default)]
pub struct Sound {
    pub io_soundcnt: [u32; 3],   // L, H, X
    pub io_soundcnt_1: [u32; 3], // L, H, X
    pub io_soundcnt_2: [u32; 2], // L, H
    pub io_soundcnt_3: [u32; 3], // L, H, X
    pub io_soundcnt_4: [u32; 2], // L, H

    pub io_bias: u16,

    pub io_fifo_a: u32,
    pub io_fifo_b: u32,
}
