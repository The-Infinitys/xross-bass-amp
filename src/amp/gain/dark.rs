use crate::params::XrossBassAmpParams;
use std::f32::consts::PI;
use std::sync::Arc;

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
    fn drive_core(&mut self, input: f32, params: &Arc<XrossBassAmpParams>) -> f32 {
        let drive = params.gain.value();
        if drive <= 0.001 {
            return input;
        }

        // 1. PRE-FILTERING (Tight & Focus)
        // Tight: ローエンドのダブつきを抑える
        let tight_hz = params.tight.value();
        let tight_norm = (tight_hz * 2.0 * PI / self.sample_rate).clamp(0.005, 0.8);
        self.pre_hp += tight_norm * (input - self.pre_hp);
        let mut x = input - self.pre_hp;

        // Focus: 中音域の押し出し。歪みの食いつきを良くする。
        let focus = params.focus.value();
        let focus_gain = 10.0f32.powf((focus * 10.0) / 20.0);
        x *= focus_gain;

        // Attack: ピッキング時の高域成分。ジャリッとしたエッジを作る。
        let attack = params.attack.value();
        let attack_gain = 10.0f32.powf((attack * 12.0) / 20.0);
        x *= 1.0 + (attack_gain - 1.0) * 0.4;

        // 2. GAIN STAGING & DYNAMIC SAG
        // Low Comp: 低域の包絡線に応じてゲインを抑え、コンプレッション感を出す。
        let low_comp = params.low_comp.value();
        let sag_val = 1.0 - (self.envelope * low_comp * 0.6);
        let grit = params.grit.value();
        let drive_gain = 1.0 + drive.powf(2.0) * 60.0 * (1.0 + grit);
        x *= drive_gain * sag_val;

        // 3. NON-LINEAR SATURATION (Asymmetric)
        let dist = params.grit.value();
        let fb_amount = 0.02 + dist * 0.2;
        let mut sig = x + (self.feedback_state * fb_amount);

        let asymmetry = 0.15 * dist;
        let drive_factor = 2.0 + dist * 3.0;

        if sig > 0.0 {
            sig = (sig * drive_factor).tanh();
        } else {
            let n_drive = drive_factor * (1.0 - asymmetry);
            sig = (sig * n_drive).tanh() * (1.0 - asymmetry);
        }
        self.feedback_state = sig;

        // 4. BASS RESONANCE
        // EQ Lowの値を反映して、歪みの後に重低音のレゾナンスを付加
        let resonance = params.resonance.value();
        let low_boost = (params.eq_low.value().max(0.0) / 18.0) * 0.5 + resonance * 0.2;
        self.low_resonance += 0.1 * (sig - self.low_resonance);
        sig += self.low_resonance * low_boost;

        // 5. POST-FILTERING (Tone Shaping)
        // High-cut: 歪みによる不要な超高域ノイズをカット
        let high_val = params.eq_high.value();
        let post_cutoff =
            (4500.0 + high_val * 100.0 + (1.0 - drive) * 3000.0) * 2.0 * PI / self.sample_rate;
        self.post_tight += post_cutoff.clamp(0.01, 0.95) * (sig - self.post_tight);
        sig = self.post_tight;

        // Slew Rate: 滑らかな歪みの質感を生む
        let max_step = 0.1 + (1.0 - dist) * 0.4;
        let diff = sig - self.slew_state;
        self.slew_state += diff.clamp(-max_step, max_step);

        self.slew_state
    }

    pub fn process_sample(&mut self, input: f32, params: &Arc<XrossBassAmpParams>) -> f32 {
        // エンベロープ・フォロワー (Low Comp用)
        self.envelope += (input.abs() - self.envelope) * 0.05;

        // 2倍オーバーサンプリング
        let mut output_sum = 0.0;
        for i in 0..2 {
            let fraction = i as f32 * 0.5;
            let sub_sample = self.prev_input + (input - self.prev_input) * fraction;
            output_sum += self.drive_core(sub_sample, params);
        }
        self.prev_input = input;

        let raw_out = output_sum * 0.5;

        // アンチエイリアシング LPF (18kHz)
        let (a1, a2, b0, b1, b2) = Self::calculate_biquad_lpf(self.sample_rate, 18000.0);
        let filtered_out = self.os_lpf_biquad.process(raw_out, a1, a2, b0, b1, b2);

        // DC Block
        let dc_fix = filtered_out - self.dc_block;
        self.dc_block = filtered_out + 0.997 * (self.dc_block - filtered_out);

        dc_fix
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
