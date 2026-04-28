use crate::modules::filter::{Biquad, FilterType};
use crate::params::XrossBassAmpParams;
use std::sync::Arc;

pub struct EqProcessor {
    pub params: Arc<XrossBassAmpParams>,

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
    pub fn new(params: Arc<XrossBassAmpParams>) -> Self {
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
            // --- ベースアンプとして「美味しい」周波数選定 ---

            // Low: 80Hz. ベースの基礎となる帯域。
            self.low_filter
                .set_params(FilterType::LowShelf(l), 80.0, 0.707);

            // Mid: 500Hz. メタルベースの「ゴリッ」とした質感（Growl）を司る。
            self.mid_filter
                .set_params(FilterType::Peaking(m), 500.0, 0.5);

            // High: 2.8kHz. ピックアタックや弦の金属的な響き（Clank）。
            self.high_filter
                .set_params(FilterType::HighShelf(h), 2800.0, 0.707);

            // Presence: 6kHz. 抜けと明瞭さ。
            self.presence_filter
                .set_params(FilterType::HighShelf(p), 6000.0, 0.8);

            // Resonance: 45Hz. サブベースの重み。
            self.resonance_filter
                .set_params(FilterType::Peaking(r), 45.0, 1.5);

            // キャッシュ更新
            self.last_eq_values = [l, m, h, p, r];
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        // パラメータ更新のチェック
        self.update_coefficients();

        let mut signal = input;

        signal = self.resonance_filter.process(signal);
        signal = self.low_filter.process(signal);
        signal = self.mid_filter.process(signal);
        signal = self.high_filter.process(signal);
        signal = self.presence_filter.process(signal);

        signal
    }
}
