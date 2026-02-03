#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;
use defmt::*;
use embassy_executor::Spawner;
use embassy_time::{Timer, Duration};
use embassy_sync::channel::mpmc::Channel;
use embassy_stm32::bind_interrupts;
use embassy_stm32::peripherals;
use embassy_stm32::i2c::I2c;
use embassy_stm32::gpio::{Input, Output, Level, Pull, Speed};
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::dac::Dac;
use embassy_stm32::usart::{Uart, Config as UartConfig};
use embassy_stm32::time::Hertz;

mod drivers;
mod safety;
mod hv_control;
mod dac_control;
mod frequency_control;
mod buttons;
mod board_id;

use drivers::mcp23017::Mcp23017;
use drivers::mcp3424::Mcp3424;
use hv_control::HvCommand;
use dac_control::DacCmd;
use frequency_control::FrequencyCmd;

#[derive(Clone, Copy, Debug, defmt::Format)]
pub enum ButtonsEvent {
    PcShort,        // PB0
    PcLong,
    PolarityShort,  // PA9
    PolarityLong,
    FreqShort,      // PA12
    FreqLong,
}

bind_interrupts!(struct Irqs {
    I2C1_EV => embassy_stm32::i2c::InterruptHandler<peripherals::I2C1>;
    I2C1_ER => embassy_stm32::i2c::InterruptHandler<peripherals::I2C1>;
    I2C3_EV => embassy_stm32::i2c::InterruptHandler<peripherals::I2C3>;
    I2C3_ER => embassy_stm32::i2c::InterruptHandler<peripherals::I2C3>;
    USART2 => embassy_stm32::usart::InterruptHandler<peripherals::USART2>;
});

static BUTTON_EVENTS: Channel<ButtonsEvent, 8> = Channel::new();
static FREQ_CH: Channel<FrequencyCmd, 8> = Channel::new();
static DAC_CH: Channel<DacCmd, 8> = Channel::new();
static HV_CH: Channel<HvCommand, 8> = Channel::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    // I/O_Exp_RST (PA3) normally HIGH
    let mut io_exp_rst = Output::new(p.PA3, Level::High, Speed::Low);
    // KILL_N (PA6) active-low, start HIGH (not killed)
    let mut kill_n = Output::new(p.PA6, Level::High, Speed::Low);

    // DAC1 (PA4)
    let mut dac = Dac::new(p.DAC);
    dac.enable_channel(embassy_stm32::dac::Channel::Ch1);
    dac.set_value(embassy_stm32::dac::Channel::Ch1, 0);

    // I2C1 (PB6/PB7) @ 100 kHz for MCP23017
    let i2c1 = I2c::new(p.I2C1, p.PB6, p.PB7, Irqs, Hertz(100_000), Default::default());
    // I2C3 (PA7/PB4) @ 400 kHz for MCP3424
    let i2c3 = I2c::new(p.I2C3, p.PA7, p.PB4, Irqs, Hertz(400_000), Default::default());

    // Expander reset pulse
    io_exp_rst.set_low();
    Timer::after_millis(1).await;
    io_exp_rst.set_high();
    Timer::after_millis(1).await;

    // Drivers
    let expander = Mcp23017::new(i2c1, 0x20);
    let adc = Mcp3424::new(i2c3, 0x68);

    // Board ID PA10/PA15
    let _id = board_id::read_board_id(p.PA10, p.PA15);

    // Buttons: PB0, PA9, PA12
    let pb0 = ExtiInput::new(Input::new(p.PB0, Pull::Up), p.EXTI0);
    let pa9 = ExtiInput::new(Input::new(p.PA9, Pull::Up), p.EXTI9);
    let pa12 = ExtiInput::new(Input::new(p.PA12, Pull::Up), p.EXTI12);

    // Spawn tasks
    spawner.spawn(buttons::buttons_task(pb0, pa9, pa12, BUTTON_EVENTS.sender())).unwrap();

    spawner.spawn(frequency_control::frequency_task(p.PA5, p.TIM2, FREQ_CH.receiver(), HV_CH.sender())).unwrap();
    spawner.spawn(dac_control::dac_task(dac, DAC_CH.receiver())).unwrap();
    spawner.spawn(safety::safety_task(adc, DAC_CH.sender(), HV_CH.sender())).unwrap();
    spawner.spawn(hv_control::hv_task(expander, DAC_CH.sender(), FREQ_CH.sender(), HV_CH.receiver())).unwrap();

    info!("Boot complete");

    loop {
        let evt = BUTTON_EVENTS.receive().await;
        match evt {
            ButtonsEvent::PcShort => { let _ = DAC_CH.sender().send(dac_control::DacCmd::ShortStep).await; }
            ButtonsEvent::PcLong => { let _ = DAC_CH.sender().send(dac_control::DacCmd::StartRamp).await; }
            ButtonsEvent::PolarityShort => { let _ = HV_CH.sender().send(hv_control::HvCommand::RequestPolarityToggle).await; }
            ButtonsEvent::PolarityLong => {}
            ButtonsEvent::FreqShort => { let _ = FREQ_CH.sender().send(frequency_control::FrequencyCmd::CycleNext).await; }
            ButtonsEvent::FreqLong => { let _ = FREQ_CH.sender().send(frequency_control::FrequencyCmd::EnterInputCaptureMode).await; }
        }
    }
}
