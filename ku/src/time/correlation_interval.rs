#![allow(rustdoc::private_intra_doc_links)]
#![forbid(unsafe_code)]

use core::ops::{Mul, MulAssign};
use chrono::{DateTime, NaiveDateTime, NaiveTime, TimeZone, Utc};
use num_traits::{Euclid, MulAdd};
use super::{
    correlation_point::{AtomicCorrelationPoint, CorrelationPoint},
    Hz,
    Tsc,
    NSECS_PER_SEC,
};


/// Предназначена для соотнесения частоты процессора и другого источника времени.
///
/// Частота отслеживаемых часов задаётся как константный параметр `TICKS_PER_SECOND`.
///
/// По этой информации можно:
/// - Вычислить частоту процессора с точки зрения отслеживаемых часов.
/// - Пересчитать произвольное значение счётчика тактов процессора [`Tsc`]
///   в показания времени отслеживаемых часов.
///   При этом мы фактически повышаем разрешение отслеживаемых часов до частоты процессора.
///   Разумеется, погрешность при этом превышает разрешение.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CorrelationInterval<const TICKS_PER_SECOND: i64> {
    /// Значение [`CorrelationPoint`] в некоторый базовый момент времени,
    /// когда тикнули отслеживаемые [`CorrelationInterval`] часы.
    base: CorrelationPoint,

    /// Значение [`CorrelationPoint`] на момент последнего тика отслеживаемых часов.
    prev: CorrelationPoint,
}


impl<const TICKS_PER_SECOND: i64> CorrelationInterval<TICKS_PER_SECOND> {
    /// Выдаёт время, соответствующее такту процессора, записанному в `tsc`,
    /// для часов, к которым привязан `atomic_correlation_interval`.
    ///
    /// Считает, что заданные ему часы показывают количество секунд,
    /// прошедших с начала Unix--эпохи в [`Utc`].
    /// В тех редких случаях, когда в `atomic_correlation_interval` ещё не прошло два тика часов,
    /// возвращает момент времени [`CorrelationInterval::prev`], **игнорируя** `tsc`.
    pub fn datetime<const PARTS_PER_SECOND: i64>(
        atomic_correlation_interval: &AtomicCorrelationInterval<TICKS_PER_SECOND>,
        tsc: Tsc,
    ) -> DateTime<Utc> {
        let timer = atomic_correlation_interval.load();

        if timer.elapsed_count() > 0 {
            timer.datetime_with_resolution::<PARTS_PER_SECOND>(tsc)
        } else {
            DateTime::from_timestamp(timer.prev.count() / TICKS_PER_SECOND, 0)
                .expect(UNEXPECTED_TIMESTAMP)
        }
    }


    /// Возвращает реальное время, которое соответствует тику номер `tsc` процессора.
    /// Значение `tsc` может быть как больше [`CorrelationInterval::prev`],
    /// так и меньше [`CorrelationInterval::base`], а может лежать где-то между ними.
    ///
    /// - Запрошенное разрешение `PARTS_PER_SECOND` задаётся в единицах в секунду,
    ///   например для миллисекунд `PARTS_PER_SECOND = 1_000`.
    ///   Оно должно быть не точнее наносекунд --- `PARTS_PER_SECOND = 1_000_000_000`.
    /// - Не поддерживает [високосные секунды](https://en.wikipedia.org/wiki/Leap_second).
    fn datetime_with_resolution<const PARTS_PER_SECOND: i64>(&self, tsc: Tsc) -> DateTime<Utc> {
        let tsc_per_tick = self.tsc_per_second();

        let delta_tsc_from_prev = tsc.get() - self.prev.tsc();

        let total_seconds = self.prev.count() + delta_tsc_from_prev.div_euclid(tsc_per_tick);

        let subsecond_parts = (delta_tsc_from_prev.rem_euclid(tsc_per_tick) * PARTS_PER_SECOND)
            .div_euclid(tsc_per_tick);

        DateTime::from_timestamp(total_seconds, subsecond_parts as u32).unwrap()
    }


    /// Возвращает частоту процессора с точки зрения часов,
    /// которые отслеживает этот [`CorrelationInterval`].
    fn tsc_per_second(&self) -> i64 {
        let elapsed_count = self.elapsed_count();
        if elapsed_count > 0 {
            TICKS_PER_SECOND * self.elapsed_tsc() / elapsed_count
        } else {
            0
        }
    }


