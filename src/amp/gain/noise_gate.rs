use std::f32::consts::PI;

#[derive(Default, Clone)]
struct GateState {
    gate_gain: f32,
    hold_timer: i32,

    // エンベロープ（帯域別）
    env_full: f32, // 全帯域の音量
    env_high: f32, // 高域（1kHz以上）の音量

    // ノイズフロア（帯域別）
    noise_floor_full: f32,
    noise_floor_high: f32,

    // フィルタ状態
    hp_state: f32,       // 解析用高域抽出フィルタの状態
    post_lpf_state: f32, // 閉じる際のスムージングフィルタ

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
        let mut state = GateState::default();
        state.gate_gain = 1.0;
        state.noise_floor_full = 0.001;
        state.noise_floor_high = 0.0005;
        state.adaptive_sensitivity = 6.0;

        Self {
            sample_rate,
            state,
            analysis_buffer: Vec::with_capacity(512),
        }
    }

    /// バッファを解析し、各サンプルのゲート開閉フラグを生成
    pub fn pre_process(&mut self, buffer: &[f32]) {
        if self.analysis_buffer.len() != buffer.len() {
            self.analysis_buffer.resize(buffer.len(), false);
        }

        let state = &mut self.state;

        // 設定定数
        let measure_window = (0.2 * self.sample_rate) as i32; // 200msの静寂でノイズ学習
        let nf_max = 0.015; // ノイズフロア学習の上限 (-36dB)
        let nf_min = 0.00003; // ノイズフロア学習の下限 (-90dB)

        // 簡易HPF係数 (約1.5kHzで高域抽出)
        let hp_coef = (2.0 * PI * 1500.0 / self.sample_rate).exp() * -1.0 + 1.0;

        for (i, &sample) in buffer.iter().enumerate() {
            let abs_in = sample.abs();

            // --- 1. 帯域別エンベロープ抽出 ---
            // 全帯域
            state.env_full += 0.1 * (abs_in - state.env_full);

            // 高域 (1次HPF経由)
            let hp_out = abs_in - state.hp_state;
            state.hp_state += hp_coef * hp_out;
            state.env_high += 0.1 * (hp_out.abs() - state.env_high);

            // --- 2. 賢いマルチバンド・ノイズ計測 ---
            // 演奏中でなく(0.02以下)、かつ信号が安定しているか
            let is_quiet = abs_in < 0.02;
            let is_stable = (abs_in - state.prev_input).abs() < 0.001;
            state.prev_input = abs_in;

            if is_quiet && is_stable {
                state.noise_measure_timer += 1;
            } else {
                state.noise_measure_timer = 0;
            }

            if state.noise_measure_timer > measure_window {
                // 帯域別にノイズレベルを学習
                let lr = 0.01; // 学習率
                state.noise_floor_full += lr * (state.env_full - state.noise_floor_full);
                state.noise_floor_high += lr * (state.env_high - state.noise_floor_high);
            }

            // クランプ処理
            state.noise_floor_full = state.noise_floor_full.clamp(nf_min, nf_max);
            state.noise_floor_high = state.noise_floor_high.clamp(nf_min * 0.5, nf_max * 0.8);

            // --- 3. 自動感度調整 (SNRに基づく) ---
            let snr = (state.env_full / (state.noise_floor_full + 1e-9)).max(1.0);
            let target_sens = if snr < 2.0 {
                14.0
            } else if snr > 12.0 {
                4.0
            } else {
                8.0
            };
            state.adaptive_sensitivity += 0.001 * (target_sens - state.adaptive_sensitivity);

            // --- 4. ゲート判定ロジック ---
            // 高域と全帯域の両方をチェック
            let full_th = state.noise_floor_full * state.adaptive_sensitivity;
            let high_th = state.noise_floor_high * (state.adaptive_sensitivity * 0.7);

            // どちらかの帯域がしきい値を超えていれば「音あり」とみなす
            // ヒステリシス：一度開いたら、しきい値の半分まで閉じるのを待つ
            let is_open = if state.gate_gain < 0.1 {
                state.env_full > full_th || state.env_high > high_th
            } else {
                state.env_full > full_th * 0.5 || state.env_high > high_th * 0.5
            };

            self.analysis_buffer[i] = is_open;
        }
    }

    /// Preの結果に基づき、ゲインと動的フィルタを適用
    pub fn post_process(&mut self, buffer: &mut [f32]) {
        let state = &mut self.state;

        // ギター用にチューニングされた時間定数
        let hold_samples = (0.015 * self.sample_rate) as i32; // 15ms
        let release_ms = 70.0;

        let atk_coef = (-1.0 / (0.0008 * self.sample_rate)).exp(); // 高速アタック
        let rel_coef = (-1.0 / (release_ms * self.sample_rate / 1000.0)).exp();

        for (i, sample) in buffer.iter_mut().enumerate() {
            let is_detected = self.analysis_buffer[i];

            // ターゲットゲインの決定とホールド処理
            let target_gain = if is_detected {
                state.hold_timer = hold_samples;
                1.0
            } else if state.hold_timer > 0 {
                state.hold_timer -= 1;
                1.0
            } else {
                0.0
            };

            // ゲインスムージング
            let coef = if target_gain > state.gate_gain {
                atk_coef
            } else {
                rel_coef
            };
            state.gate_gain = target_gain + coef * (state.gate_gain - target_gain);

            let input_val = *sample;
            let mut gated_sample = input_val * state.gate_gain;

            // --- 動的ポストフィルタ (閉じる際、高域から先に削る) ---
            // ゲートが閉じる(gate_gainが1未満になる)につれてLPFを強くかける
            // これにより「バッサリ切れた」感じをなくし、自然な減衰を演出する
            let lpf_alpha = state.gate_gain.powi(3).clamp(0.05, 1.0);
            state.post_lpf_state += lpf_alpha * (gated_sample - state.post_lpf_state);

            if state.gate_gain < 0.999 {
                gated_sample = state.post_lpf_state;
            }

            *sample = gated_sample;
        }
    }
}
