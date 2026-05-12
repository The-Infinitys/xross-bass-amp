use crate::modules::filter::{Biquad, FilterType};
use crate::params::XrossBassAmpParams;
use std::sync::Arc;
use truce::core::AudioBuffer;

const PHASE_DELAY_SIZE: usize = 2048;
const MAX_ROOM_DELAY: usize = 192000;

pub struct CabProcessor {
    pub params: Arc<XrossBassAmpParams>,

    // --- 物理モデリング・フィルター ---
    body_resonators: [Biquad; 3], // Sub-bass, Box-low, Baffle
    woofer_character: Biquad,     // 低域の太さと粘り
    tweeter_path: Biquad,         // 高域のパキッとした質感 (10-inch+Hornイメージ)

    // --- マイクロフォン・セクション ---
    // Mic A: Large Diaphragm Dynamic (D112/RE20 style) - 芯と重さ
    mic_a_tone: [Biquad; 3],
    // Mic B: Condenser/DI-Blend style - 解像度と輪郭
    mic_b_tone: [Biquad; 3],

    // --- Bass Mastering Chain ---
    sub_thump: Biquad,          // 50-60Hzの「地面を揺らす」成分
    growl_shelf: Biquad,        // 800Hz付近の歪みのエッジ
    mud_cut: Biquad,            // 250Hz付近の濁り取り
    low_end_stabilizer: Biquad, // 最終的な低域の引き締め

    // ステレオ・空間・物理挙動
    phase_alignment_delay: Vec<f32>,
    room_reflection: Vec<f32>,
    write_idx_room: usize,

    sample_rate: f32,
    cone_inertia_state: f32, // スピーカーコーンの慣性による「戻り」の遅れ
    last_params_hash: f32,
}

impl CabProcessor {
    pub fn new(params: Arc<XrossBassAmpParams>) -> Self {
        let sr = 44100.0;
        Self {
            params,
            body_resonators: std::array::from_fn(|_| Biquad::new(sr)),
            woofer_character: Biquad::new(sr),
            tweeter_path: Biquad::new(sr),
            mic_a_tone: std::array::from_fn(|_| Biquad::new(sr)),
            mic_b_tone: std::array::from_fn(|_| Biquad::new(sr)),
            sub_thump: Biquad::new(sr),
            growl_shelf: Biquad::new(sr),
            mud_cut: Biquad::new(sr),
            low_end_stabilizer: Biquad::new(sr),

            phase_alignment_delay: vec![0.0; PHASE_DELAY_SIZE],
            room_reflection: vec![0.0; MAX_ROOM_DELAY],
            write_idx_room: 0,
            sample_rate: sr,
            cone_inertia_state: 0.0,
            last_params_hash: -1.0,
        }
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        let filters: &mut [&mut Biquad] = &mut [
            &mut self.woofer_character,
            &mut self.tweeter_path,
            &mut self.sub_thump,
            &mut self.growl_shelf,
            &mut self.mud_cut,
            &mut self.low_end_stabilizer,
        ];
        for f in filters {
            f.set_sample_rate(sample_rate);
        }
        for f in &mut self.body_resonators {
            f.set_sample_rate(sample_rate);
        }
        for f in &mut self.mic_a_tone {
            f.set_sample_rate(sample_rate);
        }
        for f in &mut self.mic_b_tone {
            f.set_sample_rate(sample_rate);
        }
        self.reset();
    }

    pub fn reset(&mut self) {
        self.phase_alignment_delay.fill(0.0);
        self.room_reflection.fill(0.0);
        self.cone_inertia_state = 0.0;
    }

