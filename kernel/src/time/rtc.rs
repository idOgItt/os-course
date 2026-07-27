use core::{hint, result, sync::atomic::{AtomicI64, AtomicU8, Ordering}};

use bitflags::bitflags;
use chrono::{format, DateTime, Duration, NaiveDate, NaiveDateTime, Utc};
use derive_more::Display;
use scopeguard::defer;
use x86::io;
use x86::task::tr;
use x86_64::instructions::interrupts;

use ku::time::{self, rtc::TICKS_PER_SECOND, CorrelationPoint, Tsc};
use text::println;
use crate::{
    error::{
        Error::{self, InvalidArgument},
        Result,
    },
    log::{error, info},
    SYSTEM_INFO,
};


// ANCHOR: interrupt
/// Обработчик прерываний
/// [часов реального времени (Real-time clock, RTC)](https://en.wikipedia.org/wiki/Real-time_clock).
pub(crate) fn interrupt() {
    if interrupt_status().contains(RegisterC::UPDATE_ENDED_INTERRUPT) {
        if let Some(timestamp) = timestamp() {
            let now = CorrelationPoint::now(timestamp * TICKS_PER_SECOND);
            let rtc = SYSTEM_INFO.rtc();
            rtc.init_base(now);
            let before_correction = time::datetime(Tsc::new(now.tsc()));
            rtc.store_prev(now);
            let after_correction = time::datetime(Tsc::new(now.tsc()));

            if let Some(error) = (before_correction - after_correction).num_nanoseconds() {
                ERROR.store(error, Ordering::Relaxed);
            } else if before_correction < after_correction {
                ERROR.store(i64::MIN, Ordering::Relaxed);
            } else {
                ERROR.store(i64::MAX, Ordering::Relaxed);
            }
        }
    }
}
// ANCHOR_END: interrupt


/// Инициализация микросхемы
/// [часов реального времени (Real-time clock, RTC)](https://en.wikipedia.org/wiki/Real-time_clock).
///
/// Во время изменения настроек RTC запрещает все
/// [прерывания](https://en.wikipedia.org/wiki/Interrupt),
/// в том числе
/// [немаскируемые](https://en.wikipedia.org/wiki/Non-maskable_interrupt).
/// Иначе она может остаться в
/// [некорректном состоянии](https://wiki.osdev.org/RTC#Avoiding_NMI_and_Other_Interrupts_While_Programming).
pub(super) fn init() {
    let mut old_settings = RegisterB::empty();
    let mut new_settings = RegisterB::empty();
    let mut acknowledged_settings = RegisterB::empty();

    // ANCHOR: init_configure
    interrupts::without_interrupts(|| {
        let address = DISABLE_NMI | REGISTER_B;
        defer! {
            rtc_read(!DISABLE_NMI);
        }

        /// Такого не должно происходить, все 8 бит в [`RegisterB`] определены.
        const UNDEFINED_FLAG_FOUND: &str = "undefined flag found in RTC register B";

        old_settings = RegisterB::from_bits(rtc_read(address)).expect(UNDEFINED_FLAG_FOUND);
        if old_settings.contains(RegisterB::DAYLIGHT_SAVING) {
            error!("RTC time is not in UTC (DST is on), expect the system time to be incorrect");
        }

        new_settings = (old_settings & !RegisterB::USE_24_HOUR_FORMAT) |
            RegisterB::UPDATE_ENDED_INTERRUPT;
        rtc_write(address, (new_settings | RegisterB::SET_CLOCK).bits());

        if new_settings.contains(RegisterB::USE_BINARY_FORMAT) {
            rtc_write(4, 11);
            rtc_write(2, 59);
            rtc_write(0, 58);
        } else {
            rtc_write(4, 1 * 16 + 1);
            rtc_write(2, 5 * 16 + 9);
            rtc_write(0, 5 * 16 + 5);
        }

        rtc_write(address, new_settings.bits());
        acknowledged_settings = RegisterB::from_bits(rtc_read(address)).expect(UNDEFINED_FLAG_FOUND);

    });
    // ANCHOR_END: init_configure

    // ANCHOR: init_read
    if acknowledged_settings == new_settings {
        // ANCHOR: first_correlation_point
        let rtc = SYSTEM_INFO.rtc();
        let timestamp = timestamp();
        rtc.store_prev(CorrelationPoint::invalid(
            timestamp.unwrap_or(0) * TICKS_PER_SECOND,
        ));
        // ANCHOR_END: first_correlation_point

        if !is_time_valid() {
            error!("RTC reports low battery, its time and date values are incorrect");
        } else if timestamp.is_none() {
            error!("failed to read time and date from RTC consistently");
        } else if timestamp == Some(0) {
            error!("wrong time and date from RTC");
        } else {
            info!(?acknowledged_settings, "RTC init");
        }
    } else {
        error!(
            ?old_settings,
            ?new_settings,
            ?acknowledged_settings,
            "RTC did not acknowledge the new settings",
        );
    }
    // ANCHOR_END: init_read
}


