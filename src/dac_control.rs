use defmt::*;
use embassy_time::Timer;
use embassy_sync::channel::mpmc::Channel;
use embassy_stm32::dac::{Dac, Channel as DacChannel};

#[derive(Copy, Clone, Debug, defmt::Format)]
pub enum DacCmd { SetHvVolts(f32), ShortStep, StartRamp }

pub struct DacController { hv_setpoint_v: f32 }

const HV_MAX_PHASE1_V: f32 = 10.0;
const HV_MIN_V: f32 = 0.0;
const HV_STEP_V: f32 = 0.1;
const DAC_FULL_SCALE_V: f32 = 2.5;

impl DacController {
    pub fn new() -> Self { Self { hv_setpoint_v: 0.0 } }
    fn clamp_phase1(v: f32) -> f32 { v.clamp(HV_MIN_V, HV_MAX_PHASE1_V) }
    fn hv_to_dac_volts(hv: f32) -> f32 { (hv / 300.0) * DAC_FULL_SCALE_V }
    fn hv_to_dac_raw(hv: f32) -> u16 {
        let v_dac = Self::hv_to_dac_volts(hv);
        let code = (v_dac / 3.0 * 4095.0).round();
        code as u16
    }
    fn safe_ramp(current: f32, target: f32) -> f32 {
        let diff = target - current;
        if diff > HV_STEP_V { current + HV_STEP_V }
        else if diff < -HV_STEP_V { current - HV_STEP_V }
        else { target }
    }
}

#[embassy_executor::task]
pub async fn dac_task<'d>(mut dac: Dac<'d, { embassy_stm32::peripherals::DAC::CHANNELS }>, mut rx: Channel<DacCmd, 8>::Receiver) {
    let mut ctrl = DacController::new();
    loop {
        let cmd = rx.receive().await;
        match cmd {
            DacCmd::SetHvVolts(hv) => {
                let target = DacController::clamp_phase1(hv);
                ctrl.hv_setpoint_v = target;
                let code = DacController::hv_to_dac_raw(target);
                dac.set_value(DacChannel::Ch1, code);
                info!("DAC HV setpoint={=f32}V (code {=u16})", target, code);
            }
            DacCmd::ShortStep => {
                let target = DacController::clamp_phase1(ctrl.hv_setpoint_v + HV_STEP_V);
                let code = DacController::hv_to_dac_raw(target);
                ctrl.hv_setpoint_v = target;
                dac.set_value(DacChannel::Ch1, code);
                info!("DAC short step -> {=f32}V", target);
            }
            DacCmd::StartRamp => {
                info!("DAC ramp start");
                for _ in 0..1000 {
                    let target = DacController::clamp_phase1(ctrl.hv_setpoint_v + HV_STEP_V);
                    let stepped = DacController::safe_ramp(ctrl.hv_setpoint_v, target);
                    let code = DacController::hv_to_dac_raw(stepped);
                    ctrl.hv_setpoint_v = stepped;
                    dac.set_value(DacChannel::Ch1, code);
                    Timer::after_millis(500).await;
                    if (ctrl.hv_setpoint_v - HV_MAX_PHASE1_V).abs() < 1e-6 { break; }
                }
            }
        }
    }
}
