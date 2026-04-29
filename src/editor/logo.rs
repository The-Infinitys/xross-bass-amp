use egui::{self, Ui, Vec2};

pub struct Logo;

impl Logo {
    pub fn draw(ui: &mut Ui, height: f32) {
        // 1. 初回のみSVGローダーをインストール（必須）
        egui_extras::install_image_loaders(ui.ctx());

        // 2. SVGをロード
        let image = egui::include_image!("../../assets/xross_logo.svg");

        // 3. 描画
        ui.add(
            egui::Image::new(image)
                .max_height(height)
                .fit_to_exact_size(Vec2::new(height * 3.0, height)),
        );
    }
}
