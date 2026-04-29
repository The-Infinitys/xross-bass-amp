use crate::modules::filter::{Biquad, FilterType};
use crate::params::XrossBassAmpParams;
use std::sync::Arc;

pub struct EqProcessor {
    pub params: Arc<XrossBassAmpParams>,

    // フィルタ群（ベース用に構成を変更）
    sub_low_filter: Biquad,
    low_filter: Biquad,
    mid_filter: Biquad,
    high_filter: Biquad,
    presence_filter: Biquad,

    // パラメータ変更検知用キャッシュ (6つのパラメータを監視)
    last_eq_values: [f32; 5],
}

impl EqProcessor {
    pub fn new(params: Arc<XrossBassAmpParams>) -> Self {
        let sr = 44100.0;
        Self {
            params,
            sub_low_filter: Biquad::new(sr),
            low_filter: Biquad::new(sr),
            mid_filter: Biquad::new(sr),
            high_filter: Biquad::new(sr),
            presence_filter: Biquad::new(sr),
            last_eq_values: [-999.0; 5],
        }
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        self.sub_low_filter = Biquad::new(sample_rate);
        self.low_filter = Biquad::new(sample_rate);
        self.mid_filter = Biquad::new(sample_rate);
        self.high_filter = Biquad::new(sample_rate);
        self.presence_filter = Biquad::new(sample_rate);
        self.reset();
    }

    pub fn reset(&mut self) {
        self.sub_low_filter.reset();
        self.low_filter.reset();
        self.mid_filter.reset();
        self.high_filter.reset();
        self.presence_filter.reset();
        self.last_eq_values = [-999.0; 5];
    }

    fn update_coefficients(&mut self) {
        let sl = self.params.sub_low.value();
        let l = self.params.low.value();
        let m = self.params.mid.value();
        let h = self.params.high.value();
        let p = self.params.presence.value();

        // 変更検知
        if (sl - self.last_eq_values[0]).abs() > 0.01
            || (l - self.last_eq_values[1]).abs() > 0.01
            || (m - self.last_eq_values[2]).abs() > 0.01
            || (h - self.last_eq_values[3]).abs() > 0.01
            || (p - self.last_eq_values[4]).abs() > 0.01
        {
            // --- ベースアンプとして「美味しい」周波数選定 ---

            // Sub Low: 40Hz〜60Hz。5弦ベースのLow-Bや地響きのような重低音。
            // Qを少し鋭め(1.2)にして、ボワつきを抑えつつ持ち上げる。
            self.sub_low_filter
                .set_params(FilterType::Peaking(sl), 50.0, 1.2);

            // Low: 100Hz〜150Hz。ベースの「太さ」のメイン帯域。
            // ギターと被りやすい帯域なので、シェルフで全体を持ち上げる。
            self.low_filter
                .set_params(FilterType::LowShelf(l), 120.0, 0.707);

            // Mid: 400Hz〜600Hz。音の「芯」と「存在感」。
            // ここを削るとモダンなドンシャリ、上げるとブリブリした粘りが出る。
            self.mid_filter
                .set_params(FilterType::Peaking(m), 500.0, 0.5);

            // High: 2kHz〜3kHz。弦の擦れる音やピッキングアタック。
            // スラップのプル音などもここが担当。
            self.high_filter
                .set_params(FilterType::HighShelf(h), 2500.0, 0.707);

            // Presence (Attack): 5kHz以上。
            // アクティブベースのプリアンプのような、クリスタルな高域の伸び。
            self.presence_filter
                .set_params(FilterType::HighShelf(p), 5000.0, 0.8);

            self.last_eq_values = [sl, l, m, h, p];
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        self.update_coefficients();

        let mut signal = input;

        // 処理順序：低域の土台を固めてから高域のキャラクターを乗せる
        signal = self.sub_low_filter.process(signal);
        signal = self.low_filter.process(signal);
        signal = self.mid_filter.process(signal);
        signal = self.high_filter.process(signal);
        signal = self.presence_filter.process(signal);

        signal
    }
}