    fn update_coefficients_if_needed(&mut self) {
        let size = self.params.speaker_size.value();
        let res_mod = self.params.resonance.value();
        let pres_mod = self.params.presence.value();

        let current_hash = size + res_mod * 1.1 + pres_mod * 0.9;
        if (current_hash - self.last_params_hash).abs() < 0.0001 {
            return;
        }

        // 1. キャビネット共鳴 (ベース特有の低域重心)
        // Sub: 地面を揺らす超低域
        self.body_resonators[0].set_params(
            FilterType::Peaking(5.0 * res_mod),
            55.0 * (15.0 / size),
            1.2,
        );
        // Low-Mid: 箱の鳴り (濁りすぎないようにQを調整)
        self.body_resonators[1].set_params(FilterType::Peaking(2.0 * res_mod), 180.0, 2.0);
        // Baffle: アタックの跳ね返り
        self.body_resonators[2].set_params(FilterType::Peaking(1.5), 900.0, 1.0);

        // 2. ウーファーとツイーターの役割分担
        // ツイーター (スラップのパキパキ感)
        self.tweeter_path
            .set_params(FilterType::HighShelf(pres_mod * 6.0), 3500.0, 0.7);

        // 3. Mic A (Dynamic: 重厚感重視)
        let dist_a = self.params.mic_a_distance.value();
        self.mic_a_tone[0].set_params(FilterType::Peaking((1.0 - dist_a) * 6.0), 80.0, 0.6); // 近接効果
        self.mic_a_tone[1].set_params(FilterType::Peaking(2.0), 2500.0, 0.8); // 輪郭
        self.mic_a_tone[2].set_params(FilterType::LowPass, 6000.0, 0.7); // 高域の丸み

        // 4. Mic B (Condenser/DI: 解像度重視)
        let dist_b = self.params.mic_b_distance.value();
        self.mic_b_tone[0].set_params(FilterType::Peaking(3.0), 400.0, 0.7); // 中域の押し出し
        self.mic_b_tone[1].set_params(FilterType::HighShelf(pres_mod * 3.0), 4000.0, 0.7);
        self.mic_b_tone[2].set_params(FilterType::LowPass, 12000.0 - (dist_b * 4000.0), 0.7);

        // 5. ミックスを助ける最終処理
        self.sub_thump
            .set_params(FilterType::Peaking(2.5), 65.0, 1.5);
        self.mud_cut
            .set_params(FilterType::Peaking(-3.0), 250.0, 1.5); // 濁りカット
        self.growl_shelf
            .set_params(FilterType::HighShelf(pres_mod * 2.0), 1200.0, 0.5);
        self.low_end_stabilizer
            .set_params(FilterType::HighPass, 35.0, 0.7); // 超低域の整理

        self.last_params_hash = current_hash;
    }

    pub fn process_truce(&mut self, buffer: &mut AudioBuffer) {
        self.update_coefficients_if_needed();

        let num_samples = buffer.num_samples();
        let room_mix = self.params.room_mix.value();
        let size = self.params.speaker_size.value();

        for i in 0..num_samples {
            let mut sig = buffer.output(0)[i];

            // --- 1. Cone Inertia & Nonlinear Saturation (物理的な「粘り」) ---
            // 大口径スピーカーほど戻りが遅く、重低音で飽和する挙動
            let inertia = (0.92 - (size * 0.005)).clamp(0.8, 0.95);
            let saturated = if sig > 0.0 {
                sig.atan()
            } else {
                (sig * 0.96).atan() * 1.04
            };
            sig = self.cone_inertia_state + inertia * (saturated - self.cone_inertia_state);
            self.cone_inertia_state = sig;

            // --- 2. Cabinet Resonances ---
            for res in &mut self.body_resonators {
                sig = res.process(sig);
            }

            // --- 3. Parallel Path (Woofer & Tweeter) ---
            let woofer_sig = sig;
            let tweeter_sig = self.tweeter_path.process(sig);
            // ツイーターは高域のみ通し、ウーファーの太さにパキッとしたエッジを足す
            let combined_sig = woofer_sig + tweeter_sig * 0.4;

            // --- 4. Dual Mic Path ---
            let mut sig_a = combined_sig;
            for f in &mut self.mic_a_tone {
                sig_a = f.process(sig_a);
            }

            let mut sig_b = combined_sig;
            for f in &mut self.mic_b_tone {
                sig_b = f.process(sig_b);
            }

            // --- 5. Mixing & Stabilization ---
            // ベースはセンターの定位が命なので、ステレオ幅は控えめに、奥行きを重視
            let mut out_l = sig_a * 0.8 + sig_b * 0.4;
            let mut out_r = sig_a * 0.8 - sig_b * 0.2; // 微かな位相差で空間を作る

            // 最終的なトーン補正 (Mastering Logic)
            out_l = self.sub_thump.process(out_l);
            out_r = self.sub_thump.process(out_r);
            out_l = self.mud_cut.process(out_l);
            out_r = self.mud_cut.process(out_r);
            out_l = self.growl_shelf.process(out_l);
            out_r = self.growl_shelf.process(out_r);
            out_l = self.low_end_stabilizer.process(out_l);
            out_r = self.low_end_stabilizer.process(out_r);

            // Room (ベースのルームは「広さ」より「壁の跳ね返りの硬さ」)
            if room_mix > 0.0 {
                let reflect_time = 0.02 + self.params.room_size.value() * 0.03;
                let dr = (reflect_time * self.sample_rate) as usize;
                let buf_len = self.room_reflection.len();
                let idx = (self.write_idx_room + buf_len - dr) % buf_len;

                let reflection = (self.room_reflection[idx] * 0.7).tanh() * 0.3;
                out_l += reflection * room_mix;
                out_r -= reflection * room_mix;

                self.room_reflection[self.write_idx_room] = (out_l + out_r) * 0.5;
                self.write_idx_room = (self.write_idx_room + 1) % buf_len;
            }

            // 出力 (ベースはモノラル互換性を極めて高く保つ)
            if buffer.num_output_channels() >= 2 {
                buffer.output(0)[i] = out_l;
                buffer.output(1)[i] = out_r;
            } else {
                buffer.output(0)[i] = (out_l + out_r) * 0.5;
            }
        }
    }
}
