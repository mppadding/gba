pub struct Keypad {
    pub keyinput: u16,
    pub keycnt: u16,
}

pub const BUTTON_A: u16 = 1 << 0;
pub const BUTTON_B: u16 = 1 << 1;
pub const BUTTON_SELECT: u16 = 1 << 2;
pub const BUTTON_START: u16 = 1 << 3;
pub const BUTTON_RIGHT: u16 = 1 << 4;
pub const BUTTON_LEFT: u16 = 1 << 5;
pub const BUTTON_UP: u16 = 1 << 6;
pub const BUTTON_DOWN: u16 = 1 << 7;
pub const BUTTON_R: u16 = 1 << 8;
pub const BUTTON_L: u16 = 1 << 9;

impl Keypad {
    pub fn new() -> Self {
        Self {
            keyinput: 0xFFFF,
            keycnt: 0,
        }
    }

    pub fn press(&mut self, buttons: u16) {
        self.keyinput &= !(buttons);
    }

    pub fn release(&mut self, buttons: u16) {
        self.keyinput |= buttons;
    }

    pub fn is_irq_enabled(&self) -> bool {
        (self.keycnt & 0x4000) != 0
    }
}
