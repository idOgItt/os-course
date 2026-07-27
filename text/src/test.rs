use core::{
    cmp::min,
    fmt::{Debug, Display, Formatter, Result, Write},
    mem,
};

use serial::Serial;

use super::{
    cursor::Cursor,
    grid::{Buffer, Grid, COLUMNS, SIZE, TAB_WIDTH},
    Attribute,
    Text,
};


fn mock_buffer() -> Buffer {
    unsafe { mem::zeroed() }
}


fn mock_grid(buffer: &mut Buffer) -> Grid {
    Grid::new(buffer, Attribute::NORMAL)
}


#[test]
fn out_of_bounds() {
    let mut buffer = mock_buffer();
    let size = buffer.len();

    let mut grid = mock_grid(&mut buffer);

    let chars = ['\t', '\n', '\r', '*'];

    for ch in chars.iter() {
        for _ in 0..2 * size {
            grid.print_character(*ch);
        }
    }
}


#[test]
fn out_of_bounds_on_init() {
    let mut buffer = mock_buffer();
    let size = buffer.len();

    let mut grid = mock_grid(&mut buffer);
    let mut serial = MockSerial::new();

    grid.init(0, &mut serial);

    for _ in 0..size - 1 {
        grid.print_character('*');
    }

    grid.init(size - 1, &mut serial);

    grid.print_character('*');
}


#[test]
fn out_of_bounds_on_tab() {
    let mut buffer = mock_buffer();
    let size = buffer.len();

    let mut grid = mock_grid(&mut buffer);

    while grid.position() < size - COLUMNS {
        grid.print_character('\n');
    }

    for tabs_position in COLUMNS - 2 * TAB_WIDTH..COLUMNS + 2 * TAB_WIDTH {
        grid.print_character('\n');

        assert_position(
            &grid,
            size - COLUMNS,
            "That is, at the last line of the Grid.\n",
        );

        for _ in 0..tabs_position {
            grid.print_character('*');
        }
        for _ in tabs_position..COLUMNS + 2 * TAB_WIDTH {
            grid.print_character('\t');
        }
    }
}


#[test]
fn scroll() {
    let mut before_scroll = mock_buffer();
    let size = before_scroll.len();
    let position;

    {
        let mut grid = mock_grid(&mut before_scroll);
        grid.init(0, &mut MockSerial::new());

        for ch in Filler::new(SIZE - 1) {
            grid.print_character(ch);
        }

        position = grid.position();
        assert!(
            position > SIZE - COLUMNS,
            concat!(
                "\n\nExpected that all the lines of the Grid are filled with some text.\n",
                "But the current position in the Grid is the {:?}.\n",
                "Whilst the last line begins from the {:?}.\n\n",
            ),
            format::position(position),
            format::position(SIZE - COLUMNS),
        );
    }

    let mut after_scroll = mock_buffer();
    for i in 0..size {
        after_scroll[i].write(before_scroll[i].read());
    }

    {
        let mut grid = mock_grid(&mut after_scroll);
        grid.init(position, &mut MockSerial::new());
        grid.scroll();
    }

    for new_position in 0..size - COLUMNS {
        let old_position = new_position + COLUMNS;
        let expected = before_scroll[old_position].read();
        let found = after_scroll[new_position].read();
        assert!(
            found == expected,
            concat!(
                "\n\nExpected that the {:?}\n",
                "will move one row up from the {:?}\n",
                "to the {:?} after the Grid::scroll().\n",
                "But found the {:?} there.\n",
                "The buffer before the scroll:\n{:?}\n",
                "The buffer after the scroll:\n{:?}\n\n",
            ),
            expected,
            format::position(old_position),
            format::position(new_position),
            found,
            format::buffer(&before_scroll),
            format::buffer(&after_scroll),
        );
    }

    for position in size - COLUMNS..size {
        let found = after_scroll[position].read().character;
        assert!(
            found == b' ',
            concat!(
                "\n\nExpected that the last row will be filled\n",
                "with the white space after the Grid::scroll().\n",
                "But found the {:?}\n",
                "at the {:?}.\n",
                "The buffer is:\n{:?}\n\n",
            ),
            format::character(found),
            format::position(position),
            format::buffer(&after_scroll),
        );
    }
}


#[test]
fn explicit_newline() {
    let mut buffer = mock_buffer();
    let mut grid = mock_grid(&mut buffer);

    grid.print_character('\n');

    assert_position(&grid, COLUMNS, "After printing a single '\\n' character.\n");

    grid.print_character('\n');

    assert_position(&grid, 2 * COLUMNS, "After printing two '\\n' characters.\n");
}


