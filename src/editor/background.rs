use nih_plug_egui::egui::{
    self, Color32, Pos2,
    epaint::{Mesh, Shape, Vertex},
};

pub struct Background {}

impl Background {
    pub fn new() -> Self {
        Self {}
    }

    pub fn draw(&self, ui: &mut egui::Ui) {
        let rect = ui.max_rect();
        let painter = ui.painter();
        let time = ui.input(|i| i.time);

        // 1. ベース背景（深い紺色）
        painter.rect_filled(rect, 0.0, Color32::from_rgb(5, 5, 8));

        let center = rect.center();
        let t = time as f32;

        // 2. 虹色の「光の球」を描画
        for i in 0..12 {
            // 軌道計算
            let x_radius = rect.width() * 0.3;
            let y_radius = rect.height() * 0.2;
            let phase = i as f32 * (std::f32::consts::TAU / 12.0);

            let x = center.x
                + (t * 0.4 + phase).cos() * x_radius
                + (t * 0.6 + phase * 0.5).sin() * 50.0;
            let y = center.y
                + (t * 0.3 + phase).sin() * y_radius
                + (t * 0.5 + phase * 0.8).cos() * 30.0;

            let pos = Pos2::new(x, y);
            let hue = (i as f32 / 12.0 + t * 0.05) % 1.0;

            // HSVからRGBへ変換（彩度と輝度を調整して「発光感」を出す）
            let base_color = self.hsv_to_rgb(hue, 0.8, 0.2);
            let glow_radius = rect.width() * 0.25; // ぼかしの広がり範囲

            // メッシュを使って中心点から外周（透明）へグラデーションを作成
            self.draw_glow_circle(painter, pos, glow_radius, base_color);
        }

        // 毎フレーム再描画を要求
        ui.ctx().request_repaint();
    }

    /// 中心が色付き、外周が透明な円形メッシュを描画
    fn draw_glow_circle(&self, painter: &egui::Painter, center: Pos2, radius: f32, color: Color32) {
        let mut mesh = Mesh::default();
        let n_points = 24; // 円の分割数（滑らかさ）

        // 中心点（アルファ値を設定）
        let center_color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 40);
        let transparent = Color32::TRANSPARENT;

        let center_idx = mesh.vertices.len() as u32;
        mesh.vertices.push(Vertex {
            pos: center,
            uv: Pos2::ZERO,
            color: center_color,
        });

        // 外周の頂点を作成
        for k in 0..n_points {
            let angle = k as f32 * std::f32::consts::TAU / n_points as f32;
            let offset = egui::vec2(angle.cos(), angle.sin()) * radius;
            mesh.vertices.push(Vertex {
                pos: center + offset,
                uv: Pos2::ZERO,
                color: transparent,
            });

            // 三角形ポリゴンのインデックスを定義
            mesh.indices.push(center_idx);
            mesh.indices.push(center_idx + 1 + k);
            mesh.indices.push(center_idx + 1 + (k + 1) % n_points);
        }

        painter.add(Shape::Mesh(mesh.into()));
    }

    fn hsv_to_rgb(&self, h: f32, s: f32, v: f32) -> Color32 {
        let f = |n: f32| {
            let k = (n + h * 6.0) % 6.0;
            v - v * s * 0.0f32.max(1.0f32.min(k.min(4.0 - k)))
        };
        Color32::from_rgb(
            (f(5.0) * 255.0) as u8,
            (f(3.0) * 255.0) as u8,
            (f(1.0) * 255.0) as u8,
        )
    }
}
