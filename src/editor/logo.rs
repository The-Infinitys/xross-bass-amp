use nih_plug_egui::egui::{self, Ui, Vec2};

pub struct Logo;

impl Logo {
    pub fn draw(ui: &mut Ui, height: f32) {
        // 1. 初回のみSVGローダーをインストール（必須）
        egui_extras::install_image_loaders(ui.ctx());

        // 2. SVGをロード
        // assetsフォルダなどに置いている場合は "file://..." や include_image! を使います
        // 今回は「Xross」のSVGということなので、include_image! でコンパイル時に埋め込むのが楽です
        let image = egui::include_image!("../../assets/xross_logo.svg");

        // 3. 描画
        ui.add(
            egui::Image::new(image)
                .max_height(height)
                .fit_to_exact_size(Vec2::new(height * 3.0, height)), // 比率はSVGに合わせて調整してください
        );
    }
}
