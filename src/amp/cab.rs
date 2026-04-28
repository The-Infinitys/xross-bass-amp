use crate::modules::filter::{Biquad, FilterType};
use crate::params::XrossGuitarAmpParams;
use std::sync::Arc;

const PHASE_DELAY_SIZE: usize = 2048;
const PHASE_DELAY_MASK: usize = PHASE_DELAY_SIZE - 1;
const MAX_BUFFER_SIZE: usize = 192000;

pub struct CabProcessor {
    pub params: Arc<XrossGuitarAmpParams>,

    // --- フィルタ群 ---
    mic_a_filters: [Biquad; 5],
    mic_b_filters: [Biquad; 5],

    // 物理特性
    impedance_resonance: Biquad,
    presence_shelf: Biquad,
    cabinet_thump: Biquad, // 低域の押し出し
    box_resonance: Biquad, // 箱鳴り (200-400Hz付近)
    tight_filter: Biquad,

    // 位相を散らすためのAll-pass
    phase_smearer: [Biquad; 2],

    // スピーカーコーンの分割振動
    cone_character: [Biquad; 4],
    internal_standing_wave: Biquad,

    // --- バッファ群 ---
    phase_delay_buffer_b: Vec<f32>,
    room_delay_buffer: Vec<f32>,
    write_idx_phase: usize,
    write_idx_room: usize,

    sample_rate: f32,

    // --- キャッシュ ---
    last_speaker_size: f32,
    last_speaker_count: i32,
    last_mic_params: [f32; 4],
    last_eq_extras: [f32; 2],
}

impl CabProcessor {
    pub fn new(params: Arc<XrossGuitarAmpParams>) -> Self {
        let sr = 44100.0;
        Self {
            params,
            mic_a_filters: std::array::from_fn(|_| Biquad::new(sr)),
            mic_b_filters: std::array::from_fn(|_| Biquad::new(sr)),
            impedance_resonance: Biquad::new(sr),
            presence_shelf: Biquad::new(sr),
            cabinet_thump: Biquad::new(sr),
            box_resonance: Biquad::new(sr),
            tight_filter: Biquad::new(sr),
            phase_smearer: std::array::from_fn(|_| Biquad::new(sr)),
            cone_character: std::array::from_fn(|_| Biquad::new(sr)),
            internal_standing_wave: Biquad::new(sr),
            phase_delay_buffer_b: vec![0.0; PHASE_DELAY_SIZE],
            room_delay_buffer: vec![0.0; MAX_BUFFER_SIZE],
            write_idx_phase: 0,
            write_idx_room: 0,
            sample_rate: sr,
            last_speaker_size: -1.0,
            last_speaker_count: -1,
            last_mic_params: [-1.0; 4],
            last_eq_extras: [-1.0; 2],
        }
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        // すべてのフィルタのサンプリングレートを更新
        self.update_all_filter_rates(sample_rate);
        self.room_delay_buffer.resize(sample_rate as usize, 0.0);
        self.reset();
    }

    fn update_all_filter_rates(&mut self, sr: f32) {
        for f in &mut self.mic_a_filters {
            f.set_sample_rate(sr);
        }
        for f in &mut self.mic_b_filters {
            f.set_sample_rate(sr);
        }
        for f in &mut self.cone_character {
            f.set_sample_rate(sr);
        }
        for f in &mut self.phase_smearer {
            f.set_sample_rate(sr);
        }
        self.impedance_resonance.set_sample_rate(sr);
        self.presence_shelf.set_sample_rate(sr);
        self.cabinet_thump.set_sample_rate(sr);
        self.box_resonance.set_sample_rate(sr);
        self.tight_filter.set_sample_rate(sr);
        self.internal_standing_wave.set_sample_rate(sr);
    }

    pub fn reset(&mut self) {
        // 各フィルタの内部状態（遅延要素）をクリア
        self.phase_delay_buffer_b.fill(0.0);
        self.room_delay_buffer.fill(0.0);
        self.write_idx_phase = 0;
        self.write_idx_room = 0;
    }

