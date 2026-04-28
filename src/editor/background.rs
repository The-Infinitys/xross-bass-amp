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

        // 1. 背景はあえて少しだけ青みを入れたクリアな白
        painter.rect_filled(rect, 0.0, Color32::from_rgb(245, 245, 250));

        let center = rect.center();
        let t = time as f32;

        // 2. ビビッドなカラーパレット（高彩度）
        let colors = [
            Color32::from_rgb(255, 0, 100),  // ネオンマゼンタ
            Color32::from_rgb(0, 200, 255),  // シアンブルー
            Color32::from_rgb(255, 200, 0),  // ビビッドイエロー
            Color32::from_rgb(100, 255, 0),  // ライムグリーン
            Color32::from_rgb(150, 50, 255), // エレクトリックパープル
        ];

        for i in 0..12 {
            // 密度を上げるため個数はあえて絞り、一つを大きくする
            let seed = i as f32 * 847.12;

            // 画面をダイナミックに動く軌道
            let x_radius = rect.width() * 0.45;
            let y_radius = rect.height() * 0.4;

            let x = center.x + (t * 0.4 + seed).cos() * x_radius;
            let y = center.y + (t * 0.3 + seed * 1.5).sin() * y_radius;
            let pos = Pos2::new(x, y);

            let base_color = colors[i % colors.len()];

            // パルスに合わせて大きさを大胆に変化させる
            let pulse = (t * 0.6 + seed).sin().abs();
            let glow_radius = rect.width() * (0.3 + pulse * 0.2);

            // 乗算効果を出すため、アルファ値は「しっかり」かける（100〜180）
            let alpha = (pulse * 80.0) as u8;

            let color_with_alpha = Color32::from_rgba_unmultiplied(
                base_color.r(),
                base_color.g(),
                base_color.b(),
                alpha,
            );

            self.draw_multiply_circle(painter, pos, glow_radius, color_with_alpha);
        }

        ui.ctx().request_repaint();
    }

    fn draw_multiply_circle(
        &self,
        painter: &egui::Painter,
        center: Pos2,
        radius: f32,
        color: Color32,
    ) {
        let mut mesh = Mesh::default();
        let n_points = 40; // 滑らかな円

        let center_idx = mesh.vertices.len() as u32;

        // 中心を一番ビビッドに
        mesh.vertices.push(Vertex {
            pos: center,
            uv: Pos2::ZERO,
            color: color,
        });

        // 外側は「同じ色のまま透明」にすることで、白背景と綺麗に馴染ませる
        let edge_color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 0);

        for k in 0..n_points {
            let angle = k as f32 * std::f32::consts::TAU / n_points as f32;
            let offset = egui::vec2(angle.cos(), angle.sin()) * radius;
            mesh.vertices.push(Vertex {
                pos: center + offset,
                uv: Pos2::ZERO,
                color: edge_color,
            });

            mesh.indices.push(center_idx);
            mesh.indices.push(center_idx + 1 + k);
            mesh.indices.push(center_idx + 1 + (k + 1) % n_points);
        }

        painter.add(Shape::Mesh(mesh.into()));
    }
}
