use std::f32::consts::PI;

/// 2次フィルタ（Biquad）の状態保持用
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

pub struct MetalDistortion {
    // 内部フィルタ・状態変数
    pre_hp: f32,
    slew_state: f32,
    dc_block: f32,
    envelope: f32,
    prev_input: f32,
    low_resonance: f32,
    post_tight: f32,     // 今回反映：歪み後の動的な高域引き締め
    feedback_state: f32, // 今回反映：サチュレーションのフィードバック
    os_lpf_biquad: Biquad,
    sample_rate: f32,
}

impl MetalDistortion {
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

    /// 信号処理のコアロジック
    #[inline(always)]
    fn drive_core(
        &mut self,
        input: f32,
        drive: f32, // 0.0 - 1.0
        dist: f32,  // 0.0 - 1.0 (Texture / Gain saturation density)
        sag: f32,   // 0.0 - 1.0 (Compression / Power supply dip)
        tight: f32, // 10.0 - 500.0 (Hz) (Pre-HPF cutoff)
        s_low: f32,
        s_mid: f32,
        s_high: f32,
    ) -> f32 {
        // 0. Driveが0なら即座にリターン (完全クリーン)
        if drive <= 0.0 {
            return input;
        }

        // 1. PRE-FILTERING (Input Tightness)
        let tight_norm = (tight * 2.0 * PI / self.sample_rate).clamp(0.001, 0.5);
        self.pre_hp += tight_norm * (input - self.pre_hp);
        let mut x = input - self.pre_hp;

        // 2. GAIN STAGING & SAG
        // 入力信号の大きさに応じてゲインを絞る (Sag)
        let sag_val = 1.0 - (self.envelope * sag * 0.4);
        let drive_gain = if drive < 0.2 {
            drive * 5.0
        } else {
            1.0 + (drive - 0.2).powf(1.5) * 60.0
        };
        x *= drive_gain * sag_val;

        // 3. MULTI-STAGE SATURATION WITH FEEDBACK
        // feedback_state を使って飽和回路の相互作用をシミュレート
        let fb_amount = 0.1 + dist * 0.2;
        let mut sig = x + (self.feedback_state * fb_amount);

        // 非対称性の計算 (SagとDistに連動)
        let asymmetry = 0.06 * dist + (sag * 0.12);
        let drive_factor = 1.2 + dist * 2.0;

        if sig > 0.0 {
            sig = (sig * drive_factor).tanh();
        } else {
            sig = (sig * (drive_factor * (1.0 - asymmetry))).tanh() * (1.0 - asymmetry);
        }

        // フィードバック状態を更新 (次のサンプルへ)
        self.feedback_state = sig;

        // 4. EQUALIZATION & CHARACTER
        // Mid Scoop (メタル特有のドンシャリ感)
        let mid_scoop = (0.5 - s_mid).max(0.0) * 0.9;
        if mid_scoop > 0.0 {
            sig -= (sig - sig.powi(3)) * mid_scoop;
        }

        // Low Resonance (キャビネットの共鳴感)
        let low_boost = s_low * 0.7;
        self.low_resonance += 0.15 * (sig - self.low_resonance);
        sig += self.low_resonance * low_boost;

        // 5. POST-PROCESSING (Post Tight & Slew)
        // post_tight を使った高域の耳障りな成分の除去 (LPF)
        let post_cutoff = 0.1 + (s_high * 0.4) + (1.0 - drive * 0.5) * 0.3;
        self.post_tight += post_cutoff * (sig - self.post_tight);
        sig = self.post_tight;

        // Slew Rate Limiter (物理的な回路の追従限界による滑らかさ)
        let max_step = 0.02 + (dist * 0.4) + (s_high * 0.5) + (1.0 - drive);
        let diff = sig - self.slew_state;
        self.slew_state += diff.clamp(-max_step, max_step);

        // 低ゲイン時のクリーンミックス (整合性)
        if drive < 0.1 {
            let mix = drive * 10.0;
            return input * (1.0 - mix) + self.slew_state * mix;
        }

        self.slew_state
    }

    pub fn process_sample(
        &mut self,
        input: f32,
        drive: f32,
        dist: f32,
        sag: f32,
        tight: f32,
        s_low: f32,
        s_mid: f32,
        s_high: f32,
    ) -> f32 {
        if drive <= 0.0 {
            return input;
        }

        // オーバーサンプリング倍率
        let os_factor = if drive < 0.2 {
            1
        } else if drive < 0.5 {
            2
        } else {
            4
        };
        let inv_os = 1.0 / os_factor as f32;

        // Sag用のエンベロープ追従
        self.envelope += (input.abs() - self.envelope) * 0.1;

        let mut output_sum = 0.0;
        for i in 0..os_factor {
            let fraction = i as f32 * inv_os;
            let sub_sample = self.prev_input + (input - self.prev_input) * fraction;
            output_sum +=
                self.drive_core(sub_sample, drive, dist, sag, tight, s_low, s_mid, s_high);
        }
        self.prev_input = input;

        let raw_out = output_sum * inv_os;

        // 最終段 LPF (エイリアシング除去と音色の最終調整)
        let lpf_hz = 17500.0 - (1.0 - s_high) * 6000.0;
        let (a1, a2, b0, b1, b2) = Self::calculate_biquad_lpf(self.sample_rate, lpf_hz);
        let filtered_out = self.os_lpf_biquad.process(raw_out, a1, a2, b0, b1, b2);

        // DC Block (オフセット除去)
        let dc_fix = filtered_out - self.dc_block;
        self.dc_block = filtered_out + 0.995 * (self.dc_block - filtered_out);
        let volume = 0.4;
        dc_fix * volume
    }

    pub fn process_slice(
        &mut self,
        slice: &mut [f32],
        drive: f32,
        dist: f32,
        sag: f32,
        tight: f32,
        s_low: f32,
        s_mid: f32,
        s_high: f32,
    ) {
        for sample in slice.iter_mut() {
            *sample = self.process_sample(*sample, drive, dist, sag, tight, s_low, s_mid, s_high);
        }
    }

    fn calculate_biquad_lpf(sample_rate: f32, cutoff: f32) -> (f32, f32, f32, f32, f32) {
        let ff = (cutoff / sample_rate).min(0.45);
        let omega = 2.0 * PI * ff;
        let sn = omega.sin();
        let cs = omega.cos();
        let alpha = sn / (2.0f32).sqrt(); // Q = 0.707 (Butterworth)

        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cs / a0;
        let a2 = (1.0 - alpha) / a0;
        let b1 = (1.0 - cs) / a0;
        let b0 = b1 * 0.5;
        let b2 = b0;

        (a1, a2, b0, b1, b2)
    }
}
