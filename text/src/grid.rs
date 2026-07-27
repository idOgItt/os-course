use core::{
    cmp::min,
    ops::{Deref, DerefMut},
};

use static_assertions::const_assert;
use volatile::Volatile;

use super::Attribute;


pub fn is_graphical(octet: u8) -> bool {
    b' ' < octet && octet < 0x7F
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Cell {
    pub character: u8,
    attribute: Attribute,
}


impl Deref for Cell {
    type Target = Self;

    fn deref(&self) -> &Self {
        self
    }
}


impl DerefMut for Cell {
    fn deref_mut(&mut self) -> &mut Self {
        self
    }
}


pub const COLUMNS: usize = 80;
const ROWS: usize = 25;
pub const SIZE: usize = COLUMNS * ROWS;

pub const TAB_WIDTH: usize = 8;


const_assert!(COLUMNS > 0);
const_assert!(COLUMNS % TAB_WIDTH == 0);
const_assert!(SIZE % COLUMNS == 0);
const_assert!(SIZE >= 3 * COLUMNS);


pub type Buffer = [Volatile<Cell>; SIZE];


pub struct Grid<'buffer> {
    buffer: &'buffer mut Buffer,

    row_start: usize,
    column: usize,

    attribute: Attribute,
}


impl<'buffer> Grid<'buffer> {
    pub fn new(buffer: &'buffer mut Buffer, attribute: Attribute) -> Self {
        Self {
            buffer,
            row_start: 0,
            column: 0,
            attribute,
        }
    }


    pub fn init<S: serial::Serial>(&mut self, cursor_position: usize, serial: &mut S) -> usize {
        let first_guess_position = min(cursor_position, SIZE);
        let mut position = first_guess_position;

        for i in first_guess_position..SIZE {
            if is_graphical(self.buffer[i].read().character) {
                position = i + 1;
            }
        }

        self.row_start = 0;
        self.column = 0;

        let mut prev_graphical = 0;

        for i in 0..position {
            if is_graphical(self.buffer[i].read().character) {
                while prev_graphical <= i {
                    serial.print_octet(self.buffer[prev_graphical].read().character);
                    prev_graphical += 1;
                }
            }

            self.column += 1;

            if self.column >= COLUMNS {
                self.row_start += COLUMNS;
                self.column = 0;
                prev_graphical = i + 1;
                serial.print_octet(b'\n');
            }
        }

        if self.row_start >= SIZE {
            self.scroll();
        } else {
            self.clear(position, SIZE);
        }

        position
    }


    pub fn position(&self) -> usize {
        self.row_start + self.column
    }


    pub fn is_newline(&self) -> bool {
        self.column == 0
    }


    pub fn print_character(&mut self, ch: char) {
        let implicit_newline = self.column == COLUMNS;

        match ch {
            '\n' => {
                if !implicit_newline {
                    self.newline();
                    self.adjust_position();
                }
            },
            '\r' => self.column = 0,
            '\t' => {
                self.adjust_position();
                self.tab();
            },
            _ => {
                self.adjust_position();
                self.character(ch);
            },
        }

        if self.position() >= SIZE {
            self.scroll();
        }
    }


    pub fn scroll(&mut self) {
        assert!(self.row_start >= COLUMNS);
        self.row_start -= COLUMNS;

        for i in 0..SIZE - COLUMNS {
            let cell = self.buffer[i + COLUMNS].read();
            self.buffer[i].write(cell);
        }

        self.clear(SIZE - COLUMNS, SIZE);
    }


    fn character(&mut self, ch: char) {
        fn char_to_u8(ch: char) -> u8 {
            if ch < ' ' || ch >= 0x7F as char {
                b'?'
            } else {
                ch as u8
            }
        }

        let position = self.row_start + self.column;
        let cell = Cell {
            character: char_to_u8(ch),
            attribute: self.attribute,
        };

        self.buffer[position].write(cell);
        self.column += 1;
    }


    fn newline(&mut self) {
        while self.column < COLUMNS {
            self.character(' ');
        }
    }


    fn tab(&mut self) {
        let tab_column = (self.column + 1).next_multiple_of(TAB_WIDTH);
        if self.row_start + tab_column > SIZE {
            self.scroll();
        }
        while self.column < tab_column {
            self.character(' ');
        }
    }


    fn adjust_position(&mut self) {
        if self.column >= COLUMNS {
            self.row_start += COLUMNS;
            self.column -= COLUMNS;

            if self.row_start >= SIZE {
                self.scroll();
            }
        }
    }


    pub fn attribute(&mut self) -> Attribute {
        self.attribute
    }


    pub fn set_attribute(&mut self, attribute: Attribute) {
        self.attribute = attribute;
    }


    pub fn clear(&mut self, begin: usize, end: usize) {
        let cell = Cell {
            character: b' ',
            attribute: self.attribute,
        };

        for i in begin..end {
            self.buffer[i].write(cell);
        }
    }
}