#[test]
fn explicit_newline_after_an_implicit_one() {
    let mut buffer = mock_buffer();
    let mut grid = mock_grid(&mut buffer);

    fill_line(&mut grid, '*');

    assert_position(
        &grid,
        COLUMNS,
        "After printing a full line - #COLUMNS of non-control characters.\n",
    );

    grid.print_character('\n');

    assert_position(
        &grid,
        COLUMNS,
        "After printing a full line - #COLUMNS of non-control characters - followed by a '\\n'.\n",
    );
}


#[test]
fn position_is_always_less_than_size() {
    let mut buffer = mock_buffer();
    let size = buffer.len();

    let mut grid = mock_grid(&mut buffer);

    for _ in 0..size - 1 {
        grid.print_character('*');
    }

    assert_position(
        &grid,
        size - 1,
        "After printing #(SIZE - 1) of non-control characters.\n",
    );

    grid.print_character('*');

    assert_position(
        &grid,
        size - COLUMNS,
        concat!(
            "After filling the full screen with #SIZE of non-control characters.\n",
            "That is, expected that the Grid::scroll() will be triggered once.\n",
        ),
    );
}


#[test]
fn carriage_return() {
    let mut buffer = mock_buffer();
    let mut grid = mock_grid(&mut buffer);

    fill_line(&mut grid, '*');

    assert_position(
        &grid,
        COLUMNS,
        "After printing a full line - of #COLUMNS non-control characters.\n",
    );

    grid.print_character('\r');

    assert_position(
        &grid,
        0,
        "After printing a full line - of #COLUMNS non-control characters - followed by a '\\r'.\n",
    );

    fill_line(&mut grid, '#');

    assert!(
        buffer[0].read().character == b'#',
        concat!(
            "\n\nExpected that the first character in the buffer will be the '#'.\n",
            "After printing a full line of '*', then a '\\r', and then a full line of the '#'.\n",
            "But found it to be the {:?}.\n\n",
        ),
        format::character(buffer[0].read().character),
    );
}


fn fill_line(grid: &mut Grid, ch: char) {
    for _ in 0..COLUMNS {
        grid.print_character(ch);
    }
}


fn assert_position(grid: &Grid, position: usize, message: &str) {
    assert!(
        grid.position() == position,
        concat!(
            "\n\nExpected that the position will be the {:?}.\n",
            "{}",
            "But found it to be the {:?}.\n\n",
        ),
        format::position(position),
        message,
        format::position(grid.position()),
    );
}


#[test]
fn printing() {
    let test_data = [
        Filler::new(2 * COLUMNS).start('0'),
        Filler::new(2 * COLUMNS - 1).start('a'),
        Filler::new(2 * COLUMNS + 1).start('A'),
        Filler::new(2 * COLUMNS - 2).start(' '),
        Filler::new(2 * COLUMNS + 2).start(':'),
        Filler::new(4 * COLUMNS).start('=').enable_tabs(),
    ];

    let mut buffer = mock_buffer();
    let mut cursor_position = 0;
    let mut restore_cursor_position = false;

    for (part_index, part) in test_data.iter().enumerate() {
        {
            let mut text = Text::<MockCursor, MockSerial>::new(&mut buffer, Attribute::NORMAL);

            if restore_cursor_position {
                text.cursor.set(cursor_position);
                restore_cursor_position = !restore_cursor_position;
            }

            text.init();

            write!(text, "{part}").unwrap();

            cursor_position = text.cursor.get();
        }

        assert!(
            cursor_position + COLUMNS < buffer.len(),
            concat!(
                "\n\nDecrease the total size of the test data\n",
                "or the number of lines in it.\n",
                "This test requires that the Grid::scroll() is not triggered.\n",
                "The cursor is now at the {:?}.\n",
                "It is the last row of the buffer which size is {}.\n",
                "That means the Grid::scroll() could have been triggered already.\n\n",
            ),
            format::position(cursor_position),
            buffer.len(),
        );

        check(&buffer, cursor_position, &test_data[..part_index + 1]);
    }
}


