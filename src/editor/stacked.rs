use crate::utils::FloatParamNormalizedExt;
use egui::{
    Align2, Color32, FontId, Id, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui,
    Widget, vec2,
};
use std::f32::consts::PI;

pub struct StackedKnob<'a> {
    upper_param: &'a truce::params::FloatParam, // Inner (中心部)
    lower_param: &'a truce::params::FloatParam, // Outer (外周部)
    upper_color: Color32,
    lower_color: Color32,
}

impl<'a> StackedKnob<'a> {
    pub fn new(
        upper_param: &'a truce::params::FloatParam,
        lower_param: &'a truce::params::FloatParam,
        upper_color: Color32,
        lower_color: Color32,
    ) -> Self {
        Self {
            upper_param,
            lower_param,
            upper_color,
            lower_color,
        }
    }
    fn get_dynamic_color(base_color: Color32, visual_val: f32) -> Color32 {
        let r = (base_color.r() as f32 * (0.8 + visual_val * 0.2)) as u8;
        let g = (base_color.g() as f32 * (0.8 + visual_val * 0.2)) as u8;
        let b = (base_color.b() as f32 * (0.8 + visual_val * 0.2)) as u8;

        if visual_val > 0.85 {
            let boost = ((visual_val - 0.85) * 6.6 * 30.0) as u8;
            Color32::from_rgb(
                r.saturating_add(boost),
                g.saturating_add(boost),
                b.saturating_add(boost),
            )
        } else {
            Color32::from_rgb(r, g, b)
        }
    }
    fn draw_arc(
        painter: &egui::Painter,
        center: Pos2,
        radius: f32,
        val: f32,
        color: Color32,
        thickness: f32,
    ) {
        let start_angle = PI * 0.8;
        let end_angle = PI * 2.2;
        if val > 0.0 {
            let n_points = 24;
            let current_n = (n_points as f32 * val).ceil() as usize;
            let points: Vec<Pos2> = (0..=current_n)
                .map(|i| {
                    let a = start_angle + (i as f32 / n_points as f32) * (end_angle - start_angle);
                    center + vec2(a.cos(), a.sin()) * radius
                })
                .collect();
            if points.len() > 1 {
                painter.add(Shape::line(points, Stroke::new(thickness, color)));
            }
        }
    }
    fn draw_value_display(
        &self,
        ui: &mut Ui,
        rect: Rect,
        p: &truce::params::FloatParam,
        edit_id: Id,
        color: Color32,
    ) -> bool {
        let is_editing = ui.memory(|m| m.data.get_temp::<bool>(edit_id).unwrap_or(false));
        let painter = ui.painter();
        let font_id = FontId::monospace(10.0);
        let unit = p.info.unit;

        if is_editing {
            let mut val_str = ui.memory(|m| {
                m.data
                    .get_temp::<String>(edit_id.with("s"))
                    .unwrap_or_else(|| format!("{:.1}", p.value()))
            });

            let res = ui.put(
                rect.shrink2(vec2(4.0, 0.0)),
                egui::TextEdit::singleline(&mut val_str)
                    .font(font_id)
                    .text_color(Color32::BLACK)
                    .horizontal_align(egui::Align::Center)
                    .frame(true), // ライトモードでは枠ありが見やすい
            );

            if res.changed() {
                ui.memory_mut(|m| m.data.insert_temp(edit_id.with("s"), val_str.clone()));
            }
            if res.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if let Ok(v) = val_str.parse::<f64>() {
                    p.set_value(v);
                }
                ui.memory_mut(|m| m.data.insert_temp(edit_id, false));
                return false;
            } else {
                res.request_focus();
            }
        } else {
            let res = ui.interact(rect, edit_id, Sense::click());

            // ライトモード用の数値背景
            painter.rect_filled(rect.shrink2(vec2(2.0, 1.0)), 2.0, Color32::from_gray(245));
            painter.rect_stroke(
                rect.shrink2(vec2(2.0, 1.0)),
                2.0,
                Stroke::new(1.0, Color32::from_gray(220)),
                StrokeKind::Middle,
            );

            // インジケータードット
            painter.circle_filled(rect.left_center() + vec2(8.0, 0.0), 2.5, color);

            let display_text = format!("{:.1}{}", p.value(), unit.as_str());
            painter.text(
                rect.center() + vec2(4.0, 0.0),
                Align2::CENTER_CENTER,
                display_text,
                font_id,
                Color32::from_gray(60).lerp_to_gamma(color, p.value_normalized() as f32 * 0.4),
            );

            if res.clicked() {
                ui.memory_mut(|m| m.data.insert_temp(edit_id, true));
            }
        }
        is_editing
    }
}

