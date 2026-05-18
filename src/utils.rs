use egui::Color32;
use truce::params::{FloatParam, FloatParamReadF64};

pub trait FloatParamNormalizedExt {
    fn value_normalized(&self) -> f64;
    fn set_value_normalized(&self, norm: f64); // FloatParamは内部でAtomicを使うので&selfでOKなはずです
}

impl FloatParamNormalizedExt for FloatParam {
    fn value_normalized(&self) -> f64 {
        let val = self.value();
        let range = &self.info.range;
        range.normalize(val)
    }

    fn set_value_normalized(&self, norm: f64) {
        let range = &self.info.range;
        let val = range.denormalize(norm);
        self.set_value(val);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use truce::params::{
        FloatParam, ParamFlags, ParamInfo, ParamRange, ParamUnit, ParamValueKind, SmoothingStyle,
    };

    // ヘルパー: テスト用のFloatParamを作成
    fn create_test_param(min: f64, max: f64, default: f64) -> FloatParam {
        FloatParam::new(
            ParamInfo {
                id: 1,              // u32型なので数値を直接指定
                name: "Test Param", // &str型
                short_name: "Test", // Optionではなく&str
                group: "",          // Optionではなく&str
                range: ParamRange::Linear { min, max },
                default_plain: default,
                unit: ParamUnit::Db,
                kind: ParamValueKind::Float,
                flags: ParamFlags::empty(), // default() ではなく empty()
            },
            SmoothingStyle::Exponential(50.0),
        )
    }

    #[test]
    fn test_normalization_mapping() {
        let param = create_test_param(-60.0, 0.0, -30.0);

        // 1. 中央値のチェック (Linear)
        assert!((param.value_normalized() - 0.5).abs() < 1e-6);

        // 2. 最小値のセット
        param.set_value_normalized(0.0);
        assert!((param.value() - (-60.0)).abs() < 1e-6);

        // 3. 最大値のセット
        param.set_value_normalized(1.0);
        assert!((param.value() - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_out_of_bounds_clamping() {
        let param = create_test_param(-60.0, 12.0, 0.0);

        // 1. 1.0を超える値が入力された場合、最大値(12.0)でクランプされるか
        param.set_value_normalized(1.5);
        assert!(param.value() <= 12.0 + 1e-6);
        assert!((param.value_normalized() - 1.0).abs() < 1e-6);

        // 2. 0.0未満の値が入力された場合、最小値(-60.0)でクランプされるか
        param.set_value_normalized(-0.5);
        assert!(param.value() >= -60.0 - 1e-6);
        assert!((param.value_normalized() - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_logarithmic_normalization() {
        let param = FloatParam::new(
            ParamInfo {
                id: 2,
                name: "Freq",
                short_name: "",
                group: "",
                range: ParamRange::Logarithmic {
                    min: 20.0,
                    max: 20000.0,
                },
                default_plain: 1000.0,
                kind: ParamValueKind::Float,
                unit: ParamUnit::Hz,
                flags: ParamFlags::empty(),
            },
            SmoothingStyle::Exponential(50.0),
        );

        let norm = param.value_normalized();
        // 0.0 ～ 1.0 の間に収まっているか
        assert!(norm >= 0.0 && norm <= 1.0);

        // 再度 denormalize して元の値に戻るか
        param.set_value_normalized(norm);
        assert!((param.value() - 1000.0).abs() < 1e-3);
    }
}

pub trait WithAlpha {
    fn with_alpha(self, alpha: u8) -> Self;
}
impl WithAlpha for Color32 {
    fn with_alpha(self, alpha: u8) -> Self {
        Self::from_rgba_premultiplied(self.r(), self.g(), self.b(), alpha)
    }
}
