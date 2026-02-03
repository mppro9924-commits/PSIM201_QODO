use embassy_stm32::gpio::{Input, Pull};

pub fn read_board_id(pa10: embassy_stm32::peripherals::PA10, pa15: embassy_stm32::peripherals::PA15) -> u8 {
    let b0 = Input::new(pa10, Pull::Down).is_high() as u8;
    let b1 = Input::new(pa15, Pull::Down).is_high() as u8;
    (b1 << 1) | b0
}