    fn update_coefficients(&mut self) {
        let cab = &self.params.cab_section;
        let eq = &self.params.eq_section;

        let s_size = cab.speaker_size.value();
        let s_count = cab.speaker_count.value();
        let d_a = cab.mic_a_distance.value();
        let a_a = cab.mic_a_axis.value();
        let d_b = cab.mic_b_distance.value();
        let a_b = cab.mic_b_axis.value();
        let res_val = eq.resonance.value();
        let pres_val = eq.presence.value();

        // 変更検知（閾値による判定）
        if (s_size - self.last_speaker_size).abs() > 0.001
            || s_count != self.last_speaker_count
            || (d_a - self.last_mic_params[0]).abs() > 0.001
            || (res_val - self.last_eq_extras[0]).abs() > 0.001
        {
            let speaker_res_freq = 82.0 * (12.0 / s_size);
            let count_scale = (s_count as f32).sqrt();

            // 1. パワーアンプとの相互作用
            self.impedance_resonance.set_params(
                FilterType::Peaking(res_val * 2.5),
                speaker_res_freq,
                1.2,
            );
            self.presence_shelf
                .set_params(FilterType::HighShelf(pres_val * 1.8), 3800.0, 0.7);

            // 2. キャビネットの物理的共鳴
            // 箱のサイズに応じた中低域の溜まり
            let box_res_freq = 250.0 * (12.0 / s_size);
            self.box_resonance.set_params(
                FilterType::Peaking(2.0 * count_scale),
                box_res_freq,
                2.0,
            );

            let internal_res = 1150.0 * (12.0 / s_size);
            self.internal_standing_wave
                .set_params(FilterType::Peaking(-5.0), internal_res, 3.0);

            // 3. 位相の拡散 (All-pass) - アナログ的な曖昧さを出す
            self.phase_smearer[0].set_params(FilterType::AllPass, 1200.0, 0.5);
            self.phase_smearer[1].set_params(FilterType::AllPass, 3500.0, 0.5);

            // 4. スピーカー個体差 (Cone Breakup)
            self.cone_character[0].set_params(FilterType::Peaking(-4.0), 850.0, 1.5);
            self.cone_character[1].set_params(FilterType::Peaking(3.0), 2200.0, 1.0);
            self.cone_character[2].set_params(FilterType::Peaking(4.5), 3800.0, 2.0);
            self.cone_character[3].set_params(FilterType::Peaking(-6.0), 6500.0, 2.5);

            // 5. Mic A (Dynamic) - 近接効果と軸外減衰
            let prox_a = (1.0 - d_a).powi(3) * 12.0;
            self.mic_a_filters[0].set_params(
                FilterType::Peaking(prox_a),
                speaker_res_freq * 1.1,
                0.8,
            );
            let edge_a = (1.0 - a_a) * 8.0;
            self.mic_a_filters[2].set_params(FilterType::Peaking(edge_a), 3200.0, 1.0);
            let hc_a = 15000.0 * (1.0 - a_a * 0.5) * (1.0 - d_a * 0.15);
            self.mic_a_filters[4].set_params(FilterType::LowPass, hc_a.max(3000.0), 0.707);

            // 6. Mic B (Ribbon) - よりダークでウォームな特性
            let prox_b = (1.0 - d_b).powi(2) * 18.0;
            self.mic_b_filters[0].set_params(
                FilterType::Peaking(prox_b),
                speaker_res_freq * 0.9,
                0.6,
            );
            let dark_b = (1.0 - d_b * 0.5) * -4.0;
            self.mic_b_filters[3].set_params(FilterType::HighShelf(dark_b), 4000.0, 0.7);
            let hc_b = 11000.0 * (1.0 - a_b * 0.7) * (1.0 - d_b * 0.3);
            self.mic_b_filters[4].set_params(FilterType::LowPass, hc_b.max(2000.0), 0.707);

            // 7. Overall Tightness
            self.cabinet_thump
                .set_params(FilterType::Peaking(3.0 * count_scale), 110.0, 2.0);
            self.tight_filter
                .set_params(FilterType::HighPass, 75.0, 0.6);

            self.last_speaker_size = s_size;
            self.last_speaker_count = s_count;
            self.last_mic_params = [d_a, a_a, d_b, a_b];
            self.last_eq_extras = [res_val, pres_val];
        }
    }

