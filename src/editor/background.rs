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

        // 1. ベース背景（清潔感のあるオフホワイト）
        painter.rect_filled(rect, 0.0, Color32::from_rgb(248, 248, 252));

        let center = rect.center();
        let t = time as f32;

        // 2. 「舞う光」の描画（20個のパーティクル）
        for i in 0..20 {
            let seed = i as f32 * 123.456;
            
            // 複雑で躍動感のある動き
            let x_radius = rect.width() * (0.3 + (t * 0.1 + seed).cos() * 0.1);
            let y_radius = rect.height() * (0.25 + (t * 0.15 + seed).sin() * 0.1);
            
            let phase_x = t * (0.3 + (i % 3) as f32 * 0.1) + seed;
            let phase_y = t * (0.2 + (i % 2) as f32 * 0.15) + seed * 1.5;

            let x = center.x + phase_x.cos() * x_radius + (t * 2.0 + seed).sin() * 30.0;
            let y = center.y + phase_y.sin() * y_radius + (t * 1.5 + seed).cos() * 20.0;

            let pos = Pos2::new(x, y);
            
            // シルバー、ライトブルー、ソフトゴールドの輝き
            let base_color = match i % 4 {
                0 => Color32::from_rgb(200, 200, 230), // Silver
                1 => Color32::from_rgb(170, 210, 255), // Cyan blue
                2 => Color32::from_rgb(220, 230, 255), // Very light blue
                _ => Color32::from_rgb(255, 250, 220), // Soft gold
            };
            
            // 個別に明滅
            let pulse = (t * 0.5 + seed).sin().abs();
            let glow_radius = rect.width() * (0.15 + pulse * 0.2);
            let alpha = (10.0 + pulse * 15.0) as u8;
            
            let color_with_alpha = Color32::from_rgba_unmultiplied(
                base_color.r(), base_color.g(), base_color.b(), alpha
            );

            self.draw_glow_circle(painter, pos, glow_radius, color_with_alpha);
        }

        // 毎フレーム再描画
        ui.ctx().request_repaint();
    }

    fn draw_glow_circle(&self, painter: &egui::Painter, center: Pos2, radius: f32, color: Color32) {
        let mut mesh = Mesh::default();
        let n_points = 24;

        let center_color = color;
        let transparent = Color32::TRANSPARENT;

        let center_idx = mesh.vertices.len() as u32;
        mesh.vertices.push(Vertex {
            pos: center,
            uv: Pos2::ZERO,
            color: center_color,
        });

        for k in 0..n_points {
            let angle = k as f32 * std::f32::consts::TAU / n_points as f32;
            let offset = egui::vec2(angle.cos(), angle.sin()) * radius;
            mesh.vertices.push(Vertex {
                pos: center + offset,
                uv: Pos2::ZERO,
                color: transparent,
            });

            mesh.indices.push(center_idx);
            mesh.indices.push(center_idx + 1 + k);
            mesh.indices.push(center_idx + 1 + (k + 1) % n_points);
        }

        painter.add(Shape::Mesh(mesh.into()));
    }
}