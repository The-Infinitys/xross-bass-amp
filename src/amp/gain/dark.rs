use std::f32::consts::PI;

#[derive(Default, Clone, Copy)]
pub struct Biquad {
    z1: f32,
    z2: f32,
}

impl Biquad {
    #[inline(always)]
    fn process(&mut self, input: f32, a1: f32, a2: f32, b0: f32, b1: f32, b2: f32) -> f32 {
        let out = b0 * input + self.z1;
        self.z1 = b1 * input - a1 * out + self.z2;
        self.z2 = b2 * input - a2 * out;
        out
    }
}

pub struct DarkDistortion {
    pre_hp: f32,
    slew_state: f32,
    dc_block: f32,
    envelope: f32,
    prev_input: f32,
    low_resonance: f32,
    post_tight: f32,
    feedback_state: f32,
    os_lpf_biquad: Biquad,
    sample_rate: f32,
}

impl DarkDistortion {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            pre_hp: 0.0,
            slew_state: 0.0,
            dc_block: 0.0,
            envelope: 0.0,
            prev_input: 0.0,
            low_resonance: 0.0,
            post_tight: 0.0,
            feedback_state: 0.0,
            os_lpf_biquad: Biquad::default(),
            sample_rate,
        }
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    #[inline(always)]
    fn drive_core(
        &mut self,
        input: f32,
        drive: f32,
        dist: f32,
        sag: f32,
        tight: f32,
        focus: f32,
        attack: f32,
        s_low: f32,
        s_mid: f32,
        s_high: f32,
    ) -> f32 {
        if drive <= 0.01 {
            return input;
        }

        // 1. PRE-FILTERING (Tightness & Character)
        // 低域の飽和を防ぎ、解像度を保つ
        let tight_norm = (tight * 2.0 * PI / self.sample_rate).clamp(0.005, 0.8);
        self.pre_hp += tight_norm * (input - self.pre_hp);
        let mut x = input - self.pre_hp;

        // Focus (Mid Clarity): Boost around 600Hz before distortion
        let focus_boost = focus * 12.0;
        let focus_gain = 10.0f32.powf(focus_boost / 20.0);
        // シンプルな簡易フィルタ
        x *= 1.0 + (focus_gain - 1.0) * 0.5;

        // Attack (Picking Definition): Boost around 2.8kHz before distortion
        let attack_boost = attack * 15.0;
        let attack_gain = 10.0f32.powf(attack_boost / 20.0);
        x *= 1.0 + (attack_gain - 1.0) * 0.3;

        // 2. GAIN STAGING & SAG (Low Comp)
        let sag_val = 1.0 - (self.envelope * sag * 0.5);
        let drive_gain = 1.0 + drive.powf(2.0) * 80.0;
        x *= drive_gain * sag_val;

        // 3. ASYMMETRIC SATURATION
        let fb_amount = 0.05 + dist * 0.25;
        let mut sig = x + (self.feedback_state * fb_amount);

        let asymmetry = 0.1 * dist; // 歪ませるほど非対称にし、倍音を増やす
        let drive_factor = 1.5 + dist * 2.5;

        if sig > 0.0 {
            sig = (sig * drive_factor).tanh();
        } else {
            let n_drive = drive_factor * (1.0 - asymmetry);
            sig = (sig * n_drive).tanh() * (1.0 - asymmetry);
        }
        self.feedback_state = sig;

        // 4. CHARACTER EQ (Scoop & Resonance)
        // Mid Scoop: メタル的な質感を付与
        let mid_scoop = (-s_mid).max(0.0) * 0.5;
        sig -= (sig - sig.powi(3)) * mid_scoop;

        // Low Resonance: 低域の重みを強調
        let low_boost = s_low.max(0.0) * 0.4;
        self.low_resonance += 0.1 * (sig - self.low_resonance);
        sig += self.low_resonance * low_boost;

        // 5. POST-PROCESSING (High-cut & Slew)
        let post_cutoff =
            (4000.0 + s_high * 8000.0 + (1.0 - drive) * 4000.0) * 2.0 * PI / self.sample_rate;
        self.post_tight += post_cutoff.clamp(0.01, 0.9) * (sig - self.post_tight);
        sig = self.post_tight;

        // Slew Rate: 物理的な回路の「鈍さ」をシミュレートして高域のトゲを取る
        let max_step = 0.05 + (1.0 - drive) * 0.5;
        let diff = sig - self.slew_state;
        self.slew_state += diff.clamp(-max_step, max_step);

        self.slew_state
    }

    pub fn process_sample(
        &mut self,
        input: f32,
        drive: f32,
        dist: f32,
        sag: f32,
        tight: f32,
        focus: f32,
        attack: f32,
        s_low: f32,
        s_mid: f32,
        s_high: f32,
    ) -> f32 {
        // オーバーサンプリング (2倍)
        let mut output_sum = 0.0;
        self.envelope += (input.abs() - self.envelope) * 0.05;

        for i in 0..2 {
            let fraction = i as f32 * 0.5;
            let sub_sample = self.prev_input + (input - self.prev_input) * fraction;
            output_sum += self.drive_core(
                sub_sample, drive, dist, sag, tight, focus, attack, s_low, s_mid, s_high,
            );
        }
        self.prev_input = input;

        let raw_out = output_sum * 0.5;

        // エイリアシング除去 LPF
        let (a1, a2, b0, b1, b2) = Self::calculate_biquad_lpf(self.sample_rate, 18000.0);
        let filtered_out = self.os_lpf_biquad.process(raw_out, a1, a2, b0, b1, b2);

        // DC Block
        let dc_fix = filtered_out - self.dc_block;
        self.dc_block = filtered_out + 0.995 * (self.dc_block - filtered_out);

        dc_fix * 0.5 // 最終音量調整
    }

    pub fn process_slice(
        &mut self,
        slice: &mut [f32],
        drive: f32,
        dist: f32,
        sag: f32,
        tight: f32,
        focus: f32,
        attack: f32,
        s_low: f32,
        s_mid: f32,
        s_high: f32,
    ) {
        for sample in slice.iter_mut() {
            *sample = self.process_sample(
                *sample, drive, dist, sag, tight, focus, attack, s_low, s_mid, s_high,
            );
        }
    }

    fn calculate_biquad_lpf(sample_rate: f32, cutoff: f32) -> (f32, f32, f32, f32, f32) {
        let omega = 2.0 * PI * (cutoff / sample_rate).min(0.49);
        let sn = omega.sin();
        let cs = omega.cos();
        let alpha = sn / (2.0 * 0.707);
        let a0 = 1.0 + alpha;
        (
            -2.0 * cs / a0,
            (1.0 - alpha) / a0,
            (1.0 - cs) / 2.0 / a0,
            (1.0 - cs) / a0,
            (1.0 - cs) / 2.0 / a0,
        )
    }
}
