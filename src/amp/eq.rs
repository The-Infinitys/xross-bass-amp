use crate::modules::filter::{Biquad, FilterType};
use crate::params::XrossGuitarAmpParams;
use std::sync::Arc;

pub struct EqProcessor {
    pub params: Arc<XrossGuitarAmpParams>,

    // フィルタ群
    low_filter: Biquad,
    mid_filter: Biquad,
    high_filter: Biquad,
    presence_filter: Biquad,
    resonance_filter: Biquad,

    // パラメータ変更検知用キャッシュ
    last_eq_values: [f32; 5],
}

impl EqProcessor {
    pub fn new(params: Arc<XrossGuitarAmpParams>) -> Self {
        let sr = 44100.0;
        Self {
            params,
            low_filter: Biquad::new(sr),
            mid_filter: Biquad::new(sr),
            high_filter: Biquad::new(sr),
            presence_filter: Biquad::new(sr),
            resonance_filter: Biquad::new(sr),
            // 初期値とズラしておくことで初回に必ず計算させる
            last_eq_values: [-999.0; 5],
        }
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        // 全フィルタのサンプリングレートを更新
        self.low_filter = Biquad::new(sample_rate);
        self.mid_filter = Biquad::new(sample_rate);
        self.high_filter = Biquad::new(sample_rate);
        self.presence_filter = Biquad::new(sample_rate);
        self.resonance_filter = Biquad::new(sample_rate);
        self.reset();
    }

    pub fn reset(&mut self) {
        self.low_filter.reset();
        self.mid_filter.reset();
        self.high_filter.reset();
        self.presence_filter.reset();
        self.resonance_filter.reset();
        self.last_eq_values = [-999.0; 5];
    }

    fn update_coefficients(&mut self) {
        let eq_params = &self.params.eq_section;

        let l = eq_params.low.value();
        let m = eq_params.mid.value();
        let h = eq_params.high.value();
        let p = eq_params.presence.value();
        let r = eq_params.resonance.value();

        // 変更があった場合のみ重いフィルタ係数計算を実行
        if (l - self.last_eq_values[0]).abs() > 0.01
            || (m - self.last_eq_values[1]).abs() > 0.01
            || (h - self.last_eq_values[2]).abs() > 0.01
            || (p - self.last_eq_values[3]).abs() > 0.01
            || (r - self.last_eq_values[4]).abs() > 0.01
        {
            // --- ギターアンプとして「美味しい」周波数選定 ---

            // Low: 100Hz よりも 150Hz 辺りにピークを持たせると「厚み」が出る
            self.low_filter
                .set_params(FilterType::LowShelf(l), 150.0, 0.707);

            // Mid: 750Hz はモダン。500Hz-800Hz 辺りの変化が「粘り」を生む。
            // Q値を少し広め(0.4)にして、不自然なピーク感を抑える
            self.mid_filter
                .set_params(FilterType::Peaking(m), 700.0, 0.4);

            // High: 3kHz は「芯」の部分。
            self.high_filter
                .set_params(FilterType::HighShelf(h), 3000.0, 0.707);

            // Presence: 6kHz〜8kHz。ここが「ヌケ」の正体。
            // キャビネットで削られがちな高域をここで補う
            self.presence_filter
                .set_params(FilterType::HighShelf(p), 6500.0, 0.8);

            // Resonance: 80Hz。スピーカーの「箱の揺れ」を強調。
            // Qを鋭め(1.5)にして、タイトな重低音にする
            self.resonance_filter
                .set_params(FilterType::Peaking(r), 80.0, 1.5);

            // キャッシュ更新
            self.last_eq_values = [l, m, h, p, r];
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        // パラメータ更新のチェック
        self.update_coefficients();

        // 処理順序も重要。
        // 一般的に歪みの後は 低域のレゾナンス(Resonance)から始まり、
        // 最後にヌケを調整する Presence を通すのが音楽的。
        let mut signal = input;

        signal = self.resonance_filter.process(signal);
        signal = self.low_filter.process(signal);
        signal = self.mid_filter.process(signal);
        signal = self.high_filter.process(signal);
        signal = self.presence_filter.process(signal);

        signal
    }
}
