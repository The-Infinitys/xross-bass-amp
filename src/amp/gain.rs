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
        // 1. セーフティチェック (NaN/Inf対策)
        input.iter_mut().for_each(|i| {
            if !i.is_finite() {
                *i = 0.0;
            }
        });

        // 2. プリ・ノイズゲート
        self.noise_gate.pre_process(input);

        // 3. インプットゲイン適用
        let input_factor = 10.0f32.powf(self.params.input_gain.value() / 20.0);
        input.iter_mut().for_each(|i| *i *= input_factor);

        // 4. メイン歪みプロセッサ (DarkDistortion)
        // params.rs の各値を DarkDistortion の引数にマッピング
        let drive = self.params.gain.value();
        let grit = self.params.grit.value();
        let sag = self.params.low_comp.value();
        let tight = self.params.tight.value();
        let focus = self.params.focus.value();
        let attack = self.params.attack.value();

        // Style/EQ系 (本来はEQ Processorで行うが、歪みの特性として渡す)
        let s_low = self.params.eq_low.value() / 18.0;
        let s_mid = self.params.eq_mid.value() / 18.0;
        let s_high = self.params.eq_high.value() / 18.0;

        self.metal.process_slice(
            input, drive, grit, sag, tight, focus, attack, s_low, s_mid, s_high,
        );

        // 5. ポスト・ノイズゲート
        self.noise_gate.post_process(input);

        // 6. マスターゲイン適用
        let master_factor = 10.0f32.powf(self.params.master_gain.value() / 20.0);
        input.iter_mut().for_each(|i| *i *= master_factor);
    }
}
