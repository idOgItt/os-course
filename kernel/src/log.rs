use core::fmt::{Debug, Write};

use serde::Deserialize;
use tracing::{
    field::{Field, Visit},
    span::{Attributes, Record},
    Collect,
    Event,
    Id,
    Level,
    Metadata,
};
use tracing_core::{dispatch, dispatch::Dispatch, span::Current};

use ku::{
    log::{level_into_symbol, LogField, LogFieldValue, LogMetadata},
    sync::{PanicStrategy, Spinlock},
    time::{datetime_ms, Tsc},
    ReadBuffer,
};
use text::{print, println, Colour};

use crate::{
    error::{Error::Unimplemented, Result},
    process::Pid,
    smp::LocalApic,
};


pub use tracing::{debug, error, info, trace, warn};


/// Инициализация логирования.
pub(super) fn init() {
    dispatch::set_global_default(Dispatch::from_static(&LOG_COLLECTOR)).unwrap();
}


/// Записывает в лог все сообщения от пользовательского процесса `pid`,
/// сохранённые им в буфер `log`.
pub(super) fn user_events(pid: Pid, log: &mut ReadBuffer) {
    LOG_COLLECTOR.log.lock().user_events(pid, log);
}


/// Вспомогательная структура для печати сообщения.
struct LogEvent {
    /// Признак того, что нужно записать разделитель полей после ранее записанного поля.
    separator: bool,

    /// Текущий цвет при выводе части сообщения на экран.
    colour: Colour,
}


impl LogEvent {
    /// Цвет для вывода текста сообщения.
    const MESSAGE: Colour = Colour::WHITE;

    /// Цвет для вывода значений полей сообщения.
    const VALUE: Colour = Colour::LIGHT_CYAN;


    /// Создаёт вспомогательную структуру для печати сообщения.
    fn new() -> Self {
        Self {
            separator: false,
            colour: Self::VALUE,
        }
    }


    /// Печатает заголовок поля `name`.
    /// Если `name == "message"`, то это поле --- текст сообщения.
    /// Для него имя поля опускается.
    fn field(&mut self, name: &str) {
        if self.separator {
            print!("; ");
        } else {
            self.separator = true
        }

        if name == "message" {
            self.colour = Self::MESSAGE;
        } else {
            print!("{} = ", name);
            self.colour = Self::VALUE;
        }
    }


    /// Печатает строковый фрагмент `value_part` значения поля сообщения.
    fn str_value_part(&mut self, value_part: &str) {
        print!(colour(self.colour), "{}", value_part);
    }


    /// Печатает поле сообщения `name` со значением `value` через [`core::fmt::Debug::fmt()`].
    fn debug(&mut self, name: &str, value: &dyn Debug) {
        self.field(name);
        print!(colour(self.colour), "{:?}", value);
    }
}


impl Visit for LogEvent {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        self.debug(field.name(), value);
    }
}


/// Формат печати сообщений лога.
#[allow(unused)]
#[derive(Eq, PartialEq)]
enum Format {
    /// Компактный формат:
    /// ```console
    /// <время в UTC> <CPU id> <level char> <message>; <key1> = <value1>; <key2> = <value2>; ...
    /// ```
    /// Пример:
    /// ```console
    /// 17:50:31.233 0 I Nikka booted; now = 2023-01-01 17:50:31 UTC; tsc = Tsc(2850348853)
    /// ```
    ///
    /// - Сокращает уровень логирования до первой буквы.
    /// - Не печатает [`tracing::Metadata::target()`].
    /// - Не печатает файл [`tracing::Metadata::file()`] и строку [`tracing::Metadata::line()`]
    ///   исходного кода, где находится соответствующий вызов макроса логирования.
    Compact,

    /// Полный формат:
    /// ```console
    /// <время в UTC> <CPU id> <level> <target> <file:line> <message>; <key1> = <value1>; <key2> = <value2>; ...
    /// ```
    /// Пример:
    /// ```console
    /// 17:51:20.477 0 INFO kernel kernel/src/lib.rs:118 Nikka booted; now = 2023-01-01 17:51:20 UTC; tsc = Tsc(2732927919)
    /// ```
    Full,

    /// Аналогичен [`Format::Compact`], но дополнительно не печатает время:
    /// ```console
    /// <CPU id> <level char> <message>; <key1> = <value1>; <key2> = <value2>; ...
    /// ```
    /// Это позволяет писать в лог из функций, которые вычисляют текущее время, без зацикливания.
    /// Пример:
    /// ```console
    /// 0 I Nikka booted; now = 2023-01-01 17:52:07 UTC; tsc = Tsc(2787412607)
    /// ```
    ///
    /// - Не печатает текущее время.
    /// - Сокращает уровень логирования до первой буквы.
    /// - Не печатает [`tracing::Metadata::target()`].
    /// - Не печатает файл [`tracing::Metadata::file()`] и строку [`tracing::Metadata::line()`]
    ///   исходного кода, где находится соответствующий вызов макроса логирования.
    Timeless,
}


/// Логгер для печати сообщений в заданном формате.
struct Log {
    /// Формат печати сообщений лога.
    format: Format,
}


impl Log {
    /// Цвет для вывода номера CPU.
    const CPU: Colour = Colour::DARK_GREY;

