use nih_plug::params::FloatParam;
use nih_plug::prelude::*;
use nih_plug_egui::egui;
use std::f32::consts::PI;

pub struct Knob<'a> {
    param: &'a FloatParam,
    setter: &'a ParamSetter<'a>,
    base_color: egui::Color32,
}

impl<'a> Knob<'a> {
    pub fn new(param: &'a FloatParam, setter: &'a ParamSetter<'a>, color: egui::Color32) -> Self {
        Self {
            param,
            setter,
            base_color: color,
        }
    }

    /// 値に応じて色を動的に変化させる（高い値ほど鮮やかに）
    fn get_dynamic_color(&self, visual_val: f32) -> egui::Color32 {
        let r = (self.base_color.r() as f32 * (0.5 + visual_val * 0.5)) as u8;
        let g = (self.base_color.g() as f32 * (0.5 + visual_val * 0.5)) as u8;
        let b = (self.base_color.b() as f32 * (0.5 + visual_val * 0.5)) as u8;

        if visual_val > 0.8 {
            // 最大値付近で少し白っぽく「発光」させる
            let boost = ((visual_val - 0.8) * 5.0 * 50.0) as u8;
            egui::Color32::from_rgb(
                r.saturating_add(boost),
                g.saturating_add(boost),
                b.saturating_add(boost),
            )
        } else {
            egui::Color32::from_rgb(r, g, b)
        }
    }
}

impl<'a> egui::Widget for Knob<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        // --- 1. レイアウト計算 ---
        let desired_size = egui::vec2(80.0, 110.0);
        let (rect, response) = ui.allocate_at_least(desired_size, egui::Sense::click_and_drag());

        let id = response.id;
        let text_edit_id = id.with("text_edit");
        let edit_string_id = id.with("edit_string");

        // インタラクション
        if response.double_clicked() {
            let default_val = self.param.default_normalized_value();
            self.setter.begin_set_parameter(self.param);
            self.setter
                .set_parameter_normalized(self.param, default_val);
            self.setter.end_set_parameter(self.param);
            ui.memory_mut(|mem| mem.data.insert_temp(text_edit_id, false));
        }

        if response.drag_started() {
            self.setter.begin_set_parameter(self.param);
        }

        let visual_val = if response.dragged() {
            let mut val = ui
                .memory(|mem| mem.data.get_temp(id))
                .unwrap_or_else(|| self.param.unmodulated_normalized_value());
            let delta = -response.drag_delta().y * 0.005;
            val = (val + delta).clamp(0.0, 1.0);
            self.setter.set_parameter_normalized(self.param, val);
            ui.memory_mut(|mem| mem.data.insert_temp(id, val));
            val
        } else {
            self.param.unmodulated_normalized_value()
        };

        if response.drag_stopped() {
            self.setter.end_set_parameter(self.param);
        }

        // --- 2. 描画 ---
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let active_color = self.get_dynamic_color(visual_val);

            // 各セクションの矩形
            let title_rect =
                egui::Rect::from_min_max(rect.min, rect.min + egui::vec2(rect.width(), 20.0));
            let value_rect =
                egui::Rect::from_min_max(rect.max - egui::vec2(rect.width(), 25.0), rect.max);

            // 中央の正方形ノブ領域
            let knob_area_width = rect.width().min(rect.height() - 45.0);
            let knob_rect = egui::Rect::from_center_size(
                rect.center() + egui::vec2(0.0, -2.0),
                egui::Vec2::splat(knob_area_width),
            );

            // A. タイトル表示
            painter.text(
                title_rect.center(),
                egui::Align2::CENTER_CENTER,
                self.param.name(),
                egui::FontId::proportional(13.0),
                egui::Color32::from_gray(200),
            );

            // B. ノブ本体
            let center = knob_rect.center();
            let radius = knob_rect.width() * 0.4;
            let start_angle = PI * 0.75;
            let end_angle = PI * 2.25;
            let current_angle = start_angle + (visual_val * (end_angle - start_angle));

            // 背景（溝）
            painter.circle_filled(center, radius + 2.0, egui::Color32::BLACK);
            painter.circle_stroke(
                center,
                radius + 4.0,
                egui::Stroke::new(2.0, egui::Color32::from_gray(40)),
            );

            // 円弧インジケーター
            let n_points = 32;
            let current_n = (n_points as f32 * visual_val).ceil() as usize;
            let arc_points: Vec<egui::Pos2> = (0..=current_n)
                .map(|i| {
                    let a = start_angle + (i as f32 / n_points as f32) * (end_angle - start_angle);
                    center + egui::vec2(a.cos(), a.sin()) * (radius + 6.0)
                })
                .collect();

            if arc_points.len() > 1 {
                painter.add(egui::Shape::line(
                    arc_points,
                    egui::Stroke::new(3.0, active_color),
                ));
            }

            // ノブ本体と指針
            painter.circle_filled(center, radius, egui::Color32::from_gray(25));
            let tip = center + egui::vec2(current_angle.cos(), current_angle.sin()) * radius;
            let base =
                center + egui::vec2(current_angle.cos(), current_angle.sin()) * (radius * 0.4);
            painter.line_segment([base, tip], egui::Stroke::new(3.0, active_color));

            // C. 数値表示エリア
            let is_editing =
                ui.memory(|mem| mem.data.get_temp::<bool>(text_edit_id).unwrap_or(false));

            if is_editing {
                let mut value_text = ui.memory(|mem| {
                    mem.data
                        .get_temp::<String>(edit_string_id)
                        .unwrap_or_else(|| format!("{:.1}", self.param.value()))
                });

                let res = ui.put(
                    value_rect,
                    egui::TextEdit::singleline(&mut value_text)
                        .font(egui::FontId::proportional(12.0))
                        .horizontal_align(egui::Align::Center)
                        .frame(true),
                );

                if res.changed() {
                    ui.memory_mut(|mem| mem.data.insert_temp(edit_string_id, value_text.clone()));
                }
                if res.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Ok(parsed) = value_text.parse::<f32>() {
                        self.setter.begin_set_parameter(self.param);
                        self.setter.set_parameter_normalized(
                            self.param,
                            self.param.preview_normalized(parsed),
                        );
                        self.setter.end_set_parameter(self.param);
                    }
                    ui.memory_mut(|mem| mem.data.insert_temp(text_edit_id, false));
                } else {
                    res.request_focus();
                }
            } else {
                let val_res =
                    ui.interact(value_rect, id.with("val_interact"), egui::Sense::click());
                painter.rect_filled(
                    value_rect.shrink(2.0),
                    4.0,
                    egui::Color32::from_black_alpha(100),
                );
                painter.text(
                    value_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    self.param.to_string(),
                    egui::FontId::monospace(12.0),
                    active_color,
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