/// Значение ошибки предсказания времени для последнего прерывания RTC.
///
/// Т.е. разность времени, предсказанного для показаний RTC по счётчику тактов процессора
/// и времени по показаниям RTC для момента последнего прерывания.
pub fn error() -> Duration {
    Duration::nanoseconds(ERROR.load(Ordering::Relaxed))
}


// ANCHOR: enable_next_interrupt
/// Говорит микросхеме RTC, что процессор обработал
/// [прерывание](https://en.wikipedia.org/wiki/Interrupt)
/// от неё.
/// То есть, посылает ей
/// [end of interrupt (EOI)](https://en.wikipedia.org/wiki/End_of_interrupt).
///
/// См. [Interrupts and Register C](https://wiki.osdev.org/RTC#Interrupts_and_Register_C).
pub(crate) fn enable_next_interrupt() {
    interrupt_status();
}
// ANCHOR_END: enable_next_interrupt


bitflags! {
    /// Регистр статуса RTC.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct RegisterA: u8 {
        const UPDATE_IN_PROGRESS = 1 << 7;
    }
}


bitflags! {
    /// Регистр настроек RTC.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct RegisterB: u8 {
        /// Включает переход на летнее время.
        const DAYLIGHT_SAVING = 1 << 0;

        /// Время в микросхеме хранится
        /// в [24-часовом формате](https://en.wikipedia.org/wiki/24-hour_clock),
        /// а не в [12--часовом](https://en.wikipedia.org/wiki/12-hour_clock).
        const USE_24_HOUR_FORMAT = 1 << 1;

        /// Время в микросхеме хранится
        /// в [двоичном коде](https://en.wikipedia.org/wiki/Binary_number),
        /// а не в [двоично--десятичном](https://en.wikipedia.org/wiki/Binary-coded_decimal).
        const USE_BINARY_FORMAT = 1 << 2;

        /// Генерировать сигнал с конфигурируемой частотой но отдельном выходе микросхемы.
        const SQUARE_WAVE = 1 << 3;

        /// Включает
        /// [прерывание](https://en.wikipedia.org/wiki/Interrupt),
        /// посылаемое процессору микросхемой после обновления показаний времени при тике.
        const UPDATE_ENDED_INTERRUPT = 1 << 4;

        /// Включает
        /// [прерывание](https://en.wikipedia.org/wiki/Interrupt),
        /// посылаемое процессору при срабатывании будильника.
        const ALARM_INTERRUPT = 1 << 5;

        /// Включает периодическое
        /// [прерывание](https://en.wikipedia.org/wiki/Interrupt)
        /// с конфигурируемой частотой.
        const PERIODIC_INTERRUPT = 1 << 6;

        /// Сообщает микросхеме, что процессор меняет дату и время.
        /// Пока процессор не сбросит этот бит, микросхема не будет их обновлять.
        const SET_CLOCK = 1 << 7;
    }
}


bitflags! {
    /// Регистр статуса прерывания RTC. Сбрасывается при чтении.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct RegisterC: u8 {
        /// Микросхема сгенерировала прерывание как минимум одного из типов.
        const INTERRUPT = 1 << 7;

        /// Сгенерировано периодическое прерывание.
        const PERIODIC_INTERRUPT = 1 << 6;

        /// Сгенерировано прерывание будильника.
        const ALARM_INTERRUPT = 1 << 5;

        /// Сгенерировано прерывание после обновления показаний времени.
        const UPDATE_ENDED_INTERRUPT = 1 << 4;
    }
}


bitflags! {
    /// Регистр сохранности данных в памяти RTC при выключении.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct RegisterD: u8 {
        /// Есть заряд в батарейке.
        /// Поэтому данные в памяти RTC валидны, в том числе дата и время.
        const VALID_RAM_AND_TIME = 1 << 7;
    }
}


// ANCHOR: date
/// Структура для хранения даты
/// (по [григорианскому календарю](https://en.wikipedia.org/wiki/Gregorian_calendar))
/// и времени, прочитанных из микросхемы
/// [часов реального времени (Real-time clock, RTC)](https://en.wikipedia.org/wiki/Real-time_clock).
#[derive(Clone, Copy, Default, Display, Eq, PartialEq)]
#[display(
    "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
    year,
    month,
    day,
    hour,
    minute,
    second
)]
struct Date {
    /// Год по
    /// [григорианскому календарю](https://en.wikipedia.org/wiki/Gregorian_calendar).
    year: u16,