    /// Цвет для вывода [`tracing::Metadata::target()`],
    /// файла [`tracing::Metadata::file()`] и строки [`tracing::Metadata::line()`].
    const LOCATION: Colour = Colour::DARK_GREY;


    /// Создаёт логгер для печати сообщений в формате `format`.
    const fn new(format: Format) -> Self {
        Self { format }
    }


    /// Возвращает цвет, которым нужно печатать уровень логирования `level`.
    const fn level_colour(level: &Level) -> Colour {
        match *level {
            Level::ERROR => Colour::LIGHT_RED,
            Level::WARN => Colour::LIGHT_YELLOW,
            Level::INFO => Colour::WHITE,
            Level::DEBUG => Colour::LIGHT_BLUE,
            Level::TRACE => Colour::DARK_GREY,
        }
    }


    /// Печатает сообщение `event` с отметкой времени `timestamp`.
    fn log_event(&self, event: &Event<'_>, timestamp: Tsc) {
        self.log_metadata(
            event.metadata().level(),
            LogMetadata::new(event.metadata(), timestamp),
        );
        event.record(&mut LogEvent::new());
        println!();
    }


    /// Печатает метаданные `metadata` сообщения, включая отметку времени,
    /// на уровне логирования `level`.
    fn log_metadata(&self, level: &Level, metadata: LogMetadata) {
        if self.format != Format::Timeless {
            let timestamp = datetime_ms(metadata.timestamp());
            print!("{:?} ", timestamp.time());
        }

        print!(colour(Self::CPU), "{} ", LocalApic::id());

        match self.format {
            Format::Compact | Format::Timeless => {
                print!(
                    colour(Self::level_colour(level)),
                    "{} ",
                    level_into_symbol(level),
                );
            },
            Format::Full => {
                print!(colour(Self::level_colour(level)), "{} ", level);
                print!(
                    colour(Self::LOCATION),
                    "{} {}:{} ",
                    metadata.target(),
                    metadata.file().unwrap_or("?"),
                    metadata.line().unwrap_or(0),
                );
            },
        }
    }


    /// Печатает все сообщения от пользовательского процесса `pid`,
    /// сериализованные им в буфер `log`.
    fn user_events(&self, pid: Pid, log: &mut ReadBuffer) {
        if let Some(mut tx) = log.read_tx() {
            while let Some(event) = unsafe { tx.read() } {
                let mut deserializer = postcard::Deserializer::from_bytes(event);
                if self.user_event(pid, &mut deserializer).is_err() {
                    return;
                }
            }

            tx.commit();
        }
    }


    /// Печатает одно сообщение от пользовательского процесса `pid`,
    /// десериализуя его из `deserializer`.
    fn user_event<'a>(
        &self,
        pid: Pid,
        deserializer: &mut postcard::Deserializer<'a, postcard::de_flavors::Slice<'a>>,
    ) -> Result<()> {
        let metadata = LogMetadata::deserialize(&mut *deserializer)?;
        let level = metadata.level().map_err(|_| Unimplemented)?;
        self.log_metadata(&level, metadata);

        let count = u8::deserialize(&mut *deserializer)?;
        let mut event = LogEvent::new();
        for _ in 0..count {
            let field = LogField::deserialize(&mut *deserializer)?;
            match field.value() {
                LogFieldValue::VecStr => {
                    event.field(field.name());
                    while let Some(value) = Option::<&str>::deserialize(&mut *deserializer)? {
                        event.str_value_part(value);
                    }
                },
                LogFieldValue::I64(value) => event.debug(field.name(), &value as &dyn Debug),
                LogFieldValue::U64(value) => event.debug(field.name(), &value as &dyn Debug),
                LogFieldValue::Bool(value) => event.debug(field.name(), &value as &dyn Debug),
                LogFieldValue::Str(value) => event.debug(field.name(), &value as &dyn Debug),
            }
        }
        event.debug("pid", &pid as &dyn Debug);

        println!();

        Ok(())
    }
}


/// Сборщик сообщений лога, печатающий сообщения на экран и в COM--порт.
struct LogCollector {
    /// Уровень логирования.
    /// Печатаются только сообщения с уровнем логирования, равным [`LogCollector::level`] и выше.
    level: Level,

    /// Логгер для печати сообщений в заданном формате.
    log: Spinlock<Log, { PanicStrategy::KnockDown }>,
}


impl LogCollector {
    /// Создаёт сборщик сообщений лога, печатающий сообщения с уровнем логирования `level` и выше
    /// на экран и в COM--порт в формате `format`.
    const fn new(format: Format, level: Level) -> Self {
        Self {
            level,
            log: Spinlock::new(Log::new(format)),
        }
    }
}


impl Collect for LogCollector {
    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(0)
    }


    fn event(&self, event: &Event<'_>) {
        let now = Tsc::now();
        self.log.lock().log_event(event, now);
    }


    fn record(&self, _span: &Id, _values: &Record<'_>) {
    }


    fn record_follows_from(&self, _span: &Id, _follows: &Id) {
    }


    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= &self.level
    }


    fn enter(&self, _span: &Id) {
    }


    fn exit(&self, _span: &Id) {
    }


    fn current_span(&self) -> Current {
        Current::unknown()
    }
}


/// Сборщик сообщений лога, печатающий сообщения на экран и в COM--порт.
static LOG_COLLECTOR: LogCollector = LogCollector::new(Format::Compact, Level::DEBUG);
