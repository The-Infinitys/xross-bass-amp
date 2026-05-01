use crate::utils::FloatParamNormalizedExt;
use egui::{Align2, Color32, FontId, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Widget, vec2};
use std::f32::consts::PI;

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

    /// StackedKnob / LinearSlider と共通の動的カラー計算
    fn get_dynamic_color(&self, visual_val: f32) -> Color32 {
        let r = (self.base_color.r() as f32 * (0.6 + visual_val * 0.4)) as u8;
        let g = (self.base_color.g() as f32 * (0.6 + visual_val * 0.4)) as u8;
        let b = (self.base_color.b() as f32 * (0.6 + visual_val * 0.4)) as u8;

        if visual_val > 0.85 {
            let boost = ((visual_val - 0.85) * 6.6 * 40.0) as u8;
            Color32::from_rgb(
                r.saturating_add(boost),
                g.saturating_add(boost),
                b.saturating_add(boost),
            )
        } else {
            Color32::from_rgb(r, g, b)
        }
    }
}

impl<'a> Widget for Knob<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        // --- 1. レイアウト定義 ---
        let desired_size = vec2(62.0, 120.0);
        let (rect, response) = ui.allocate_at_least(desired_size, Sense::click_and_drag());

        let id = response.id;
        let text_edit_id = id.with("edit_mode");
        let edit_string_id = id.with("edit_str");

        // インタラクション: ダブルクリックでデフォルト値
        if response.double_clicked() {
            self.param.set_value(self.param.info.default_plain);
        }

        // ドラッグ操作
        if response.dragged() {
            let delta = -response.drag_delta().y * 0.006;
            let new_norm = (self.param.value_normalized() + delta as f64).clamp(0.0, 1.0);
            self.param.set_value_normalized(new_norm);
        }

        let visual_val = self.param.value_normalized() as f32;
        let unit = self.param.info.unit.as_str();

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let active_color = self.get_dynamic_color(visual_val);

            // 中央配置計算
            let center = rect.center();
            let radius = rect.width() * 0.38;
            let start_angle = PI * 0.8;
            let end_angle = PI * 2.2;
            let current_angle = start_angle + (visual_val * (end_angle - start_angle));

            // A. タイトル
            painter.text(
                rect.center() + vec2(0.0, -44.0),
                Align2::CENTER_CENTER,
                self.param.info.name,
                FontId::proportional(10.0),
                Color32::from_gray(160),
            );

            // B. ノブ本体
            // 背景溝
            painter.circle_stroke(
                center,
                radius + 3.0,
                Stroke::new(1.5, Color32::from_gray(35)),
            );

            // 円弧インジケーター
            let n_points = 24;
            let current_n = (n_points as f32 * visual_val).ceil() as usize;
            let arc_points: Vec<Pos2> = (0..=current_n)
                .map(|i| {
                    let a = start_angle + (i as f32 / n_points as f32) * (end_angle - start_angle);
                    center + vec2(a.cos(), a.sin()) * (radius + 3.5)
                })
                .collect();

            if arc_points.len() > 1 {
                painter.add(Shape::line(arc_points, Stroke::new(2.5, active_color)));
            }

            // ノブ本体キャップ
            painter.circle_filled(center, radius, Color32::from_gray(20));

            // 指針 (針) - StackedKnobと同じスタイル
            let tip = center + vec2(current_angle.cos(), current_angle.sin()) * (radius - 1.0);
            let base = center + vec2(current_angle.cos(), current_angle.sin()) * 2.0;
            painter.line_segment([base, tip], Stroke::new(2.5, active_color));

            // C. 数値表示エリア
            let value_rect =
                Rect::from_center_size(center + vec2(0.0, radius + 20.0), vec2(rect.width(), 14.0));
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
                        .horizontal_align(egui::Align::Center)
                        .frame(false),
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

                // 数値表示の背景
                painter.rect_filled(
                    value_rect.shrink2(vec2(4.0, 1.0)),
                    2.0,
                    Color32::from_black_alpha(80),
                );

                // 値 + 単位
                let display_text = format!("{:.1}{}", self.param.value(), unit);
                painter.text(
                    value_rect.center(),
                    Align2::CENTER_CENTER,
                    display_text,
                    FontId::monospace(10.0),
                    Color32::from_gray(220)
                        .lerp_to_gamma(self.base_color, self.param.value_normalized() as f32),
                );

                if val_res.clicked() {
                    ui.memory_mut(|mem| {
                        mem.data.insert_temp(text_edit_id, true);
                        mem.data
                            .insert_temp(edit_string_id, format!("{:.1}", self.param.value()));
                    });
                }
            }
        }

        if response.dragged() {
            ui.ctx().request_repaint();
        }
        response
    }
}