    /// Месяц по
    /// [григорианскому календарю](https://en.wikipedia.org/wiki/Gregorian_calendar).
    month: u8,

    /// День месяца по
    /// [григорианскому календарю](https://en.wikipedia.org/wiki/Gregorian_calendar).
    day: u8,

    /// Час.
    hour: u8,

    /// Минута.
    minute: u8,

    /// Секунда.
    second: u8,
}
// ANCHOR_END: date


impl Date {
    /// [Пытается несколько раз прочитать данные из микросхемы RTC](https://wiki.osdev.org/CMOS#RTC_Update_In_Progress)
    /// методом [`Date::read_inconsistent()`].
    /// Перед каждым чтением в цикле ждёт, пока в регистре `A` микросхемы RTC флаг
    /// [`RegisterA::UPDATE_IN_PROGRESS`] установлен,
    /// то есть пока микросхема обновляет данные в своей памяти.
    /// Возвращает [`Some`], если два чтения подряд вернут одинаковое значение
    /// структуры [`Date`].

    fn read() -> Option<Self> {
        if !is_time_valid() {
            return None;
        }
        for _ in 0..3 {
            while is_update_in_progress() {
                core::hint::spin_loop();
            }

            let first_read = Self::read_inconsistent();

            while is_update_in_progress() {
                core::hint::spin_loop();
            }

            let second_read = Self::read_inconsistent();

            if first_read == second_read {
                return Some(first_read);
            }
        }

        None
    }

    /// Считывает из микросхемы RTC показания даты и времени и возвращает их в виде [`Date`].
    ///
    /// Может вернуть некорректное значение [`Date`],
    /// если во время его работы произошёл тик RTC и микросхема конкурентно
    /// обновляла содержимое соответствующих полей в своей памяти.
    fn read_inconsistent() -> Self {
        let raw_second : u8 = rtc_read(0x00);
        let raw_minute : u8 = rtc_read(0x02);
        let raw_hour : u8 = rtc_read(0x04);
        let raw_day : u8 = rtc_read(0x07);
        let raw_month : u8 = rtc_read(0x08);
        let raw_year : u8 = rtc_read(0x09);

        let format = RegisterB::from_bits_truncate(SETTINGS.load(Ordering::Relaxed));


        let second = parse_value(raw_second, format);
        let minute = parse_value(raw_minute, format);
        let hour = parse_hour(raw_hour, format);
        let day = parse_value(raw_day, format);
        let month = parse_value(raw_month, format);
        let year = 2000 + u16::from(parse_value(raw_year, format));

        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        }
    }
}

fn is_update_in_progress() -> bool {
    let status_reg_a : u8 = rtc_read(0x0A);
    RegisterA::from_bits_truncate(status_reg_a).contains(RegisterA::UPDATE_IN_PROGRESS)
}

impl TryFrom<Date> for NaiveDateTime {
    type Error = Error;


    fn try_from(date: Date) -> Result<Self> {
        if date == Date::default() {
            Ok(Self::default())
        } else {
            NaiveDate::from_ymd_opt(date.year.into(), date.month.into(), date.day.into())
                .and_then(|result| {
                    result.and_hms_opt(date.hour.into(), date.minute.into(), date.second.into())
                })
                .ok_or(InvalidArgument)
        }
    }
}


/// Номер порта для выбора адреса в памяти микросхемы RTC.
const ADDRESS_PORT: u16 = 0x0070;

/// Номер порта для обмена данными с памятью микросхемы RTC.
const DATA_PORT: u16 = 0x0071;


// ANCHOR: rtc_read
/// Читает значение, которое находится в байте номер `address` внутренней памяти микросхемы RTC.
///
/// Адрес `address` не имеет отношения к основной памяти компьютера,
/// он адресует внутреннюю память микросхемы RTC.
fn rtc_read(address: u8) -> u8 {
    unsafe {
        io::outb(ADDRESS_PORT, address);
        io::inb(DATA_PORT)
    }
}
// ANCHOR_END: rtc_read


// ANCHOR: rtc_write
/// Записывает значение `data`, в байт номер `address` внутренней памяти микросхемы RTC.
///
/// Адрес `address` не имеет отношения к основной памяти компьютера,
/// он адресует внутреннюю память микросхемы RTC.
fn rtc_write(address: u8, data: u8) {
    unsafe {
        io::outb(ADDRESS_PORT, address);
        io::outb(DATA_PORT, data);
    }
}
// ANCHOR_END: rtc_write


