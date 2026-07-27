use x86::io;


pub trait Cursor {
    fn new() -> Self;
    fn get(&self) -> usize;
    fn set(&mut self, position: usize);
}


pub struct VgaCursor {}


impl VgaCursor {
    const ADDRESS_REGISTER: u16 = 0x03D4;
    const DATA_REGISTER: u16 = 0x03D5;

    const CURSOR_LOCATION_HIGH: u8 = 0x0E;
    const CURSOR_LOCATION_LOW: u8 = 0x0F;


    fn set_register(&mut self, register: u8, value: u8) {
        unsafe {
            io::outb(Self::ADDRESS_REGISTER, register);
            io::outb(Self::DATA_REGISTER, value);
        }
    }


    fn register(&self, register: u8) -> u8 {
        unsafe {
            io::outb(Self::ADDRESS_REGISTER, register);

            io::inb(Self::DATA_REGISTER)
        }
    }


    fn set_height(&mut self, height: u8) {
        const CURSOR_BEGIN_LINE: u8 = 0x0A;
        const CURSOR_END_LINE: u8 = 0x0B;

        const MAX_LINES: u8 = 1 << 4;
        const LAST_LINE: u8 = MAX_LINES - 1;

        self.set_register(CURSOR_BEGIN_LINE, MAX_LINES - height);
        self.set_register(CURSOR_END_LINE, LAST_LINE);
    }
}


impl Cursor for VgaCursor {
    fn new() -> Self {
        let mut cursor = Self {};
        cursor.set_height(2);

        cursor
    }


    fn get(&self) -> usize {
        let mut position = self.register(Self::CURSOR_LOCATION_HIGH) as usize;
        position <<= 8;
        position |= self.register(Self::CURSOR_LOCATION_LOW) as usize;

        position
    }


    fn set(&mut self, position: usize) {
        self.set_register(Self::CURSOR_LOCATION_HIGH, (position >> 8) as u8);
        self.set_register(Self::CURSOR_LOCATION_LOW, position as u8);
    }
}
