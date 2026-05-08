use std::f32::consts::PI;

#[derive(Default, Clone)]
struct GateState {
    gate_gain: f32,
    hold_timer: i32,

    // エンベロープ（帯域別解析）
    env_low: f32,  // 400Hz以下（ベースの基本波・芯）
    env_high: f32, // 2kHz以上（アタックおよびヒスノイズ成分）

    // ノイズフロア（高域ノイズに特化して学習）
    noise_floor_high: f32,

    // フィルタ状態
    lp_state: f32,           // 解析用LPF
    hp_state: f32,           // 解析用HPF
    noise_filter_state: f32, // 最終出力用の動的ハイカットフィルタ

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
        s.state.adaptive_sensitivity = 6.0;
        s
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    /// バッファを解析し、ゲートの開閉およびノイズレベルを特定
    pub fn pre_process(&mut self, buffer: &[f32]) {
        if self.analysis_buffer.len() != buffer.len() {
            self.analysis_buffer.resize(buffer.len(), false);
        }

        let state = &mut self.state;

        // サンプルレートに基づいた解析用フィルタ係数
        let lp_alpha = 1.0 - (-2.0 * PI * 400.0 / self.sample_rate).exp();
        let hp_alpha = 1.0 - (-2.0 * PI * 2000.0 / self.sample_rate).exp();

        for (i, &sample) in buffer.iter().enumerate() {
            let abs_in = sample.abs();

            // --- 1. マルチバンド・エンベロープ解析 ---
            // 低域（芯の維持判定用）
            state.lp_state += lp_alpha * (abs_in - state.lp_state);
            state.env_low = state.lp_state;

            // 高域（アタック判定およびノイズフロア学習用）
            let hp_out = abs_in - state.hp_state;
            state.hp_state += hp_alpha * hp_out;
            state.env_high += 0.1 * (hp_out.abs() - state.env_high);

            // --- 2. 賢いノイズ学習 (高域ターゲット) ---
            let is_quiet = abs_in < 0.02;
            let is_stable = (abs_in - state.prev_input).abs() < 0.001;
            state.prev_input = abs_in;

            if is_quiet && is_stable {
                state.noise_measure_timer += 1;
            } else {
                state.noise_measure_timer = 0;
            }

            // 200msの静寂で学習
            if state.noise_measure_timer > (0.2 * self.sample_rate) as i32 {
                let lr = 0.01;
                state.noise_floor_high += lr * (state.env_high - state.noise_floor_high);
            }
            state.noise_floor_high = state.noise_floor_high.clamp(0.00001, 0.01);

            // --- 3. ゲート判定ロジック ---
            let high_th = state.noise_floor_high * state.adaptive_sensitivity;

            // ヒステリシス：一度開いたら、低い閾値（0.5倍）まで閉じない
            let is_open = if state.gate_gain < 0.1 {
                state.env_high > high_th || state.env_low > 0.012
            } else {
                state.env_high > high_th * 0.5 || state.env_low > 0.006
            };

            self.analysis_buffer[i] = is_open;
        }
    }

    /// 解析結果に基づき、ゲインと「動的ハイカット」を適用
    pub fn post_process(&mut self, buffer: &mut [f32]) {
        let state = &mut self.state;

        let hold_samples = (0.045 * self.sample_rate) as i32; // 45ms
        let atk_alpha = 1.0 - (-1.0 / (0.001 * self.sample_rate)).exp(); // 1ms (高速アタック)
        let rel_alpha = 1.0 - (-1.0 / (0.110 * self.sample_rate)).exp(); // 110ms (自然なリリース)

        for (i, sample) in buffer.iter_mut().enumerate() {
            let is_detected = self.analysis_buffer[i];

            // ターゲットゲインの計算
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
            let alpha = if target_gain > state.gate_gain {
                atk_alpha
            } else {
                rel_alpha
            };
            state.gate_gain += alpha * (target_gain - state.gate_gain);

            // --- 4. Dynamic Noise Shaper (演奏中ハイカット) ---
            // 高域成分がノイズフロアに近い場合、たとえゲートが開いていてもハイを削る
            let noise_trigger =
                (state.env_high / (state.noise_floor_high * 3.0 + 1e-9)).clamp(0.0, 1.0);

            // 演奏中：高域成分が少なければカットオフを1kHz付近まで落とす
            // 閉鎖中：gate_gainの減少に伴い、さらに強力に(0.005)までフィルタを絞る
            let cutoff_alpha = if state.gate_gain > 0.99 {
                0.08 + 0.92 * noise_trigger.powi(2)
            } else {
                state.gate_gain.powi(3).clamp(0.005, 1.0)
            };

            // ゲイン適用後の信号に動的LPFを適用
            let gated_input = *sample * state.gate_gain;
            state.noise_filter_state += cutoff_alpha * (gated_input - state.noise_filter_state);

            *sample = state.noise_filter_state;
        }
    }
}
