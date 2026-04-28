use nih_plug_egui::egui::{self, Color32, Vec2};

pub struct Logo;

impl Logo {
    pub fn draw(ui: &mut egui::Ui, height: f32) {
        // 1. 初回のみSVGローダーをインストール
        egui_extras::install_image_loaders(ui.ctx());

        let image = egui::include_image!("../../assets/xross_logo.svg");

        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(height * 3.0, height), egui::Sense::hover());

        // 3. 描画
        ui.put(
            rect,
            egui::Image::new(image)
                .max_height(height)
                .tint(Color32::from_gray(40)), // ロゴ自体を少し暗めのグレーにして読みやすくする
        );
    }
}
