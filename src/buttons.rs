use defmt::*;
use embassy_stm32::exti::ExtiInput;
use embassy_time::Timer;
use embassy_sync::channel::mpmc::Channel;
use crate::ButtonsEvent;

const DEBOUNCE_MS: u64 = 30;
const LONG_PRESS_MS_PC: u64 = 800;
const LONG_PRESS_MS_FREQ: u64 = 1000;

#[embassy_executor::task]
pub async fn buttons_task<'d>(
    mut pb0_pc: ExtiInput<'d>,
    mut pa9_pol: ExtiInput<'d>,
    mut pa12_freq: ExtiInput<'d>,
    tx: Channel<ButtonsEvent, 8>::Sender,
) {
    loop {
        pb0_pc.wait_for_falling_edge().await;
        Timer::after_millis(DEBOUNCE_MS).await;
        if pb0_pc.is_low() {
            let t0 = embassy_time::Instant::now();
            while pb0_pc.is_low() {
                if t0.elapsed().as_millis() as u64 >= LONG_PRESS_MS_PC { let _ = tx.send(ButtonsEvent::PcLong).await; while pb0_pc.is_low() { Timer::after_millis(10).await; } continue; }
                Timer::after_millis(10).await;
            }
            let _ = tx.send(ButtonsEvent::PcShort).await;
        }
        pa9_pol.wait_for_falling_edge().await;
        Timer::after_millis(DEBOUNCE_MS).await;
        if pa9_pol.is_low() {
            let t0 = embassy_time::Instant::now();
            while pa9_pol.is_low() {
                if t0.elapsed().as_millis() as u64 >= 800 { let _ = tx.send(ButtonsEvent::PolarityLong).await; while pa9_pol.is_low() { Timer::after_millis(10).await; } continue; }
                Timer::after_millis(10).await;
            }
            let _ = tx.send(ButtonsEvent::PolarityShort).await;
        }
        pa12_freq.wait_for_falling_edge().await;
        Timer::after_millis(DEBOUNCE_MS).await;
        if pa12_freq.is_low() {
            let t0 = embassy_time::Instant::now();
            while pa12_freq.is_low() {
                if t0.elapsed().as_millis() as u64 >= LONG_PRESS_MS_FREQ { let _ = tx.send(ButtonsEvent::FreqLong).await; while pa12_freq.is_low() { Timer::after_millis(10).await; } continue; }
                Timer::after_millis(10).await;
            }
            let _ = tx.send(ButtonsEvent::FreqShort).await;
        }
    }
}
