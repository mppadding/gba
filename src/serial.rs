pub struct Serial {
    pub registers_1: [u8; 12],

    pub rcnt: u16,
    pub joy_cnt: u16,
    pub joy_recv: u32,
    pub joy_trans: u32,
    pub joy_stat: u16,
}

impl Serial {
    pub fn new() -> Self {
        Self {
            registers_1: [0; 12],
            rcnt: 0,
            joy_cnt: 0,
            joy_recv: 0,
            joy_trans: 0,
            joy_stat: 0,
        }
    }

    pub fn read_u16(&self, addr: u32) -> u16 {
        let ptr = (addr - 0x120) as usize;

        match addr {
            0x120..=0x12A => {
                ((self.registers_1[ptr + 1] as u16) << 8) | (self.registers_1[ptr] as u16)
            }
            0x134 => self.rcnt,
            0x140 => self.joy_cnt,
            0x150 => (self.joy_recv & 0xFFFF) as u16,
            0x152 => (self.joy_recv >> 16) as u16,
            0x154 => (self.joy_trans & 0xFFFF) as u16,
            0x156 => (self.joy_trans >> 16) as u16,
            0x158 => self.joy_stat,
            _ => panic!("Addr out of range for Serial `{:04X}`", addr),
        }
    }

    pub fn read_u32(&self, addr: u32) -> u32 {
        let ptr = (addr - 0x120) as usize;

        match addr {
            0x120..=0x128 => {
                ((self.registers_1[ptr + 3] as u32) << 24)
                    | ((self.registers_1[ptr + 2] as u32) << 16)
                    | ((self.registers_1[ptr + 1] as u32) << 8)
                    | (self.registers_1[ptr] as u32)
            }
            0x12A => ((self.registers_1[ptr + 1] as u32) << 8) | (self.registers_1[ptr] as u32),
            0x134 => self.rcnt as u32,
            0x140 => self.joy_cnt as u32,
            0x150 => self.joy_recv,
            0x154 => self.joy_trans,
            0x158 => self.joy_stat as u32,
            _ => panic!("Addr out of range for Serial `{:04X}`", addr),
        }
    }

    pub fn write_u8(&mut self, addr: usize, val: u8) {
        let ptr = (addr - 0x120) as usize;

        let val_high = (val as u16) << 8;

        match addr {
            0x120..=0x12A => self.registers_1[ptr] = val,
            0x134 => self.rcnt = (self.rcnt & 0xFF00) | val as u16,
            0x135 => self.rcnt = (self.rcnt & 0x00FF) | val_high,
            0x140 => self.joy_cnt = (self.joy_cnt & 0xFF00) | val as u16,
            0x141 => self.joy_cnt = (self.joy_cnt & 0x00FF) | val_high,
            0x150..=0x153 => {
                let shift = 8 * (addr - 0x150);
                self.joy_recv = (self.joy_recv & (0xFF << shift)) | ((val as u32) << shift);
            }
            0x154..=0x157 => {
                let shift = 8 * (addr - 0x154);
                self.joy_trans = (self.joy_trans & (0xFF << shift)) | ((val as u32) << shift);
            }
            0x158 => self.joy_stat = (self.joy_stat & 0xFF00) | val as u16,
            0x159 => self.joy_stat = (self.joy_stat & 0x00FF) | val_high,
            _ => panic!("Addr out of range for Serial `{:04X}`", addr),
        }
    }

    pub fn write_u32(&mut self, addr: u32, val: u32) {
        let ptr = (addr - 0x120) as usize;

        let byte0 = (val & 0xFF) as u8;
        let byte1 = ((val >> 8) & 0xFF) as u8;
        let byte2 = ((val >> 16) & 0xFF) as u8;
        let byte3 = ((val >> 24) & 0xFF) as u8;

        let u16 = (val & 0xFFFF) as u16;

        match addr {
            0x120..=0x128 => {
                self.registers_1[ptr + 3] = byte3;
                self.registers_1[ptr + 2] = byte2;
                self.registers_1[ptr + 1] = byte1;
                self.registers_1[ptr + 0] = byte0;
            }
            0x12A => {
                self.registers_1[ptr + 1] = byte1;
                self.registers_1[ptr + 0] = byte0;
            }
            0x134 => self.rcnt = u16,
            0x140 => self.joy_cnt = u16,
            0x150 => self.joy_recv = val,
            0x154 => self.joy_trans = val,
            0x158 => self.joy_stat = u16,
            _ => panic!("Addr out of range for Serial `{:04X}`", addr),
        }
    }
}