/// Читает регистр статуса прерывания RTC.
fn interrupt_status() -> RegisterC {
    RegisterC::from_bits_truncate(rtc_read(REGISTER_C))
}


/// Проверяет правильность данных даты и времени в RTC.
///
/// На самом деле просто заряд в батарейке.
fn is_time_valid() -> bool {
    let register_d = RegisterD::from_bits_truncate(rtc_read(REGISTER_D));
    register_d.contains(RegisterD::VALID_RAM_AND_TIME)
}


// ANCHOR: timestamp
/// Переводит текущее время RTC в
/// [секунды с момента начала Unix--эпохи](https://en.wikipedia.org/wiki/Unix_time).
/// Предполагает, что микросхема RTC хранит
/// [всемирное координированное время (Coordinated Universal Time, UTC)](https://en.wikipedia.org/wiki/Coordinated_Universal_Time).
fn timestamp() -> Option<i64> {
    Date::read()
        .and_then(|date| date.try_into().ok())
        .map(|date| DateTime::<Utc>::from_naive_utc_and_offset(date, Utc).timestamp())
}
// ANCHOR_END: timestamp


/// Переводит значение `x` из формата RTC `format` в двоичный.
///
/// `format` может быть как
/// [двоично--десятичным](https://en.wikipedia.org/wiki/Binary-coded_decimal), так и
/// [двоичным](https://en.wikipedia.org/wiki/Binary_number),
/// см. [`RegisterB`].
///
/// [Подробнее про формат времени в RTC](https://wiki.osdev.org/CMOS#Format_of_Bytes).
// Просто перерводит в hexlit не убирает старший бит
fn parse_value(x: u8, format: RegisterB) -> u8 {
    if format.contains(RegisterB::USE_BINARY_FORMAT) {
        x
    } else {
       // let pm_bit = x & 0x80;
        ((x >> 4) * 10) + (x & 0x0F)
        //result | pm_bit
            //((x & 0xF0) >> 1) + ( (x & 0xF0) >> 3) + (x & 0xf)
    }
}


/// Переводит `hour` из формата RTC `format` в двоичный 24-ти часовой формат.
///
/// `format` может иметь четыре варианта, независимо задаваемые двумя битами, см. [`RegisterB`]:
///   - [двоично--десятичный](https://en.wikipedia.org/wiki/Binary-coded_decimal) или
///     [двоичный](https://en.wikipedia.org/wiki/Binary_number);
///   - [12-ти часовой](https://en.wikipedia.org/wiki/12-hour_clock) или
///     [24-часовой](https://en.wikipedia.org/wiki/24-hour_clock).
///
/// [Подробнее про формат времени в RTC](https://wiki.osdev.org/CMOS#Format_of_Bytes).
fn parse_hour(hour: u8, format: RegisterB) -> u8 {
    let mut hour_n;
    if !format.contains(RegisterB::USE_24_HOUR_FORMAT) {
        let is_pm = hour & 0x80 != 0;
        hour_n = hour & 0x7F;
        hour_n = parse_value(hour_n, format);

        if hour_n == 12 {
            if !is_pm {
                hour_n = 0;
            }
        } else if is_pm {
            hour_n += 12;
        }
    } else {
        hour_n = parse_value(hour, format);
    }
    hour_n
}


/// Значение ошибки предсказания времени для последнего прерывания RTC в наносекундах.
static ERROR: AtomicI64 = AtomicI64::new(0);

/// Копия текущих настроек микросхемы --- [`RegisterB`].
static SETTINGS: AtomicU8 = AtomicU8::new(0);


/// Запрет
/// [немаскируемых прерываний](https://en.wikipedia.org/wiki/Non-maskable_interrupt).
/// Разделяет тот же номер
/// [порта ввода--вывода](https://wiki.osdev.org/Port_IO),
/// что и [`ADDRESS_PORT`].
const DISABLE_NMI: u8 = 1 << 7;


/// Адрес регистра статуса RTC.
const REGISTER_A: u8 = 0xA;

/// Адрес регистра настроек RTC.
const REGISTER_B: u8 = 0xB;

/// Адрес регистра статуса прерывания RTC.
const REGISTER_C: u8 = 0xC;

/// Адрес регистра сохранности данных в памяти RTC при выключении.
const REGISTER_D: u8 = 0xD;


#[doc(hidden)]
pub(super) mod test_scaffolding {
    pub use super::RegisterB;


    pub fn parse_hour(hour: u8, format: RegisterB) -> u8 {
        super::parse_hour(hour, format)
    }
}
