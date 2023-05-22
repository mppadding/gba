pub const TIME_NS_SCANLINE: u64 = 73430;

pub const DISPSTATE_VBLANK: u16 = 1 << 0;
pub const DISPSTATE_HBLANK: u16 = 1 << 1;
pub const DISPSTATE_VCOUNTER: u16 = 1 << 2;

pub struct LCD {
    pub registers: [u8; 88],
}

impl LCD {
    pub fn new() -> Self {
        Self { registers: [0; 88] }
    }

    pub fn reset(&mut self) {
        self.registers = [0; 88];
    }

    fn get_u16(&self, addr: u32) -> u16 {
        let addr = addr as usize;
        ((self.registers[addr + 1] as u16) << 8) | (self.registers[addr] as u16)
    }

    fn set_u16(&mut self, addr: u32, val: u16) {
        let addr = addr as usize;
        self.registers[addr + 1] = ((val >> 8) & 0xFF) as u8;
        self.registers[addr] = (val & 0xFF) as u8;
    }

    pub fn get_dispcnt(&self) -> u16 {
        self.get_u16(0)
    }

    pub fn set_dispcnt(&mut self, val: u16) {
        self.set_u16(0, val);
    }

    pub fn get_dispstat(&self) -> u16 {
        self.get_u16(4)
    }

    pub fn set_dispstat(&mut self, val: u16) {
        self.set_u16(4, val)
    }

    pub fn set_dispstat_vblank(&mut self, set: bool) {
        self.set_dispstat(match set {
            false => self.get_dispstat() & !(1 << 0),
            true => self.get_dispstat() | (1 << 0),
        });
    }

    pub fn set_dispstat_hblank(&mut self, set: bool) {
        self.set_dispstat(match set {
            false => self.get_dispstat() & !(1 << 1),
            true => self.get_dispstat() | (1 << 1),
        });
    }

    pub fn set_dispstat_vcount(&mut self, set: bool) {
        self.set_dispstat(match set {
            false => self.get_dispstat() & !(1 << 2),
            true => self.get_dispstat() | (1 << 2),
        });
    }

    pub fn is_vblank_irq_enabled(&self) -> bool {
        (self.get_dispstat() & 0x8) != 0
    }

    pub fn is_hblank_irq_enabled(&self) -> bool {
        (self.get_dispstat() & 0x10) != 0
    }
    pub fn is_vcount_irq_enabled(&self) -> bool {
        (self.get_dispstat() & 0x20) != 0
    }

    pub fn get_dispcnt_mode(&self) -> u8 {
        (self.get_dispcnt() & 0x7) as u8
    }

    pub fn get_dispcnt_frame(&self) -> bool {
        (self.get_dispcnt() & 0x10) != 0
    }

    pub fn get_vcount(&self) -> u16 {
        self.get_u16(6)
    }

    pub fn set_vcount(&mut self, val: u16) {
        self.set_u16(6, val)
    }

    /// Increments vcount to next scanline
    /// Updates V-Blank, H-Blank and V-Counter flag accordingly
    /// Performs wrap around to 0 if limit reached
    pub fn increment_vcount(&mut self) -> u16 {
        let val = self.get_vcount();
        let inc = match val == 227 {
            false => val + 1,
            true => 0,
        };

        self.set_dispstat_vblank(inc >= 160 && inc <= 226);

        let lyc = (self.get_dispstat() >> 8) & 0xFF;
        self.set_dispstat_vcount(lyc == inc);

        self.set_vcount(inc);
        inc
    }
}
