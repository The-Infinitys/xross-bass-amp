use egui::Color32;
use truce::params::FloatParam;

pub trait FloatParamNormalizedExt {
    fn value_normalized(&self) -> f64;
    fn set_value_normalized(&self, norm: f64);
}

impl FloatParamNormalizedExt for FloatParam {
    fn value_normalized(&self) -> f64 {
        let val = self.value() as f64;
        let range = &self.info.range;
        range.normalize(val)
    }

    fn set_value_normalized(&self, norm: f64) {
        let range = &self.info.range;
        let val = range.denormalize(norm);
        self.set_value(val);
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
