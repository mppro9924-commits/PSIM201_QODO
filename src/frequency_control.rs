use defmt::*;
use embassy_time::Timer;
use embassy_sync::channel::mpmc::Channel;
use embassy_stm32::gpio::{Output, Level, Speed};
use embassy_stm32::peripherals::TIM2;
use crate::hv_control::HvCommand;

#[derive(Copy, Clone, Debug, defmt::Format)]
pub enum FrequencyCmd { CycleNext, EnterInputCaptureMode, SetFrequency(u32) }

pub struct FrequencyControl { idx: usize, table: [u32; 12], freq_hz: u32 }
impl FrequencyControl {
    pub fn new() -> Self {
        Self { idx: 0, table: [1,2,5,10,20,50,60,100,200,400,0,0], freq_hz: 1 }
    }
    pub fn next(&mut self) -> u32 { self.idx = (self.idx+1)%self.table.len(); self.freq_hz = self.table[self.idx]; self.freq_hz }
    pub fn set(&mut self, f: u32) -> u32 { self.freq_hz = f; f }
    pub fn current(&self) -> u32 { self.freq_hz }
}

#[embassy_executor::task]
pub async fn frequency_task<'d>(
    pa5: embassy_stm32::peripherals::PA5,
    _tim2: TIM2,
    mut rx: Channel<FrequencyCmd, 8>::Receiver,
    hv_tx: Channel<HvCommand, 8>::Sender,
) {
    let mut ctrl = FrequencyControl::new();
    let mut pin = Output::new(pa5, Level::Low, Speed::VeryHigh);
    let mut running = true;
    loop {
        while let Ok(cmd) = rx.try_receive() {
            match cmd {
                FrequencyCmd::CycleNext => {
                    let f = ctrl.next();
                    info!("Frequency -> {=u32} Hz", f);
                    if f == 0 { running = false; let _ = hv_tx.send(HvCommand::ForceStop).await; pin.set_low(); } else { running = true; }
                }
                FrequencyCmd::EnterInputCaptureMode => { info!("Enter input capture mode PA2 (TIM2_CH3)"); }
                FrequencyCmd::SetFrequency(f) => { ctrl.set(f); if f == 0 { running=false; pin.set_low(); } else { running = true; } }
            }
        }
        if running {
            let f = ctrl.current();
            if f>0 { let half = 500_000u32 / f; pin.toggle(); Timer::after_micros(half as u64).await; }
            else { pin.set_low(); Timer::after_millis(10).await; }
        } else {
            pin.set_low(); Timer::after_millis(50).await;
        }
    }
}