    /// Возвращает количество тиков отслеживаемых часов между точками
    /// [`CorrelationInterval::base`] и [`CorrelationInterval::prev`].
    fn elapsed_count(&self) -> i64 {
        if self.base.is_valid() && self.prev.is_valid() {
            self.prev.count() - self.base.count()
        } else {
            0
        }
    }


    /// Возвращает количество тактов процессора между точками
    /// [`CorrelationInterval::base`] и [`CorrelationInterval::prev`].
    fn elapsed_tsc(&self) -> i64 {
        assert!(self.base.is_valid());
        assert!(self.prev.is_valid());

        self.prev.tsc() - self.base.tsc()
    }
}


/// Конкурентное хранилище в памяти для [`CorrelationInterval`].
#[derive(Debug, Default)]
pub struct AtomicCorrelationInterval<const TICKS_PER_SECOND: i64> {
    /// Значение [`AtomicCorrelationPoint`] в некоторый базовый момент времени,
    /// когда тикнули отслеживаемые [`AtomicCorrelationInterval`] часы.
    base: AtomicCorrelationPoint,

    /// Значение [`AtomicCorrelationPoint`] на момент последнего тика отслеживаемых часов.
    prev: AtomicCorrelationPoint,
}


impl<const TICKS_PER_SECOND: i64> AtomicCorrelationInterval<TICKS_PER_SECOND> {
    /// Возвращает [`AtomicCorrelationInterval`], заполненную нулями.
    /// Аналогична [`AtomicCorrelationInterval::default()`], но доступна в константном контексте.
    pub const fn new() -> Self {
        Self {
            base: AtomicCorrelationPoint::new(),
            prev: AtomicCorrelationPoint::new(),
        }
    }


    /// Читает значение [`CorrelationInterval`] из структуры [`AtomicCorrelationInterval`].
    pub fn load(&self) -> CorrelationInterval<TICKS_PER_SECOND> {
        CorrelationInterval {
            base: self.base.load(),
            prev: self.prev.load(),
        }
    }


    /// Инициализирует [`AtomicCorrelationInterval::base`] значением `base`,
    /// если оно ещё не инициализировано.
    pub fn init_base(&self, base: CorrelationPoint) {
        if !self.base.is_valid() {
            self.base.store(base);
        }
    }


    /// Сохраняет `prev` в значение [`AtomicCorrelationInterval::prev`].
    pub fn store_prev(&self, prev: CorrelationPoint) {
        self.prev.store(prev);
    }


    /// Инкрементирует значение [`AtomicCorrelationInterval::prev`] и
    /// привязывает его к тику `tsc` процессора.
    pub fn inc_prev(&self, tsc: i64) {
        self.prev.inc(tsc);
    }


    /// Возвращает частоту процессора с точки зрения часов,
    /// которые отслеживает этот [`AtomicCorrelationInterval`].
    pub fn tsc_per_second(&self) -> Option<Hz> {
        self.load().tsc_per_second().try_into().ok().and_then(Hz::new)
    }
}


#[doc(hidden)]
pub mod test_scaffolding {
    use chrono::{DateTime, Utc};

    use super::{
        super::{correlation_point::test_scaffolding::new_point, Tsc},
        CorrelationInterval,
    };


    pub fn new_correlation_interval<const TICKS_PER_SECOND: i64>(
        base_tsc: i64,
        prev_tsc: i64,
    ) -> CorrelationInterval<TICKS_PER_SECOND> {
        CorrelationInterval {
            base: new_point(0, base_tsc),
            prev: new_point(1, prev_tsc),
        }
    }


    pub fn datetime_with_resolution<const PARTS_PER_SECOND: i64, const TICKS_PER_SECOND: i64>(
        correlation_interval: &CorrelationInterval<TICKS_PER_SECOND>,
        tsc: Tsc,
    ) -> DateTime<Utc> {
        correlation_interval.datetime_with_resolution::<PARTS_PER_SECOND>(tsc)
    }
}


/// Сообщение для паники при обнаружении заведомо некорректной даты с точки зрения [`chrono`].
const UNEXPECTED_TIMESTAMP: &str =
    "unexpected timestamp - more than ca. 262_000 years away from common era";