fn check(buffer: &Buffer, cursor_position: usize, data: &[Filler]) {
    let mut position = 0;

    for part in data {
        if position % COLUMNS != 0 {
            position = check_white_space(&buffer, position, COLUMNS, data);
        }
        if position != 0 {
            position = check_white_space(&buffer, position, COLUMNS, data);
        }

        let mut implicit_newline = false;

        for ch in *part {
            assert!(
                position < buffer.len(),
                concat!(
                    "\n\nThe data exceeded the buffer of size {}\n",
                    "at the {:?}.\n",
                    "The data is: {:?}.\n",
                    "The buffer is:\n{:?}\n\n",
                ),
                buffer.len(),
                format::position(position),
                data,
                format::buffer(&buffer),
            );

            match ch {
                '\t' => {
                    position = check_white_space(&buffer, position, TAB_WIDTH, data);
                    implicit_newline = position % COLUMNS == 0;
                },
                '\n' => {
                    if implicit_newline {
                        implicit_newline = false;
                    } else {
                        position = check_white_space(&buffer, position, COLUMNS, data);
                    }
                },
                _ => {
                    let expected = ch as u8;
                    let found = buffer[position].read().character;
                    assert!(
                        found == expected,
                        concat!(
                            "\n\nExpected the {:?}\n",
                            "but found the {:?}\n",
                            "at the {:?}.\n",
                            "The data is: {:?}.\n",
                            "The buffer is:\n{:?}\n\n",
                        ),
                        format::character(expected),
                        format::character(found),
                        format::position(position),
                        data,
                        format::buffer(&buffer),
                    );
                    position += 1;

                    implicit_newline = position % COLUMNS == 0;
                },
            }
        }
    }

    assert!(
        cursor_position == position,
        concat!(
            "\n\nExpected the cursor to be at the {:?}\n",
            "but found it at the {:?}.\n\n",
        ),
        format::position(position),
        format::position(cursor_position),
    );
}


fn check_white_space(buffer: &Buffer, begin: usize, alignment: usize, data: &[Filler]) -> usize {
    let end = min((begin + 1).next_multiple_of(alignment), buffer.len());

    for position in begin..end {
        let found = buffer[position].read().character;
        assert!(
            found == b' ',
            concat!(
                "\n\nExpected the white space from the {:?}\n",
                "until the {}-cells aligned {:?}.\n",
                "But found the {:?}\n",
                "at the {:?}.\n",
                "The data is: {:?}.\n",
                "The buffer is:\n{:?}\n\n",
            ),
            format::position(begin),
            alignment,
            format::position(end),
            format::character(found),
            format::position(position),
            data,
            format::buffer(buffer),
        );
    }

    end
}


struct MockCursor {
    position: usize,
}


impl Cursor for MockCursor {
    fn new() -> Self {
        Self { position: 0 }
    }


    fn get(&self) -> usize {
        self.position
    }


    fn set(&mut self, position: usize) {
        self.position = position;
    }
}


struct MockSerial {}


impl Serial for MockSerial {
    fn new() -> Self {
        Self {}
    }

    fn print_octet(&mut self, _: u8) {
    }
}


#[derive(Clone, Copy)]
struct Filler {
    current_octet: u8,
    remaining_size: usize,
    until_tab: usize,
    next_until_tab: usize,
}


impl Filler {
    const PRIME: u8 = 89;
    const BEGIN: u8 = b' ';
    const END: u8 = Self::BEGIN + Self::PRIME;


    fn new(size: usize) -> Self {
        Self {
            current_octet: Self::BEGIN,
            remaining_size: size,
            until_tab: size,
            next_until_tab: 0,
        }
    }


    fn start(&mut self, start: char) -> Self {
        self.current_octet = start as u8;

        *self
    }


    fn enable_tabs(&mut self) -> Self {
        self.schedule_next_tab();

        *self
    }


    fn schedule_next_tab(&mut self) {
        const MAX_UNTIL_TAB: usize = 17;

        self.until_tab = self.next_until_tab % MAX_UNTIL_TAB;
        self.next_until_tab = self.until_tab + 1;
    }
}


impl Iterator for Filler {
    type Item = char;


    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining_size == 0 {
            return None;
        }

        self.remaining_size -= 1;

        if self.until_tab == 0 {
            self.schedule_next_tab();

            Some('\t')
        } else {
            self.until_tab -= 1;

            self.current_octet += 1;
            if self.current_octet >= Self::END {
                self.current_octet = Self::BEGIN;
            }

            Some(self.current_octet as char)
        }
    }
}


impl Debug for Filler {
    fn fmt(&self, formatter: &mut Formatter) -> Result {
        write!(formatter, "\"")?;

        for ch in *self {
            match ch {
                '\t' => write!(formatter, "\\t")?,
                '\r' => write!(formatter, "\\r")?,
                '\n' => write!(formatter, "\\n")?,
                '\\' | '"' => write!(formatter, "\\{ch}")?,
                _ => write!(formatter, "{ch}")?,
            }
        }

        write!(formatter, "\"")
    }
}


