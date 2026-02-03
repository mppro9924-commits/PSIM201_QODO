use defmt::*;
use embassy_time::Timer;
use embassy_sync::channel::mpmc::Channel;
use crate::drivers::mcp23017::Mcp23017;
use crate::dac_control::DacCmd;
use crate::frequency_control::FrequencyCmd;

const GPB0: u8 = 1 << 0;
const GPB1: u8 = 1 << 1;
const GPB2: u8 = 1 << 2;
const GPB3: u8 = 1 << 3;
const GPB4: u8 = 1 << 4; // +Step_vtg relay
const GPB5: u8 = 1 << 5; // CTGP_RELAY
const GPB6: u8 = 1 << 6; // HV_ON
const GPB7: u8 = 1 << 7; // Cin_RLY1
const GPA6: u8 = 1 << 6; // -Step_vtg relay
const GPA7: u8 = 1 << 7; // Cin_RLY2

#[derive(Copy, Clone, Debug, defmt::Format, PartialEq)]
pub enum Polarity { Positive, Negative }

#[derive(Copy, Clone, Debug, defmt::Format)]
pub enum HvState { Off, Discharging, WaitingForDischarge, PreSetting, Completing, Toggling, Restoring, Running }

#[derive(Copy, Clone, Debug, defmt::Format)]
pub enum HvCommand { RequestPolarityToggle, ForceStop }

pub struct HvController<'d> {
    exp: Mcp23017<'d>,
    state: HvState,
    pol: Polarity,
    gpa: u8,
    gpb: u8,
}

impl<'d> HvController<'d> {
    pub fn new(exp: Mcp23017<'d>) -> Self { Self { exp, state: HvState::Off, pol: Polarity::Positive, gpa: 0, gpb: 0 } }
    async fn apply(&mut self) -> Result<(), ()> { self.exp.write_gpa(self.gpa).await?; self.exp.write_gpb(self.gpb).await }
    async fn set_polarity(&mut self, pol: Polarity) -> Result<(), ()> {
        self.gpb &= !(GPB0|GPB1|GPB2|GPB3);
        match pol {
            Polarity::Positive => { self.gpb |= GPB0 | GPB3; }
            Polarity::Negative => { self.gpb |= GPB1 | GPB2; }
        }
        let b0 = (self.gpb & GPB0)!=0; let b1=(self.gpb & GPB1)!=0; let b2=(self.gpb & GPB2)!=0; let b3=(self.gpb & GPB3)!=0;
        assert!((b0 as u8) ^ (b1 as u8) == 1);
        assert!((b2 as u8) ^ (b3 as u8) == 1);
        self.apply().await?; self.pol = pol; Ok(())
    }
    async fn hv_on_update(&mut self, f: u32) -> Result<(), ()> { if f>0 { self.gpb |= GPB6; } else { self.gpb &= !GPB6; } self.exp.write_gpb(self.gpb).await }
}

#[embassy_executor::task]
pub async fn hv_task<'d>(
    expander: Mcp23017<'d>,
    dac_tx: Channel<DacCmd, 8>::Sender,
    freq_tx: Channel<FrequencyCmd, 8>::Sender,
    mut rx: Channel<HvCommand, 8>::Receiver,
) {
    let mut hv = HvController::new(expander);
    let mut freq: u32 = 0;
    loop {
        let cmd = rx.receive().await;
        match cmd {
            HvCommand::ForceStop => {
                info!("HV ForceStop");
                let _ = dac_tx.send(DacCmd::SetHvVolts(0.0)).await;
                let _ = freq_tx.send(FrequencyCmd::SetFrequency(0)).await;
                let _ = hv.hv_on_update(0).await;
                hv.state = HvState::Off;
            }
            HvCommand::RequestPolarityToggle => {
                info!("HV polarity toggle start");
                let _ = dac_tx.send(DacCmd::SetHvVolts(0.0)).await;
                let _ = freq_tx.send(FrequencyCmd::SetFrequency(0)).await;
                let _ = hv.hv_on_update(0).await;
                hv.state = HvState::Discharging;
                hv.state = HvState::WaitingForDischarge;
                Timer::after_millis(2150).await; // mandatory hold
                hv.state = HvState::PreSetting;
                // set new polarity
                let new_pol = if hv.pol==Polarity::Positive { Polarity::Negative } else { Polarity::Positive };
                let _ = hv.set_polarity(new_pol).await;
                Timer::after_millis(1).await; hv.state = HvState::Completing;
                hv.gpb |= GPB4 | GPB5; let _ = hv.apply().await; Timer::after_millis(1).await; hv.state = HvState::Toggling;
                hv.gpb &= !(GPB4 | GPB5); let _ = hv.apply().await; Timer::after_millis(1).await; hv.state = HvState::Restoring;
                Timer::after_millis(100).await; hv.state = HvState::Running;
                info!("HV polarity switch complete");
            }
        }
    }
}
