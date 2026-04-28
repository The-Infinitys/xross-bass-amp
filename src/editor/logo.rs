use nih_plug_egui::egui::{self, Color32, Vec2};

pub struct Logo;

impl Logo {
    pub fn draw(ui: &mut egui::Ui, height: f32) {
        // 1. 初回のみSVGローダーをインストール
        egui_extras::install_image_loaders(ui.ctx());

        let image = egui::include_image!("../../assets/xross_logo.svg");

        // 2. 視認性向上のための工夫
        // 背後に非常に薄い影（グロー）を配置して、白背景でもロゴの輪郭をはっきりさせる
        let (rect, _) = ui.allocate_exact_size(Vec2::new(height * 3.0, height), egui::Sense::hover());
        
        let painter = ui.painter();
        // わずかに暗いソフトな影を背後に描画
        painter.rect_filled(
            rect.expand(4.0),
            10.0,
            Color32::from_black_alpha(10), // 非常に薄い黒
        );

        // 3. 描画
        ui.put(
            rect,
            egui::Image::new(image)
                .max_height(height)
                .tint(Color32::from_gray(40)) // ロゴ自体を少し暗めのグレーにして読みやすくする
        );
    }
}
