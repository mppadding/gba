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
            0x120..=0x12A => {
                ((self.registers_1[ptr + 3] as u32) << 24)
                    | ((self.registers_1[ptr + 2] as u32) << 16)
                    | ((self.registers_1[ptr + 1] as u32) << 8)
                    | (self.registers_1[ptr] as u32)
            }
            0x134 => self.rcnt as u32,
            0x140 => self.joy_cnt as u32,
            0x150 => self.joy_recv,
            0x154 => self.joy_trans,
            0x158 => self.joy_stat as u32,
            _ => panic!("Addr out of range for Serial `{:04X}`", addr),
        }
    }
}
