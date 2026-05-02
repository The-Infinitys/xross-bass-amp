use crate::params::XrossBassAmpParams;
use std::sync::Arc;
mod dark;
use dark::DarkDistortion;
mod noise_gate;
use noise_gate::AutoNoiseGate;

pub struct GainProcessor {
    pub params: Arc<XrossBassAmpParams>,
    metal: DarkDistortion,
    noise_gate: AutoNoiseGate,
    sample_rate: f32,
}

impl GainProcessor {
    pub fn new(params: Arc<XrossBassAmpParams>) -> Self {
        Self {
            params,
            metal: DarkDistortion::new(44100.0),
            noise_gate: AutoNoiseGate::new(44100.0),
            sample_rate: 44100.0,
        }
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.metal.initialize(sample_rate);
        self.noise_gate.initialize(sample_rate);
    }

    pub fn process(&mut self, input: &mut [f32]) {
        // 1. セーフティチェック
        input.iter_mut().for_each(|i| {
            if !i.is_finite() {
                *i = 0.0;
            }
        });

        // 2. プリ・ノイズゲート (歪ませる前の入力を監視)
        self.noise_gate.pre_process(input);

        // 3. インプットゲイン
        let input_factor = 10.0f32.powf(self.params.input_gain.value() / 20.0);
        if input_factor != 1.0 {
            input.iter_mut().for_each(|i| *i *= input_factor);
        }

        // 4. メイン歪み処理
        // DarkDistortion内部でparamsを直接参照
        for sample in input.iter_mut() {
            *sample = self.metal.process_sample(*sample, &self.params);
        }

        // 5. ポスト・ノイズゲート (歪み後のノイズをカット)
        self.noise_gate.post_process(input);

        // 6. マスターゲイン
        let master_factor = 10.0f32.powf(self.params.master_gain.value() / 20.0);
        if master_factor != 1.0 {
            input.iter_mut().for_each(|i| *i *= master_factor);
        }
    }
}
