use crate::params::XrossBassAmpParams;
use std::f32::consts::PI;
use std::sync::Arc;

struct Biquad {
    z1: f32,
    z2: f32,
}

impl Biquad {
    fn new() -> Self {
        Self { z1: 0.0, z2: 0.0 }
    }

    #[inline]
    fn process(&mut self, input: f32, coeffs: &(f32, f32, f32, f32, f32)) -> f32 {
        let (a1, a2, b0, b1, b2) = *coeffs;
        let out = b0 * input + self.z1;
        self.z1 = b1 * input - a1 * out + self.z2;
        self.z2 = b2 * input - a2 * out;

        // デノーマル対策 (簡易版)
        if out.abs() < 1e-18 {
            self.z1 = 0.0;
            self.z2 = 0.0;
            return 0.0;
        }
        out
    }
}

pub struct GainProcessor {
    pub params: Arc<XrossBassAmpParams>,
    // Filters for Darkglass-style "Grunt" and "Attack"
    grunt_lp: f32,
    attack_hp: f32,
    pre_drive_hp: f32,

    dc_block: f32,
    envelope: f32,
    prev_input: f32,
    feedback_state: f32,

    // Anti-aliasing
    os_lpf_biquad: Biquad,
    sample_rate: f32,
}

impl GainProcessor {
    pub fn new(params: Arc<XrossBassAmpParams>) -> Self {
        Self {
            params,
            grunt_lp: 0.0,
            attack_hp: 0.0,
            pre_drive_hp: 0.0,
            dc_block: 0.0,
            envelope: 0.0,
            prev_input: 0.0,
            feedback_state: 0.0,
            os_lpf_biquad: Biquad::new(),
            sample_rate: 44100.0,
        }
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.reset();
    }

    pub fn reset(&mut self) {
        self.grunt_lp = 0.0;
        self.attack_hp = 0.0;
        self.pre_drive_hp = 0.0;
        self.dc_block = 0.0;
        self.envelope = 0.0;
        self.prev_input = 0.0;
        self.feedback_state = 0.0;
        self.os_lpf_biquad = Biquad::new();
    }

    pub fn process(&mut self, input: f32) -> f32 {
        // パラメータ取得
        let drive = self.params.drive.value();
        let blend = self.params.blend.value();
        let compression = self.params.compression.value();
        let sag = self.params.sag.value();
        let attack_boost = self.params.presence.value(); // Attack switch相当
        let tight = self.params.tight.value(); // Grunt switch相当

        // 1. Envelope Detection (コンプとサグ用)
        let abs_in = input.abs();
        let env_target = if abs_in > self.envelope { 0.5 } else { 0.05 }; // Fast Attack, Slow Release
        self.envelope += (abs_in - self.envelope) * env_target;

        // 2. Input Gain & Compression (Darkglass特有のパキパキ感)
        let comp_threshold = 0.2;
        let comp_ratio = 1.0 + (compression * 4.0);
        let mut dry_signal = input * 1.5; // 入力を少し突っ込む

        if self.envelope > comp_threshold {
            let gain_reduction = (self.envelope / comp_threshold).powf(1.0 / comp_ratio - 1.0);
            dry_signal *= gain_reduction;
        }

        // 3. Oversampling Loop (4x)
        let os_factor = 4;
        let mut output_sum = 0.0;
        let inv_os = 1.0 / os_factor as f32;

        // パワーアンプのへたり（Sag）計算
        let dynamic_gain = 1.0 - (self.envelope * sag * 0.4).min(0.5);

        for i in 0..os_factor {
            let fraction = i as f32 * inv_os;
            let sub_sample = self.prev_input + (dry_signal - self.prev_input) * fraction;

            // --- Drive Core Path ---
            let distorted = self.drive_core(sub_sample, drive, attack_boost, tight) * dynamic_gain;

            // --- Parallel Blend ---
            // Darkglassの肝：低域はクリーンな信号を維持し、高域の歪みを混ぜる
            // ここでは簡易的にfull-range cleanとfull-range distortedをblend
            output_sum += (distorted * blend) + (sub_sample * (1.0 - blend));
        }
        self.prev_input = dry_signal;

        let raw_out = output_sum * inv_os;

        // 4. Post-Filter (Cabinets/Airy feeling)
        let lpf_coeffs = self.calculate_biquad_lpf(14000.0);
        let filtered_out = self.os_lpf_biquad.process(raw_out, &lpf_coeffs);

        // 5. DC Block & Master
        let dc_fix = filtered_out - self.dc_block;
        self.dc_block = filtered_out + 0.996 * (self.dc_block - filtered_out);

        let master_gain = 10.0f32.powf(self.params.master_gain.value() / 20.0);
        dc_fix * master_gain
    }

    #[inline]
    fn drive_core(&mut self, input: f32, drive: f32, attack: f32, grunt: f32) -> f32 {
        // --- PRE-FILTERING (Darkglass DNA) ---

        // 1. Grunt (Tightness): 低域をどれだけ歪み回路に送るか
        let hp_freq = 0.01 + (grunt * 0.08); // 100Hz ~ 500Hzをカット
        self.pre_drive_hp += hp_freq * (input - self.pre_drive_hp);
        let mut x = input - self.pre_drive_hp;

        // 2. Attack (Clarity): 3kHz付近をブーストしてカチカチ感を出す
        let attack_hp_freq = 0.15; // 高域強調用のカットオフ
        self.attack_hp += attack_hp_freq * (x - self.attack_hp);
        x += (x - self.attack_hp) * attack * 2.0;

        // --- DISTORTION ---
        let gain = 10.0f32.powf((drive * 45.0) / 20.0);
        x *= gain;

        // 微小なフィードバックによる非線形挙動の付加
        x += self.feedback_state * 0.1;

        // CMOS Soft Clipping Simulation (B3Kの心臓部)
        // atanやtanhよりも、少し「角」がある多項式クリッパーを使用
        let distorted = if x > 1.2 {
            1.0 // Hard ceiling
        } else if x < -1.2 {
            -1.0
        } else {
            // Asymmetric soft clipping
            if x > 0.0 {
                x * (1.0 - 0.2 * x)
            } else {
                x * (1.0 + 0.3 * x)
            }
        }
        .tanh(); // 最終的な平滑化

        self.feedback_state = distorted;

        // 低域の補償（歪ませたあとに少し太さを戻す）
        self.grunt_lp += 0.1 * (distorted - self.grunt_lp);
        distorted + self.grunt_lp * 0.2
    }

    fn calculate_biquad_lpf(&self, cutoff: f32) -> (f32, f32, f32, f32, f32) {
        let ff = (cutoff / self.sample_rate).min(0.48);
        let omega = 2.0 * PI * ff;
        let sn = omega.sin();
        let cs = omega.cos();
        let alpha = sn / (2.0f32).sqrt(); // Q = 0.707
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
