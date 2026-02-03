use defmt::*;
use embassy_stm32::i2c::I2c;
use embassy_stm32::peripherals::I2C3;
use embedded_hal_async::i2c::I2c as AsyncI2c;

pub struct Mcp3424<'d> {
    i2c: I2c<'d, I2C3>,
    addr: u8,
}

impl<'d> Mcp3424<'d> {
    pub fn new(i2c: I2c<'d, I2C3>, addr: u8) -> Self { Self { i2c, addr } }

    pub async fn init_18bit_pga1(&mut self) -> Result<(), ()> { Ok(()) }

    async fn start_conversion(&mut self, channel: u8) -> Result<(), ()> {
        let chan_bits = match channel { 1 => 0b00, 2 => 0b01, 3 => 0b10, 4 => 0b11, _ => 0b00 };
        let cfg = (0 << 7) | (chan_bits << 5) | (0 << 4) | (0b11 << 2) | 0b00;
        self.i2c.write(self.addr, &[cfg]).await.map_err(|_| ())
    }

    pub async fn read_channel_uv(&mut self, channel: u8) -> Result<i64, ()> {
        self.start_conversion(channel).await?;
        // Wait up to ~300ms for 18-bit conversion
        for _ in 0..7 {
            embassy_time::Timer::after_millis(50).await;
            let mut buf = [0u8; 4];
            if self.i2c.read(self.addr, &mut buf).await.is_ok() {
                if (buf[3] & 0x80) == 0 {
                    let raw = (((buf[0] as i32) << 16) | ((buf[1] as i32) << 8) | (buf[2] as i32)) >> 6;
                    let value = if (raw & (1 << 17)) != 0 { raw | !0x3FFFF } else { raw & 0x3FFFF };
                    let microvolts = (value as i64) * 15625 / 1000; // 15.625 uV LSB
                    return Ok(microvolts);
                }
            }
        }
        Err(())
    }

    pub fn uv_to_volts(uv: i64) -> f32 { (uv as f32) / 1_000_000.0 }
}
