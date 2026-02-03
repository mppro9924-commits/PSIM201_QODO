use defmt::*;
use embassy_time::Timer;
use embassy_sync::channel::mpmc::Channel;
use crate::drivers::mcp3424::Mcp3424;
use crate::dac_control::DacCmd;
use crate::hv_control::HvCommand;

const DISCHARGE_THRESH_V: f32 = 0.045_409; // |ADC| < 0.045409V
const OV_WARN_V: f32 = 1.527; // >310V equivalent
const EMERG_SHUT_V: f32 = 1.724; // >350V equivalent

#[embassy_executor::task]
pub async fn safety_task<'d>(
    mut adc: Mcp3424<'d>,
    dac_tx: Channel<DacCmd, 8>::Sender,
    hv_tx: Channel<HvCommand, 8>::Sender,
) {
    loop {
        Timer::after_millis(100).await;
        let ch1 = adc.read_channel_uv(1).await;
        let ch2 = adc.read_channel_uv(2).await;
        if let (Ok(uv1), Ok(uv2)) = (ch1, ch2) {
            let v1 = Mcp3424::uv_to_volts(uv1);
            let v2 = Mcp3424::uv_to_volts(uv2);
            info!("ADC CH1={=f32}V CH2={=f32}V", v1, v2);
            let a1 = v1.abs();
            let a2 = v2.abs();
            if a1 > OV_WARN_V || a2 > OV_WARN_V { warn!("OV warn >310V"); }
            if a1 > EMERG_SHUT_V || a2 > EMERG_SHUT_V {
                error!("Emergency shutdown >350V");
                let _ = dac_tx.send(DacCmd::SetHvVolts(0.0)).await;
                let _ = hv_tx.send(HvCommand::ForceStop).await;
            }
        } else {
            warn!("ADC read error");
        }
    }
}
