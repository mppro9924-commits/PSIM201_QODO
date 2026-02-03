use defmt::*;
use embassy_stm32::i2c::I2c;
use embassy_stm32::peripherals::I2C1;
use embedded_hal_async::i2c::I2c as AsyncI2c;

pub struct Mcp23017<'d> {
    i2c: I2c<'d, I2C1>,
    addr: u8,
}

/* Register map used */
/* IODIRA 0x00, IODIRB 0x01; OLATA 0x14, OLATB 0x15; GPIOA 0x12, GPIOB 0x13; GPPU 0x0C/0x0D */

impl<'d> Mcp23017<'d> {
    pub fn new(i2c: I2c<'d, I2C1>, addr: u8) -> Self { Self { i2c, addr } }

    async fn write_reg(&mut self, reg: u8, data: u8) -> Result<(), ()> {
        let buf = [reg, data];
        self.i2c.write(self.addr, &buf).await.map_err(|_| ())
    }
    async fn read_reg(&mut self, reg: u8, out: &mut [u8]) -> Result<(), ()> {
        self.i2c.write(self.addr, &[reg]).await.map_err(|_| ())?;
        self.i2c.read(self.addr, out).await.map_err(|_| ())
    }

    pub async fn init(mut self) -> Result<Self, ()> {
        // all outputs
        self.write_reg(0x00, 0x00).await?; // IODIRA
        self.write_reg(0x01, 0x00).await?; // IODIRB
        self.write_reg(0x02, 0x00).await?; // IPOLA
        self.write_reg(0x03, 0x00).await?; // IPOLB
        self.write_reg(0x0C, 0x00).await?; // GPPUA
        self.write_reg(0x0D, 0x00).await?; // GPPUB
        self.write_reg(0x14, 0x00).await?; // OLATA
        self.write_reg(0x15, 0x00).await?; // OLATB
        Ok(self)
    }

    pub async fn set_gpb(&mut self, mask: u8, value: u8) -> Result<(), ()> {
        let mut buf = [0u8];
        self.read_reg(0x15, &mut buf).await?;
        let cur = buf[0];
        let newv = (cur & !mask) | (value & mask);
        self.write_reg(0x15, newv).await
    }

    pub async fn set_gpa(&mut self, mask: u8, value: u8) -> Result<(), ()> {
        let mut buf = [0u8];
        self.read_reg(0x14, &mut buf).await?;
        let cur = buf[0];
        let newv = (cur & !mask) | (value & mask);
        self.write_reg(0x14, newv).await
    }

    pub async fn write_gpb(&mut self, value: u8) -> Result<(), ()> { self.write_reg(0x15, value).await }
    pub async fn write_gpa(&mut self, value: u8) -> Result<(), ()> { self.write_reg(0x14, value).await }
}
