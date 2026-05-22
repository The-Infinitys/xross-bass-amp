// noise_gate.rs
use crate::modules::filter::{Biquad, FilterType};

pub struct NoiseGate {
    sample_rate: f32,
    envelope: f32,
    gate_gain: f32,

    // 演奏音が通る動的フィルター
    hpf: Biquad,
    lpf: Biquad,

    // [NEW] ベースの低域による誤作動を防ぐサイドチェーン用HPF（検出回路専用）
    sidechain_hpf: Biquad,
}

impl NoiseGate {
    pub fn new(sample_rate: f32) -> Self {
        let mut sidechain_hpf = Biquad::new(sample_rate);
        // 120Hz以下をカットした信号で音量を検出することで、5弦低音のうねりによる誤作動を防ぐ
        sidechain_hpf.set_params(FilterType::HighPass, 120.0, 0.707);

        Self {
            sample_rate,
            envelope: 0.0,
            gate_gain: 1.0, // 初期値は音が出る状態に
            hpf: Biquad::new(sample_rate),
            lpf: Biquad::new(sample_rate),
            sidechain_hpf,
        }
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.hpf.set_sample_rate(sample_rate);
        self.lpf.set_sample_rate(sample_rate);
        self.sidechain_hpf.set_sample_rate(sample_rate);
        self.sidechain_hpf
            .set_params(FilterType::HighPass, 120.0, 0.707);
        self.envelope = 0.0;
        self.gate_gain = 1.0;
    }

    pub fn pre_process(&mut self, buffer: &[f32]) {
        // メタルベースの激しいアタックに追従するため、アタック側の係数は即時（1.0）
        // リリース側の追従を少しだけ滑らかに（0.008）
        let env_coef = 0.008;

        for &sample in buffer {
            // 原音ではなく、120Hz HPFを通した「サイドチェーン信号」でエンベロープを検出
            let sc_sample = self.sidechain_hpf.process(sample);
            let abs = sc_sample.abs();

            if abs > self.envelope {
                self.envelope = abs;
            } else {
                self.envelope += env_coef * (abs - self.envelope);
            }
        }
    }

    pub fn post_process(
        &mut self,
        buffer: &mut [f32],
        attack_ms: f32,
        release_ms: f32,
        hysteresis_db: f32,
        threshold_db: f32,
    ) {
        if buffer.is_empty() {
            return;
        }

        let attack_samples = (attack_ms * self.sample_rate / 1000.0).max(1.0);
        let release_samples = (release_ms * self.sample_rate / 1000.0).max(1.0);

        let atk_max_step = 1.0 / attack_samples;
        let rel_max_step = 1.0 / release_samples;

        let open_thr_db = threshold_db;
        let close_thr_db = threshold_db - hysteresis_db.max(0.1);
        let db_range = open_thr_db - close_thr_db;

        let env_db = if self.envelope > 1e-6 {
            20.0 * self.envelope.log10()
        } else {
            -100.0
        };

        // ターゲットゲインの算出
        let target_gain = if env_db <= close_thr_db {
            0.0
        } else if env_db >= open_thr_db {
            1.0
        } else {
            ((env_db - close_thr_db) / db_range).clamp(0.0, 1.0)
        };

        for sample in buffer.iter_mut() {
            // ゲインのスムージング
            if target_gain > self.gate_gain {
                self.gate_gain = (self.gate_gain + atk_max_step).min(target_gain);
            } else {
                self.gate_gain = (self.gate_gain - rel_max_step).max(target_gain);
            }

            // === 変更点: ベース専用・指数マッピング dynamic filter ===
            // ゲートが閉じる（gate_gain -> 0）につれて：
            // HPFは 35Hz（重低音）から 280Hz（ミッドの濁り成分）へ変化。ベースのローエンドを最後まで守る。
            // LPFは 16000Hz から 3200Hz（メタルベースのクランク・エッジ成分）へ変化。高域のジーというノイズを先に消す。
            let hpf_fc = 35.0 * (280.0 / 35.0f32).powf(1.0 - self.gate_gain);
            let lpf_fc = 16000.0 * (3200.0 / 16000.0f32).powf(1.0 - self.gate_gain);

            self.hpf.set_params(FilterType::HighPass, hpf_fc, 0.707);
            self.lpf.set_params(FilterType::LowPass, lpf_fc, 0.707);

            // フィルター処理
            let filtered = self.lpf.process(self.hpf.process(*sample));

            // ゲート閉鎖時のブレンド比率。
            // 0.35だとメタルベースの低域ノイズが残りすぎる場合があるため、0.25程度に締めてタイトに。
            let wet = (1.0 - self.gate_gain) * 0.25;

            // 最終出力
            *sample = (*sample * self.gate_gain) + (filtered * wet);
        }
    }
}
