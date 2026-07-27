use core::{mem, result};

use embedded_graphics_core::{
    draw_target::DrawTarget,
    geometry::{Dimensions, OriginDimensions, Point, Size},
    pixelcolor::PixelColor,
    primitives::rectangle::Rectangle,
    Pixel,
};

use ku::{
    error::{Error, Result},
    memory::{mmu::PageTableFlags, size, Block, Phys},
    time,
};

use kernel::{
    log::debug,
    memory::{BASE_ADDRESS_SPACE, KERNEL_RW},
};


/// Управляет содержимым экрана через его
/// [фрейм буффер](https://en.wikipedia.org/wiki/Framebuffer).
/// Поддерживает
/// [двойную буферизацию](https://en.wikipedia.org/wiki/Multiple_buffering).
pub struct FrameBuffer<Color: Default + PixelColor + 'static> {
    /// [Фрейм буффер](https://en.wikipedia.org/wiki/Framebuffer)
    /// экрана.
    /// Является первичным буфером (front buffer)
    /// [двойной буферизации](https://en.wikipedia.org/wiki/Multiple_buffering).
    /// Информация, записываемая в него сразу отображается на экране.
    front_buffer: &'static mut [Color],

    /// Вторичный буфер (back buffer)
    /// [двойной буферизации](https://en.wikipedia.org/wiki/Multiple_buffering).
    /// Информация, записываемая в него, не отображается до тех пор, пока не вызван метод
    /// [`FrameBuffer::flush()`].
    back_buffer: &'static mut [Color],

    /// Разрешение экрана.
    resolution: Size,

    /// Количество пикселей на экране.
    pixel_count: usize,

    /// Разность между индексами пикселей, соседних по вертикальной координате.
    /// В общем случае не равна `self.resolution.width`.
    stride: usize,
}


impl<Color: Default + PixelColor> FrameBuffer<Color> {
    /// Создаёт фрейм буфер по адресу `frame_buffer`
    /// с разрешением `resolution` и глубиной цвета, задаваемой `Color`.
    pub fn new(frame_buffer: Phys, resolution: Size) -> Result<Self> {
        let pixel_count = size::from(resolution.width * resolution.height);
        let frame_buffer_size = pixel_count * mem::size_of::<Color>();

        let (front_buffer, back_buffer) = Self::map_buffers(Block::new(
            frame_buffer,
            (frame_buffer + frame_buffer_size)?,
        )?)?;

        let mut frame_buffer = Self {
            front_buffer,
            back_buffer,
            resolution,
            pixel_count,
            stride: size::from(resolution.width),
        };

        frame_buffer.flush();

        Ok(frame_buffer)
    }


    /// Копирует содержимое вторичного буфера, накопившего изображение,
    /// в первичный.
    /// Это приводит к обновлению содержимого экрана.
    /// Не ждёт вертикальной синхронизации.
    pub fn flush(&mut self) {
        let timer = time::timer();

        self.front_buffer[..self.pixel_count]
            .copy_from_slice(&self.back_buffer[..self.pixel_count]);

        debug!(duration = %timer.elapsed(), "flush the frame buffer");
    }


    /// Записывает в заданный пиксел заданный цвет.
    /// Проверяет, что `pixel` находится внутри экрана.
    #[inline(always)]
    fn set_pixel(&mut self, pixel: Pixel<Color>) -> Result<()> {
        let Pixel::<Color>(point, color) = pixel;

        if self.bounding_box().contains(point) {
            self.back_buffer[self.index(point)?] = color;
        }

        Ok(())
    }


    /// Возвращает смещение пиксела с заданными координатами во
    /// [фрейм буффере](https://en.wikipedia.org/wiki/Framebuffer)
    /// экрана.
    /// Не проверяет, что `point` находится внутри экрана.
    #[inline(always)]
    fn index(&self, point: Point) -> Result<usize> {
        Ok(size::from(u32::try_from(point.x)?) + size::from(u32::try_from(point.y)?) * self.stride)
    }


    /// Отображает `frame_buffer` в виртуальную память в качестве первичного буфера.
    /// И создаёт вторичный буфер такого же размера.
    fn map_buffers(
        frame_buffer: Block<Phys>,
    ) -> Result<(&'static mut [Color], &'static mut [Color])> {
        let back_buffer_flags = KERNEL_RW;
        let front_buffer_flags =
            back_buffer_flags | PageTableFlags::NO_CACHE | PageTableFlags::WRITE_THROUGH;

        let mut address_space = BASE_ADDRESS_SPACE.lock();

        let front_buffer = address_space.allocate(frame_buffer.size(), KERNEL_RW)?;

        for (frame, page) in frame_buffer.enclosing().into_iter().zip(front_buffer.into_iter()) {
            unsafe {
                address_space.map_page_to_frame(page, frame, front_buffer_flags)?;
            }
        }

        let front_buffer = unsafe { front_buffer.try_into_mut_slice()? };
        let back_buffer =
            address_space.map_slice(front_buffer.len(), back_buffer_flags, Color::default)?;

        Ok((front_buffer, back_buffer))
    }
}


impl<Color: Default + PixelColor> Drop for FrameBuffer<Color> {
    fn drop(&mut self) {
        let message = "failed to unmap the frame buffer";
        let mut address_space = BASE_ADDRESS_SPACE.lock();
        unsafe {
            address_space.unmap_slice(self.front_buffer).expect(message);
            address_space.unmap_slice(self.back_buffer).expect(message);
        }
    }
}


impl<Color: Default + PixelColor> OriginDimensions for FrameBuffer<Color> {
    fn size(&self) -> Size {
        self.resolution
    }
}


impl<Color: Default + PixelColor> DrawTarget for FrameBuffer<Color> {
    type Color = Color;
    type Error = Error;


    fn draw_iter<I>(&mut self, pixels: I) -> result::Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for pixel in pixels.into_iter() {
            self.set_pixel(pixel)?;
        }

        Ok(())
    }


    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> result::Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        let area = area.intersection(&self.bounding_box());
        let mut colors = colors.into_iter();
        let mut start = self.index(area.top_left)?;

        for _ in 0..area.size.height {
            let end = start + size::from(area.size.width);

            for (pixel, color) in self.back_buffer[start..end].iter_mut().zip(&mut colors) {
                *pixel = color;
            }

            start += self.stride;
        }

        Ok(())
    }


    fn fill_solid(
        &mut self,
        area: &Rectangle,
        color: Self::Color,
    ) -> result::Result<(), Self::Error> {
        let area = area.intersection(&self.bounding_box());
        let mut start = self.index(area.top_left)?;

        for _ in 0..area.size.height {
            let end = start + size::from(area.size.width);

            self.back_buffer[start..end].fill(color);

            start += self.stride;
        }

        Ok(())
    }


    fn clear(&mut self, color: Self::Color) -> result::Result<(), Self::Error> {
        self.back_buffer[..self.pixel_count].fill(color);

        Ok(())
    }
}
