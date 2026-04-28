use nih_plug::params::FloatParam;
use nih_plug::prelude::*;
use nih_plug_egui::egui::{self, StrokeKind};

pub struct LinearSlider<'a> {
    param: &'a FloatParam,
    setter: &'a ParamSetter<'a>,
    color: egui::Color32,
}

impl<'a> LinearSlider<'a> {
    pub fn new(param: &'a FloatParam, setter: &'a ParamSetter<'a>, color: egui::Color32) -> Self {
        Self {
            param,
            setter,
            color,
        }
    }
}

impl<'a> egui::Widget for LinearSlider<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let desired_size = egui::vec2(120.0, 24.0);
        let (rect, response) = ui.allocate_at_least(desired_size, egui::Sense::click_and_drag());

        let id = response.id;
        let text_edit_id = id.with("text_edit");
        let edit_string_id = id.with("edit_string");

        // メモリから編集状態を取得
        let mut is_editing_text =
            ui.memory(|mem| mem.data.get_temp::<bool>(text_edit_id).unwrap_or(false));

        let text_rect = rect.shrink(2.0); // テキスト表示領域

        // ====================== インタラクション処理 ======================
        // 1. テキスト領域以外で右クリック → リセット
        if response.secondary_clicked() {
            self.setter.begin_set_parameter(self.param);
            self.setter
                .set_parameter_normalized(self.param, self.param.default_normalized_value());
            self.setter.end_set_parameter(self.param);

            is_editing_text = false;
            ui.memory_mut(|mem| {
                mem.data.insert_temp(text_edit_id, false);
                mem.data.remove::<String>(edit_string_id);
            });
        }

        // 2. テキスト領域をクリック → 編集モード
        let text_interaction = ui.interact(text_rect, id.with("text_area"), egui::Sense::click());
        if text_interaction.clicked() && !is_editing_text {
            is_editing_text = true;
            ui.memory_mut(|mem| {
                mem.data.insert_temp(text_edit_id, true);
                // 初期値として現在の値を入れる
                mem.data
                    .insert_temp(edit_string_id, format!("{:.2}", self.param.value()));
            });
        }

        // 3. ドラッグ操作（編集モード中は無効）
        if response.drag_started() && !is_editing_text {
            self.setter.begin_set_parameter(self.param);
        }

        if response.dragged() && !is_editing_text {
            let val = self.param.unmodulated_normalized_value();
            let delta = response.drag_delta().x / rect.width(); // 横スライダー前提
            if delta != 0.0 {
                let new_val = (val + delta).clamp(0.0, 1.0);
                self.setter.set_parameter_normalized(self.param, new_val);
            }
        }

        if response.drag_stopped() && !is_editing_text {
            self.setter.end_set_parameter(self.param);
        }

        // ====================== 描画 ======================
        if ui.is_rect_visible(rect) {
            let visual_val = self.param.unmodulated_normalized_value();
            let bar_color = self.color.linear_multiply(0.6);

            // 背景とバー
            let painter = ui.painter();
            painter.rect_filled(rect, 2.0, egui::Color32::from_rgb(5, 5, 5));

            let fill_rect = {
                let x_pos = rect.left() + (visual_val * rect.width());
                egui::Rect::from_min_max(rect.left_top(), egui::pos2(x_pos, rect.bottom()))
            };
            painter.rect_filled(fill_rect, 1.0, bar_color);

            painter.rect_stroke(
                rect,
                2.0,
                egui::Stroke::new(1.0, egui::Color32::from_gray(60)),
                StrokeKind::Middle,
            );

            // ハンドル
            let handle_x = (rect.left() + visual_val * rect.width())
                .clamp(rect.left() + 1.0, rect.right() - 1.0);
            let handle_rect = egui::Rect::from_center_size(
                egui::pos2(handle_x, rect.center().y),
                egui::vec2(2.0, rect.height()),
            );
            painter.rect_filled(handle_rect, 0.0, egui::Color32::WHITE);

            // テキスト描画
            if is_editing_text {
                let mut value_text = ui.memory(|mem| {
                    mem.data
                        .get_temp::<String>(edit_string_id)
                        .unwrap_or_else(|| format!("{:.2}", self.param.value()))
                });

                let output = ui.put(
                    text_rect,
                    egui::TextEdit::singleline(&mut value_text)
                        .font(egui::FontId::proportional(11.0))
                        .text_color(egui::Color32::WHITE)
                        .horizontal_align(egui::Align::Center)
                        .frame(false),
                );

                if output.changed() {
                    ui.memory_mut(|mem| mem.data.insert_temp(edit_string_id, value_text.clone()));
                }

                if output.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Ok(parsed) = value_text.parse::<f32>() {
                        let norm_val = self.param.preview_normalized(parsed);
                        self.setter.begin_set_parameter(self.param);
                        self.setter.set_parameter_normalized(self.param, norm_val);
                        self.setter.end_set_parameter(self.param);
                    }
                    is_editing_text = false;
                    ui.memory_mut(|mem| {
                        mem.data.insert_temp(text_edit_id, false);
                        mem.data.remove::<String>(edit_string_id);
                    });
                } else {
                    output.request_focus();
                }
            } else {
                // 通常テキスト
                let text = format!("{}: {}", self.param.name(), self.param);
                let font_id = egui::FontId::proportional(11.0);
                let text_pos = rect.center();

                painter.text(
                    text_pos + egui::vec2(1.0, 1.0),
                    egui::Align2::CENTER_CENTER,
                    &text,
                    font_id.clone(),
                    egui::Color32::from_black_alpha(200),
                );
                painter.text(
                    text_pos,
                    egui::Align2::CENTER_CENTER,
                    &text,
                    font_id.clone(),
                    egui::Color32::from_gray(180),
                );
                painter.with_clip_rect(fill_rect).text(
                    text_pos,
                    egui::Align2::CENTER_CENTER,
                    &text,
                    font_id,
                    egui::Color32::WHITE,
                );
            }
        }

        if response.dragged() || is_editing_text {
            ui.ctx().request_repaint();
        }

        response
    }
}
