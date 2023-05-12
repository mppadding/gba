#[derive(Debug)]
pub struct Memory {}

impl Memory {
    pub fn new() -> Self {
        Self {}
    }

    pub fn read_u8(&self, addr: usize) -> u8 {
        todo!("read_u8");
    }

    pub fn read_u16(&self, addr: usize) -> u16 {
        todo!("read_u16");
    }

    pub fn read_u32(&self, addr: usize) -> u32 {
        match addr {
            //0x06000000..=0x06017FFF => println!("Read from video memory (addr={:08X})", addr),
            _ => panic!("Addr out of range (addr={:08X})", addr),
        }
    }

    pub fn write_u8(&self, addr: usize, value: u8) {
        todo!("write_u8");
    }

    pub fn write_u16(&self, addr: usize, value: u16) {
        todo!("write_u16");
    }

    pub fn write_u32(&self, addr: usize, value: u32) {
        match addr {
            0x06000000..=0x06017FFF => println!(
                "Write to video memory (addr={:08X}, val={:08X})",
                addr, value
            ),
            _ => panic!("Addr out of range (addr={:08X})", addr),
        }
    }
}
