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
    fn process(&mut self, input: f32, a1: f32, a2: f32, b0: f32, b1: f32, b2: f32) -> f32 {
        let out = b0 * input + self.z1;
        self.z1 = b1 * input - a1 * out + self.z2;
        self.z2 = b2 * input - a2 * out;
        // デノーマル対策
        if out.abs() < 1e-12 {
            self.z1 = 0.0;
            self.z2 = 0.0;
            return 0.0;
        }
        out
    }
}

pub struct GainProcessor {
    pub params: Arc<XrossBassAmpParams>,
    pre_lp: f32,
    pre_hp: f32,
    dc_block: f32,
    envelope: f32,
    prev_input: f32,
    feedback_state: f32,
    os_lpf_biquad: Biquad,
    sample_rate: f32,
}

impl GainProcessor {
    pub fn new(params: Arc<XrossBassAmpParams>) -> Self {
        Self {
            params,
            pre_lp: 0.0,
            pre_hp: 0.0,
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
        self.pre_lp = 0.0;
        self.pre_hp = 0.0;
        self.dc_block = 0.0;
        self.envelope = 0.0;
        self.prev_input = 0.0;
        self.feedback_state = 0.0;
        self.os_lpf_biquad = Biquad::new();
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let drive = self.params.drive.value();
        let blend = self.params.blend.value(); // モダンベースに必須のBlend
        let compression = self.params.compression.value();
        let sag = self.params.sag.value();

        // 1. Input Conditioning & Envelope
        let in_signal = input * 1.2;
        self.envelope += (in_signal.abs() - self.envelope) * 0.15;

        // 2. Dynamic Compression (Darkglass風の入力段コンプ)
        let comp_gain = 1.0 / (1.0 + self.envelope * compression * 5.0);
        let compressed_in = in_signal * comp_gain;

        // 3. Sag (パワーアンプのへたりをシミュレート)
        let dynamic_gain = 1.0 - (self.envelope * sag * 0.3).min(0.4);

        // 4. Oversampling Loop (4x)
        let os_factor = 4;
        let mut output_sum = 0.0;
        let inv_os = 1.0 / os_factor as f32;

        for i in 0..os_factor {
            let fraction = i as f32 * inv_os;
            let sub_sample =
                (self.prev_input + (compressed_in - self.prev_input) * fraction) * dynamic_gain;

            // 歪み成分のみを生成
            let distorted = self.drive_core(sub_sample, drive);

            // 5. Parallel Blend (モダンベースの肝: クリーンを混ぜる)
            // 低域が痩せないように、歪んだ信号と入力をパラレルミックス
            output_sum += (distorted * blend) + (sub_sample * (1.0 - blend));
        }
        self.prev_input = compressed_in;

        let raw_out = output_sum * inv_os;

        // 6. Anti-aliasing LPF (15kHz以上をカットしてデジタル感を消す)
        let (a1, a2, b0, b1, b2) = self.calculate_biquad_lpf(15000.0);
        let filtered_out = self.os_lpf_biquad.process(raw_out, a1, a2, b0, b1, b2);

        // 7. DC Block & Master Gain
        let dc_fix = filtered_out - self.dc_block;
        self.dc_block = filtered_out + 0.996 * (self.dc_block - filtered_out);

        let master_gain = 10.0f32.powf(self.params.master_gain.value() / 20.0);
        dc_fix * master_gain
    }

    #[inline]
    fn drive_core(&mut self, input: f32, drive: f32) -> f32 {
        // --- PRE-FILTERING (Darkglass的モデリング) ---
        // ベースの歪みは「中域から上」だけを狙うのが鉄則。
        // Pre-HPで低域の濁りを除去。Tightパラメータを連動させる。
        let tight = self.params.tight.value() / 500.0; // 0.0~1.0
        let hp_cutoff = 0.02 + tight * 0.1;
        self.pre_hp += hp_cutoff * (input - self.pre_hp);
        let mut x = input - self.pre_hp;

        // --- DISTORTION STAGING ---
        let drive_db = drive * 40.0; // 最大40dBブースト
        let gain = 10.0f32.powf(drive_db / 20.0);

        // フィードバックを介した粘りのあるサチュレーション
        x = (x + self.feedback_state * 0.15) * gain;

        // Asymmetric Soft Clipping (真空管+ダイアモンドクリッパ混成風)
        let side_chain = if x > 0.0 {
            (x * 0.8).tanh()
        } else {
            (x * 1.1).tanh() * 0.95
        };

        // --- ATTACK SHAPING (Gritty texture) ---
        // 高域を少し強調して、モダンベース特有の「カチカチ」感を出す
        let attack = self.params.presence.value() / 18.0; // 0~1
        let mut distorted = side_chain;
        if attack > 0.0 {
            let diff = distorted - self.pre_lp;
            distorted += diff * attack * 0.5; // 擬似的な高域ブースト
        }
        self.pre_lp += 0.4 * (distorted - self.pre_lp);

        self.feedback_state = distorted;
        distorted
    }

    fn calculate_biquad_lpf(&self, cutoff: f32) -> (f32, f32, f32, f32, f32) {
        let ff = (cutoff / self.sample_rate).min(0.45);
        let omega = 2.0 * PI * ff;
        let sn = omega.sin();
        let cs = omega.cos();
        let alpha = sn / (2.0f32).sqrt();
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
