use std::sync::Arc;

use crate::params::XrossBassAmpParams;
mod metal;
use metal::MetalDistortion;
mod noise_gate;
use noise_gate::AutoNoiseGate;

pub struct GainProcessor {
    pub params: Arc<XrossBassAmpParams>,
    metal: MetalDistortion,
    noise_gate: AutoNoiseGate,
    sample_rate: f32,
}

impl GainProcessor {
    pub fn new(params: Arc<XrossBassAmpParams>) -> Self {
        Self {
            params,
            metal: MetalDistortion::new(44100.0),
            noise_gate: AutoNoiseGate::new(44100.0),
            sample_rate: 44100.0,
        }
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.metal.initialize(sample_rate);
        self.reset();
    }
    pub fn reset(&mut self) {
        let p = self.params.clone();
        let sr = self.sample_rate;
        *self = Self::new(p);
        self.sample_rate = sr;
    }

    pub fn process(&mut self, input: &mut [f32]) {
        input.iter_mut().for_each(|i| {
            if !i.is_finite() {
                *i = 0.0;
            }
        });
        self.noise_gate.pre_process(input);
        let input_factor = 10.0f32.powf(self.params.input_gain.value() / 20.0);
        input.iter_mut().for_each(|i| {
            *i *= input_factor;
        });

        let drive = self.params.drive.value();
        let dist = self.params.distortion.value();
        let s_low = self.params.style_low.value();
        let s_mid = self.params.style_mid.value();
        let s_high = self.params.style_high.value();
        let sag = self.params.sag.value();
        let tight = self.params.tight.value();
        self.metal
            .process_slice(input, drive, dist, sag, tight, s_low, s_mid, s_high);
        self.noise_gate.post_process(input);
        let master_factor = 10.0f32.powf(self.params.master_gain.value() / 20.0);
        input.iter_mut().for_each(|i| {
            *i *= master_factor;
        });
    }
}
