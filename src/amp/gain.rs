use crate::params::XrossBassAmpParams;
use std::f32::consts::PI;
use std::sync::Arc;

struct StableFilter {
    s: f32,
}

impl StableFilter {
    fn new() -> Self {
        Self { s: 0.0 }
    }
    fn process_lp(&mut self, input: f32, g: f32) -> f32 {
        let v = (input - self.s) * g / (1.0 + g);
        let res = v + self.s;
        self.s = res + v;
        res
    }
    fn process_hp(&mut self, input: f32, g: f32) -> f32 {
        input - self.process_lp(input, g)
    }
    fn reset(&mut self) {
        self.s = 0.0;
    }
}

pub struct GainProcessor {
    pub params: Arc<XrossBassAmpParams>,
    sample_rate: f32,

    input_dc_block: StableFilter,
    output_dc_block: StableFilter,
    crossover_lpf: StableFilter,
    pre_drive_hp: StableFilter,
    post_drive_lpf: StableFilter, // 高域の「サー」を抑えるフィルタ

    comp_env: f32,
    gate_env: f32, // ノイズゲート用エンベロープ
    last_dirty_out: f32,
}

impl GainProcessor {
    pub fn new(params: Arc<XrossBassAmpParams>) -> Self {
        Self {
            params,
            sample_rate: 44100.0,
            input_dc_block: StableFilter::new(),
            output_dc_block: StableFilter::new(),
            crossover_lpf: StableFilter::new(),
            pre_drive_hp: StableFilter::new(),
            post_drive_lpf: StableFilter::new(),
            comp_env: 0.0,
            gate_env: 0.0,
            last_dirty_out: 0.0,
        }
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.reset();
    }

    pub fn reset(&mut self) {
        self.input_dc_block.reset();
        self.output_dc_block.reset();
        self.crossover_lpf.reset();
        self.pre_drive_hp.reset();
        self.post_drive_lpf.reset();
        self.comp_env = 0.0;
        self.gate_env = 0.0;
        self.last_dirty_out = 0.0;
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let (in_db, master_db, drive, grind, blend) = {
            let g = &self.params.gain_section;
            (
                g.input_gain.value(),
                g.master_gain.value(),
                g.drive.value(),
                g.grind.value(),
                g.blend.value(),
            )
        };
        let (tight, comp_amt, gate_amt) = {
            let fx = &self.params.fx_section;
            (
                fx.tight.value(),
                fx.compressor.value(),
                fx.noise_gate.value(),
            )
        };

        // 1. DC Block & Input Gain
        let g_dc = (PI * 10.0 / self.sample_rate).tan();
        let mut sig = self.input_dc_block.process_hp(input, g_dc);
        sig *= 10.0f32.powf(in_db / 20.0);

        // 2. Noise Gate (歪み増幅の前に適用するのが最も効果的)
        sig = self.apply_gate(sig, gate_amt);

        // 3. Compressor
        sig = self.apply_compression(sig, comp_amt);

        // 4. Parallel Crossover (250Hz)
        let g_cross = (PI * 250.0 / self.sample_rate).tan();
        let clean_path = self.crossover_lpf.process_lp(sig, g_cross);
        let dirty_input = sig - clean_path;

        // 5. Drive Section
        let mut dirty_path = self.drive_core(dirty_input, drive, grind, tight);

        // 歪み後の高域ノイズカット (8kHz以上の「サー」を緩和)
        let g_post = (PI * 8000.0 / self.sample_rate).tan();
        dirty_path = self.post_drive_lpf.process_lp(dirty_path, g_post);

        // 6. Blend & Master
        let mixed = clean_path + (dirty_path * blend * 2.0);
        let out = self.output_dc_block.process_hp(mixed, g_dc);

        (out * 10.0f32.powf(master_db / 20.0)).clamp(-1.0, 1.0)
    }

    fn apply_gate(&mut self, sig: f32, amount: f32) -> f32 {
        if amount < 0.01 {
            return sig;
        }

        // ゲートの閾値を設定
        let threshold = 0.001 * (amount * 10.0).exp();
        let abs_sig = sig.abs();

        // 非常に速いアタックと、少し遅めのリリース
        let att = (-1.0 / (0.001 * self.sample_rate)).exp();
        let rel = (-1.0 / (0.050 * self.sample_rate)).exp();
        let coeff = if abs_sig > self.gate_env { att } else { rel };
        self.gate_env = abs_sig + coeff * (self.gate_env - abs_sig);

        if self.gate_env < threshold {
            // 急激に消すとプチプチいうので、スムーズに減衰させる
            let gain = (self.gate_env / threshold).powi(2);
            sig * gain
        } else {
            sig
        }
    }

    fn apply_compression(&mut self, sig: f32, amount: f32) -> f32 {
        if amount < 0.01 {
            return sig;
        }
        let threshold = 0.12;
        let ratio = 1.0 + amount * 12.0;
        let abs_sig = sig.abs();

        let att = (-1.0 / (0.002 * self.sample_rate)).exp();
        let rel = (-1.0 / (0.080 * self.sample_rate)).exp();
        let coeff = if abs_sig > self.comp_env { att } else { rel };
        self.comp_env = abs_sig + coeff * (self.comp_env - abs_sig);

        if self.comp_env > threshold {
            let gr = (self.comp_env / threshold).powf((1.0 / ratio) - 1.0);
            sig * gr * (1.0 + amount * 0.4)
        } else {
            sig
        }
    }

    fn drive_core(&mut self, input: f32, drive: f32, grind: f32, tight: f32) -> f32 {
        let f_tight = 80.0 + tight * 420.0;
        let g_tight = (PI * f_tight / self.sample_rate).tan();
        let mut x = self.pre_drive_hp.process_hp(input, g_tight);

        // Drive Gain (2xオーバーサンプリングなしでも安定するように調整)
        let gain = 1.0 + drive * 60.0;
        x *= gain;

        // Grind: 歪みの前にハイミッドを強調
        x = x * (1.0 - grind * 0.4) + (x * 4.0 * grind).tanh();

        // 飽和処理
        let mut saturated = (x * 1.3).atan() * 0.7;
        saturated += self.last_dirty_out * 0.05; // 控えめなフィードバック

        if saturated > 0.0 {
            saturated = (saturated * 1.1).tanh();
        }

        self.last_dirty_out = saturated;
        saturated
    }
}