    fn apply_speaker_physics(&self, input: f32) -> f32 {
        // スピーカーの物理的な振幅限界によるソフトサチュレーション
        // ＋ 非対称な磁気回路の挙動
        let drive = 1.1;
        if input > 0.0 {
            (input * drive).tanh()
        } else {
            (input * drive * 0.95).tanh() * 1.02
        }
    }

    pub fn process(&mut self, input: f32) -> (f32, f32) {
        self.update_coefficients();

        // 1. スピーカーダイナミクス
        let mut sig = self.apply_speaker_physics(input);

        // 2. 共通フィルタリング
        sig = self.impedance_resonance.process(sig);
        sig = self.presence_shelf.process(sig);
        sig = self.box_resonance.process(sig);
        sig = self.cabinet_thump.process(sig);
        sig = self.internal_standing_wave.process(sig);
        sig = self.tight_filter.process(sig);

        for ap in &mut self.phase_smearer {
            sig = ap.process(sig);
        }
        for f in &mut self.cone_character {
            sig = f.process(sig);
        }

        // 3. マイクパラレル処理
        let mut sig_a = sig;
        for f in &mut self.mic_a_filters {
            sig_a = f.process(sig_a);
        }

        let mut sig_b = sig;
        for f in &mut self.mic_b_filters {
            sig_b = f.process(sig_b);
        }

        // 4. マイク間位相干渉 (Time Alignment)
        // 距離による微小な遅延差 (1ms ≒ 34cm)
        let delay_a = self.params.cab_section.mic_a_distance.value() * 2.5;
        let delay_b = self.params.cab_section.mic_b_distance.value() * 5.0;
        let diff_samples = (delay_b - delay_a).abs() * 0.001 * self.sample_rate;

        let delay_int = diff_samples as usize;
        let frac = diff_samples - (delay_int as f32);

        self.phase_delay_buffer_b[self.write_idx_phase] = sig_b;
        let r1 = (self.write_idx_phase + PHASE_DELAY_SIZE - delay_int) & PHASE_DELAY_MASK;
        let r2 = (r1 + PHASE_DELAY_SIZE - 1) & PHASE_DELAY_MASK;

        // 線形補間
        sig_b = self.phase_delay_buffer_b[r1] * (1.0 - frac) + self.phase_delay_buffer_b[r2] * frac;
        self.write_idx_phase = (self.write_idx_phase + 1) & PHASE_DELAY_MASK;

        // 5. Stereo Mixing & MS-like Spread
        // マイクAをセンター、マイクBを少しサイドに振ることで実在感を出す
        let mut out_l = sig_a * 0.7 + sig_b * 0.3;
        let mut out_r = sig_a * 0.7 - sig_b * 0.1; // わずかな位相差による広がり

        // 6. Early Reflections (Room Simulation)
        let room_mix = self.params.cab_section.room_mix.value();
        if room_mix > 0.0 {
            let room_size = self.params.cab_section.room_size.value();

            // 左右で異なるタップ時間を設定し、ステレオ感を強調
            let taps_l = [0.011, 0.023, 0.041];
            let taps_r = [0.013, 0.027, 0.048];

            let mut reflections_l = 0.0;
            let mut reflections_r = 0.0;

            let buf_len = self.room_delay_buffer.len();
            for i in 0..3 {
                let dl = ((taps_l[i] + room_size * 0.05) * self.sample_rate) as usize;
                let dr = ((taps_r[i] + room_size * 0.05) * self.sample_rate) as usize;

                reflections_l +=
                    self.room_delay_buffer[(self.write_idx_room + buf_len - dl) % buf_len];
                reflections_r +=
                    self.room_delay_buffer[(self.write_idx_room + buf_len - dr) % buf_len];
            }

            out_l += reflections_l * room_mix * 0.4;
            out_r += reflections_r * room_mix * 0.4;

            self.room_delay_buffer[self.write_idx_room] = (sig_a + sig_b) * 0.5;
            self.write_idx_room = (self.write_idx_room + 1) % buf_len;
        }

        // 最終ゲイン補正 (アナログの飽和感を考慮して少し持ち上げる)
        (out_l * 1.25, out_r * 1.25)
    }
}
