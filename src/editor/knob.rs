use egui::{Align2, Color32, FontId, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Widget, vec2};
use std::f32::consts::PI;

use crate::utils::FloatParamNormalizedExt;

pub struct Knob<'a> {
    param: &'a truce::params::FloatParam,
    base_color: Color32,
}

impl<'a> Knob<'a> {
    pub fn new(param: &'a truce::params::FloatParam, color: Color32) -> Self {
        Self {
            param,
            base_color: color,
        }
    }

    fn get_dynamic_color(&self, visual_val: f32) -> Color32 {
        // ライトモードでは背景が明るいため、少し暗め/濃いめの色からスタートさせる
        let r = (self.base_color.r() as f32 * (0.8 + visual_val * 0.2)) as u8;
        let g = (self.base_color.g() as f32 * (0.8 + visual_val * 0.2)) as u8;
        let b = (self.base_color.b() as f32 * (0.8 + visual_val * 0.2)) as u8;

        if visual_val > 0.85 {
            // ピーク時のグロー効果（白背景なので白く光らせすぎず、色を濃くする）
            let boost = ((visual_val - 0.85) * 5.0 * 20.0) as u8;
            Color32::from_rgb(
                r.saturating_add(boost),
                g.saturating_sub(boost / 2), // 若干色相をシフトさせて「熱」を表現
                b.saturating_sub(boost / 2),
            )
        } else {
            Color32::from_rgb(r, g, b)
        }
    }
}

impl<'a> Widget for Knob<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let desired_size = vec2(62.0, 78.0);
        let (rect, response) = ui.allocate_at_least(desired_size, Sense::click_and_drag());

        let id = response.id;
        let text_edit_id = id.with("edit_mode");
        let edit_string_id = id.with("edit_str");

        if response.double_clicked() {
            self.param.set_value(self.param.info.default_plain);
        }

        if response.dragged() {
            let delta = -response.drag_delta().y * 0.006;
            let new_norm = (self.param.value_normalized() + delta as f64).clamp(0.0, 1.0);
            self.param.set_value_normalized(new_norm);
        }

        let visual_val = self.param.value_normalized() as f32;

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let active_color = self.get_dynamic_color(visual_val);

            // エリア分割
            let title_h = 14.0;
            let value_h = 16.0;
            let title_rect = Rect::from_min_size(rect.min, vec2(rect.width(), title_h));
            let value_rect = Rect::from_min_size(
                rect.max - vec2(rect.width(), value_h),
                vec2(rect.width(), value_h),
            );

            let knob_rect = Rect::from_center_size(
                rect.center() + vec2(0.0, -1.0),
                vec2(rect.width() * 0.8, rect.width() * 0.8),
            );

            // A. タイトル (白背景用に濃いグレーへ)
            painter.text(
                title_rect.center(),
                Align2::CENTER_CENTER,
                &self.param.info.name,
                FontId::proportional(10.0),
                Color32::from_gray(80), // 180 -> 80
            );

            // B. ノブ本体の描画
            let center = knob_rect.center();
            let radius = knob_rect.width() * 0.35; // 少しだけ余裕を持たせる
            let start_angle = PI * 0.8;
            let end_angle = PI * 2.2;
            let current_angle = start_angle + (visual_val * (end_angle - start_angle));

            // 背景溝 (ライトモードでは薄いグレー)
            painter.circle_stroke(
                center,
                radius + 4.0,
                Stroke::new(2.0, Color32::from_gray(210)),
            );

            // 円弧インジケーター
            let n_points = 24;
            let current_n = (n_points as f32 * visual_val).ceil() as usize;
            let arc_points: Vec<Pos2> = (0..=current_n)
                .map(|i| {
                    let a = start_angle + (i as f32 / n_points as f32) * (end_angle - start_angle);
                    center + vec2(a.cos(), a.sin()) * (radius + 4.0)
                })
                .collect();

            if arc_points.len() > 1 {
                painter.add(Shape::line(arc_points, Stroke::new(3.0, active_color)));
            }

            // ノブ本体キャップ (ライトモード向けに明るいグラデーション風のベタ塗り)
            painter.circle_filled(center, radius, Color32::from_gray(240)); // 本体
            painter.circle_stroke(center, radius, Stroke::new(1.0, Color32::from_gray(180))); // 輪郭

            // 指針（ドットではなく、はっきりした線）
            let tip = center + vec2(current_angle.cos(), current_angle.sin()) * (radius - 2.0);
            let base = center + vec2(current_angle.cos(), current_angle.sin()) * (radius * 0.3);
            painter.line_segment([base, tip], Stroke::new(2.5, active_color));

            // C. 数値表示 / エディット
            let is_editing =
                ui.memory(|mem| mem.data.get_temp::<bool>(text_edit_id).unwrap_or(false));

            if is_editing {
                let mut value_text = ui.memory(|mem| {
                    mem.data
                        .get_temp::<String>(edit_string_id)
                        .unwrap_or_else(|| format!("{:.1}", self.param.value()))
                });

                let res = ui.put(
                    value_rect.shrink2(vec2(4.0, 0.0)),
                    egui::TextEdit::singleline(&mut value_text)
                        .font(FontId::monospace(10.0))
                        .text_color(Color32::BLACK)
                        .horizontal_align(egui::Align::Center)
                        .margin(vec2(2.0, 0.0))
                        .frame(true), // 入力時は枠を出す
                );

                if res.changed() {
                    ui.memory_mut(|mem| mem.data.insert_temp(edit_string_id, value_text.clone()));
                }
                if res.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Ok(p) = value_text.parse::<f64>() {
                        self.param.set_value(p);
                    }
                    ui.memory_mut(|mem| mem.data.insert_temp(text_edit_id, false));
                } else {
                    res.request_focus();
                }
            } else {
                let val_res = ui.interact(value_rect, id.with("val_hit"), Sense::click());

                // 背景（白背景になじむ薄いグレー）
                painter.rect_filled(
                    value_rect.shrink2(vec2(4.0, 2.0)),
                    4.0,
                    Color32::from_gray(230),
                );

                painter.text(
                    value_rect.center(),
                    Align2::CENTER_CENTER,
                    format!("{:.1}", self.param.value()),
                    FontId::monospace(10.0),
                    active_color, // 数値の色もアクティブカラーに
                );

                if val_res.clicked() {
                    ui.memory_mut(|mem| mem.data.insert_temp(text_edit_id, true));
                }
            }
        }

        if response.dragged() {
            ui.ctx().request_repaint();
        }
        response
    }
}
