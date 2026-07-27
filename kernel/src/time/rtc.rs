use core::{
    hint,
    sync::atomic::{AtomicI64, AtomicU8, Ordering},
};

use bitflags::bitflags;
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, Utc};
use derive_more::Display;
use scopeguard::defer;
use x86::io;
use x86_64::instructions::interrupts;

use ku::time::{self, rtc::TICKS_PER_SECOND, CorrelationPoint, Tsc};

use crate::{
    error::{
        Error::{self, InvalidArgument},
        Result,
    },
    log::{error, info},
    SYSTEM_INFO,
};


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

    interrupts::without_interrupts(|| {
        defer! {
            rtc_read(!DISABLE_NMI);
        }

        /// Такого не должно происходить, все 8 бит в [`RegisterB`] определены.
        const UNDEFINED_FLAG_FOUND: &str = "undefined flag found in RTC register B";

        old_settings =
            RegisterB::from_bits(rtc_read(DISABLE_NMI | REGISTER_B)).expect(UNDEFINED_FLAG_FOUND);
        new_settings =
            (old_settings & !RegisterB::DAYLIGHT_SAVING) | RegisterB::UPDATE_ENDED_INTERRUPT;
        rtc_write(DISABLE_NMI | REGISTER_B, new_settings.bits());
        acknowledged_settings =
            RegisterB::from_bits(rtc_read(DISABLE_NMI | REGISTER_B)).expect(UNDEFINED_FLAG_FOUND);

        SETTINGS.store(acknowledged_settings.bits(), Ordering::Relaxed);
    });

    if acknowledged_settings == new_settings {
        let rtc = SYSTEM_INFO.rtc();
        let timestamp = timestamp();
        rtc.store_prev(CorrelationPoint::invalid(
            timestamp.unwrap_or(0) * TICKS_PER_SECOND,
        ));

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
}


/// Значение ошибки предсказания времени для последнего прерывания RTC.
///
/// Т.е. разность времени, предсказанного для показаний RTC по счётчику тактов процессора
/// и времени по показаниям RTC для момента последнего прерывания.
pub fn error() -> Duration {
    Duration::nanoseconds(ERROR.load(Ordering::Relaxed))
}


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

        // TODO: your code here.
        Some(Self::read_inconsistent()) // TODO: remove before flight.
    }


    /// Считывает из микросхемы RTC показания даты и времени и возвращает их в виде [`Date`].
    ///
    /// Может вернуть некорректное значение [`Date`],
    /// если во время его работы произошёл тик RTC и микросхема конкурентно
    /// обновляла содержимое соответствующих полей в своей памяти.
    fn read_inconsistent() -> Self {
        // TODO: your code here.
        Self::default() // TODO: remove before flight.
    }
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


/// Переводит текущее время RTC в
/// [секунды с момента начала Unix--эпохи](https://en.wikipedia.org/wiki/Unix_time).
/// Предполагает, что микросхема RTC хранит
/// [всемирное координированное время (Coordinated Universal Time, UTC)](https://en.wikipedia.org/wiki/Coordinated_Universal_Time).
fn timestamp() -> Option<i64> {
    Date::read()
        .and_then(|date| date.try_into().ok())
        .map(|date| DateTime::<Utc>::from_naive_utc_and_offset(date, Utc).timestamp())
}


/// Переводит значение `x` из формата RTC `format` в двоичный.
///
/// `format` может быть как
/// [двоично--десятичным](https://en.wikipedia.org/wiki/Binary-coded_decimal), так и
/// [двоичным](https://en.wikipedia.org/wiki/Binary_number),
/// см. [`RegisterB`].
///
/// [Подробнее про формат времени в RTC](https://wiki.osdev.org/CMOS#Format_of_Bytes).
fn parse_value(x: u8, format: RegisterB) -> u8 {
    // TODO: your code here.
    x // TODO: remove before flight.
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
    // TODO: your code here.
    hour // TODO: remove before flight.
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