impl Display for Filler {
    fn fmt(&self, formatter: &mut Formatter) -> Result {
        for ch in *self {
            write!(formatter, "{ch}")?;
        }

        Ok(())
    }
}


mod format {
    use core::{
        cmp::min,
        fmt::{Debug, Formatter, Result},
    };

    use super::super::grid::{is_graphical, Buffer, COLUMNS};


    pub struct BufferFormatter<'buffer> {
        buffer: &'buffer Buffer,
    }


    pub fn buffer(buffer: &Buffer) -> BufferFormatter {
        BufferFormatter { buffer }
    }


    impl<'buffer> Debug for BufferFormatter<'buffer> {
        fn fmt(&self, formatter: &mut Formatter) -> Result {
            format_header(formatter, 2, COLUMNS, 0, 0)?;
            write!(formatter, "\n")?;

            let mut end = self.buffer.len();
            while end >= 1 && self.buffer[end - 1].read().character == b'\0' {
                end -= 1;
            }
            end = min(end.next_multiple_of(COLUMNS), self.buffer.len());

            for (index, cell) in self.buffer[0..end].iter().enumerate() {
                let mut ch = cell.read().character;
                if !is_graphical(ch) {
                    ch = b' ';
                }

                let position = position(index);
                if position.column == 0 {
                    if position.row > 0 {
                        write!(formatter, "│\n")?;
                    }
                    write!(formatter, "{:2}│", position.row)?;
                }

                write!(formatter, "{}", ch as char)?;
            }

            write!(formatter, "│\n")?;

            format_footer(formatter, 2, COLUMNS, 10, 5)
        }
    }


    fn format_labels(
        formatter: &mut Formatter,
        indent: usize,
        width: usize,
        label_distance: usize,
    ) -> Result {
        assert!(
            label_distance >= 3,
            "\n\nlabel_distance must be at least 3, otherwise the labels will not fit.\n\n",
        );

        for _ in 0..indent + 1 {
            write!(formatter, " ")?;
        }

        for i in 0..width {
            if i % label_distance == 0 {
                write!(formatter, "{:<2}", i)?;
            } else if i % label_distance != 1 {
                write!(formatter, " ")?;
            }
        }

        Ok(())
    }


    struct LineChars {
        line: char,
        mark: char,
        left_corner: char,
        right_corner: char,
    }


    fn format_line(
        formatter: &mut Formatter,
        indent: usize,
        width: usize,
        mark_distance: usize,
        chars: &LineChars,
    ) -> Result {
        for _ in 0..indent {
            write!(formatter, " ")?;
        }

        write!(formatter, "{}", chars.left_corner)?;

        for i in 0..width {
            if mark_distance > 0 && i % mark_distance == 0 {
                write!(formatter, "{}", chars.mark)?;
            } else {
                write!(formatter, "{}", chars.line)?;
            }
        }

        write!(formatter, "{}", chars.right_corner)
    }


    fn format_header(
        formatter: &mut Formatter,
        indent: usize,
        width: usize,
        label_distance: usize,
        mark_distance: usize,
    ) -> Result {
        if label_distance > 0 {
            format_labels(formatter, indent, width, label_distance)?;
            write!(formatter, "\n")?;
        }

        const HEADER_CHARS: LineChars = LineChars {
            line: '─',
            mark: '┴',
            left_corner: '┌',
            right_corner: '┐',
        };

        format_line(formatter, indent, width, mark_distance, &HEADER_CHARS)
    }


    fn format_footer(
        formatter: &mut Formatter,
        indent: usize,
        width: usize,
        label_distance: usize,
        mark_distance: usize,
    ) -> Result {
        const FOOTER_CHARS: LineChars = LineChars {
            line: '─',
            mark: '┬',
            left_corner: '└',
            right_corner: '┘',
        };

        format_line(formatter, indent, width, mark_distance, &FOOTER_CHARS)?;

        if label_distance > 0 {
            write!(formatter, "\n")?;
            format_labels(formatter, indent, width, label_distance)
        } else {
            Ok(())
        }
    }


    #[derive(Debug)]
    pub struct Character {
        #[allow(unused)]
        value: char,
        #[allow(unused)]
        code: u8,
    }


    pub fn character(code: u8) -> Character {
        Character {
            value: code as char,
            code,
        }
    }


    #[derive(Debug)]
    pub struct Position {
        row: usize,
        column: usize,
        #[allow(unused)]
        index: usize,
    }


    pub fn position(index: usize) -> Position {
        Position {
            row: index / COLUMNS,
            column: index % COLUMNS,
            index,
        }
    }
}
