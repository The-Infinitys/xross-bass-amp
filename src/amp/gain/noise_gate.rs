use std::f32::consts::PI;

#[derive(Default, Clone)]
struct GateState {
    gate_gain: f32,
    hold_timer: i32,

    // エンベロープ（帯域別解析）
    env_low: f32,  // 400Hz以下（ベースの基本波・芯）
    env_high: f32, // 3.5kHz以上（ヒスノイズおよびアタック成分）

    // ノイズフロア学習
    noise_floor_high: f32,

    // フィルター状態 (12dB/oct への強化のため2つ用意)
    lp_state_1: f32,
    lp_state_2: f32,

    // 解析用
    lp_analysis_state: f32,
    hp_analysis_state: f32,

    prev_input: f32,
    noise_measure_timer: i32,
    adaptive_sensitivity: f32,
}

pub struct AutoNoiseGate {
    sample_rate: f32,
    state: GateState,
    analysis_buffer: Vec<bool>,
}

impl AutoNoiseGate {
    pub fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            sample_rate,
            state: GateState::default(),
            analysis_buffer: Vec::with_capacity(512),
        };
        s.state.gate_gain = 1.0;
        s.state.noise_floor_high = 0.0005;
        s.state.adaptive_sensitivity = 4.5; // 少しタイトに設定
        s
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    pub fn pre_process(&mut self, buffer: &[f32]) {
        if self.analysis_buffer.len() != buffer.len() {
            self.analysis_buffer.resize(buffer.len(), false);
        }

        let state = &mut self.state;

        // 解析帯域の調整
        // 低域: 400Hz (音の芯)
        // 高域: 3500Hz (ここより上を「ノイズ領域」として重点監視)
        let lp_alpha = 1.0 - (-2.0 * PI * 400.0 / self.sample_rate).exp();
        let hp_alpha = 1.0 - (-2.0 * PI * 3500.0 / self.sample_rate).exp();

        for (i, &sample) in buffer.iter().enumerate() {
            let abs_in = sample.abs();

            // --- 1. マルチバンド解析 ---
            state.lp_analysis_state += lp_alpha * (abs_in - state.lp_analysis_state);
            state.env_low = state.lp_analysis_state;

            let hp_out = abs_in - state.hp_analysis_state;
            state.hp_analysis_state += hp_alpha * hp_out;
            // 高域エンベロープはピークを逃さないよう速めに設定
            state.env_high += 0.15 * (hp_out.abs() - state.env_high);

            // --- 2. ノイズ学習 ---
            let is_quiet = abs_in < 0.015;
            let is_stable = (abs_in - state.prev_input).abs() < 0.0005;
            state.prev_input = abs_in;

            if is_quiet && is_stable {
                state.noise_measure_timer += 1;
            } else {
                state.noise_measure_timer = 0;
            }

            // 静寂時にノイズの平均レベルを更新
            if state.noise_measure_timer > (0.15 * self.sample_rate) as i32 {
                let lr = 0.005;
                state.noise_floor_high += lr * (state.env_high - state.noise_floor_high);
            }
            state.noise_floor_high = state.noise_floor_high.clamp(0.00001, 0.01);

            // --- 3. 周波数依存ヒステリシス・ロジック ---
            let high_th = state.noise_floor_high * state.adaptive_sensitivity;

            // 判定：高域が閾値を超えるか、低域に十分なパワーがある場合に開く
            let is_open = if state.gate_gain < 0.1 {
                // 閉鎖中：開くためには高いエネルギーが必要
                state.env_high > high_th || state.env_low > 0.015
            } else {
                // 開放中：維持するためには半分のエネルギーで良い（チャタリング防止）
                state.env_high > high_th * 0.5 || state.env_low > 0.007
            };

            self.analysis_buffer[i] = is_open;
        }
    }

    pub fn post_process(&mut self, buffer: &mut [f32]) {
        let state = &mut self.state;

        let hold_samples = (0.040 * self.sample_rate) as i32;
        let atk_alpha = 1.0 - (-1.0 / (0.0005 * self.sample_rate)).exp(); // 0.5ms 超高速アタック
        let rel_alpha = 1.0 - (-1.0 / (0.120 * self.sample_rate)).exp(); // 120ms 自然なリリース

        for (i, sample) in buffer.iter_mut().enumerate() {
            let is_detected = self.analysis_buffer[i];

            let target_gain = if is_detected {
                state.hold_timer = hold_samples;
                1.0
            } else if state.hold_timer > 0 {
                state.hold_timer -= 1;
                1.0
            } else {
                0.0
            };

            // ゲインのスムージング
            let g_alpha = if target_gain > state.gate_gain {
                atk_alpha
            } else {
                rel_alpha
            };
            state.gate_gain += g_alpha * (target_gain - state.gate_gain);

            // --- 4. 2段式 Dynamic High-Cut (12dB/oct) ---
            // SNR（信号対ノイズ比）を計算
            let snr = (state.env_high / (state.noise_floor_high + 1e-9)).clamp(0.0, 10.0);

            // 演奏中：SNRが低い（ノイズに近い）ほど、カットオフを大胆に下げる
            // 閉鎖中：gate_gainに追従して、フィルターを完全に「閉じる」
            let cutoff_base = if state.gate_gain > 0.95 {
                // 演奏中：2kHz(0.1) 〜 20kHz(1.0) の間で動的に変化
                (snr / 10.0).powi(2).clamp(0.1, 1.0)
            } else {
                // ゲート閉鎖中：gate_gainの3乗で急激に絞り込む（遮断性能を重視）
                state.gate_gain.powi(3).clamp(0.001, 1.0)
            };

            let gated_input = *sample * state.gate_gain;

            // 1段目 (6dB/oct)
            state.lp_state_1 += cutoff_base * (gated_input - state.lp_state_1);
            // 2段目 (さらに 6dB/oct 重ねて 12dB/oct に)
            state.lp_state_2 += cutoff_base * (state.lp_state_1 - state.lp_state_2);

            *sample = state.lp_state_2;
        }
    }
}