impl<'a> Widget for StackedKnob<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        // --- 1. レイアウト定義 ---
        let desired_size = vec2(62.0, 120.0);
        let (rect, response) = ui.allocate_at_least(desired_size, Sense::click_and_drag());

        let center = rect.center();
        let outer_radius = rect.width() * 0.38;
        let inner_radius = outer_radius * 0.6;

        let target_id = response.id.with("target");
        let mut active_target = ui.memory(|mem| mem.data.get_temp::<u8>(target_id).unwrap_or(0));

        // --- 2. インタラクション ---
        if response.drag_started() {
            let pos = response.interact_pointer_pos().unwrap_or(center);
            active_target = if pos.distance(center) <= inner_radius + 4.0 {
                1
            } else {
                2
            };
            ui.memory_mut(|mem| mem.data.insert_temp(target_id, active_target));
        }

        if response.double_clicked() {
            let pos = response.interact_pointer_pos().unwrap_or(center);
            let target = if pos.distance(center) <= inner_radius + 4.0 {
                self.upper_param
            } else {
                self.lower_param
            };
            target.set_value(target.info.default_plain);
        }

        if response.dragged() && active_target != 0 {
            let p = if active_target == 1 {
                self.upper_param
            } else {
                self.lower_param
            };
            let delta = -response.drag_delta().y * 0.006;
            let val = (p.value_normalized() + delta as f64).clamp(0.0, 1.0);
            p.set_value_normalized(val);
        }

        if response.drag_stopped() {
            ui.memory_mut(|mem| mem.data.insert_temp(target_id, 0u8));
        }

        // --- 3. 描画 ---
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let start_angle = PI * 0.8;
            let end_angle = PI * 2.2;

            // タイトル描画（操作中を濃い黒、非操作を薄いグレーに）
            let title_color = |idx| {
                if active_target == idx {
                    Color32::BLACK
                } else {
                    Color32::from_gray(140)
                }
            };

            painter.text(
                rect.center() + vec2(0.0, -48.0),
                Align2::CENTER_CENTER,
                self.upper_param.info.name,
                FontId::proportional(9.0),
                title_color(1),
            );
            painter.text(
                rect.center() + vec2(0.0, -39.0),
                Align2::CENTER_CENTER,
                self.lower_param.info.name,
                FontId::proportional(9.0),
                title_color(2),
            );

            let u_val = self.upper_param.value_normalized() as f32;
            let l_val = self.lower_param.value_normalized() as f32;
            let u_color = Self::get_dynamic_color(self.upper_color, u_val);
            let l_color = Self::get_dynamic_color(self.lower_color, l_val);

            // 背景溝 (ライトモード: 薄いグレー)
            painter.circle_stroke(
                center,
                outer_radius + 3.0,
                Stroke::new(1.5, Color32::from_gray(210)),
            );

            // Outer ノブ本体 (ライトモード: 明るいグレー)
            painter.circle_filled(center, outer_radius, Color32::from_gray(230));
            painter.circle_stroke(
                center,
                outer_radius,
                Stroke::new(1.0, Color32::from_gray(200)),
            );
            Self::draw_arc(painter, center, outer_radius + 3.5, l_val, l_color, 2.5);

            // Inner ノブ本体 (少し濃いグレーにして段差を強調)
            painter.circle_filled(center, inner_radius, Color32::from_gray(215));
            painter.circle_stroke(
                center,
                inner_radius,
                Stroke::new(1.0, Color32::from_gray(180)),
            );
            Self::draw_arc(painter, center, inner_radius + 2.5, u_val, u_color, 2.0);

            // --- 針 (Needle) ---
            let draw_needle = |val: f32, r_start: f32, r_end: f32, color: Color32, width: f32| {
                let ang = start_angle + val * (end_angle - start_angle);
                let p1 = center + vec2(ang.cos(), ang.sin()) * r_start;
                let p2 = center + vec2(ang.cos(), ang.sin()) * r_end;
                painter.line_segment([p1, p2], Stroke::new(width, color));
            };

            draw_needle(l_val, inner_radius + 3.0, outer_radius - 1.0, l_color, 2.5);
            draw_needle(u_val, 2.0, inner_radius - 1.0, u_color, 2.0);

            // 数値表示エリア (下部)
            let val_rect_u = Rect::from_center_size(
                center + vec2(0.0, outer_radius + 18.0),
                vec2(rect.width(), 14.0),
            );
            let val_rect_l = Rect::from_center_size(
                center + vec2(0.0, outer_radius + 34.0),
                vec2(rect.width(), 14.0),
            );

            let edit_u = self.draw_value_display(
                ui,
                val_rect_u,
                self.upper_param,
                response.id.with("ed_u"),
                u_color,
            );
            let edit_l = self.draw_value_display(
                ui,
                val_rect_l,
                self.lower_param,
                response.id.with("ed_l"),
                l_color,
            );

            if response.dragged() || edit_u || edit_l {
                ui.ctx().request_repaint();
            }
        }

        response
    }
}
