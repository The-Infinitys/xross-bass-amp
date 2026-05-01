use crate::utils::FloatParamNormalizedExt;
use egui::{Align2, Color32, FontId, Rect, Response, Sense, Stroke, Ui, Widget, vec2};

pub struct LinearSlider<'a> {
    param: &'a truce::params::FloatParam,
    color: Color32,
}

impl<'a> LinearSlider<'a> {
    pub fn new(param: &'a truce::params::FloatParam, color: Color32) -> Self {
        Self { param, color }
    }
}

impl<'a> Widget for LinearSlider<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let desired_size = vec2(120.0, 24.0);
        let (rect, response) = ui.allocate_at_least(desired_size, Sense::click_and_drag());

        let id = response.id;
        let text_edit_id = id.with("text_edit");
        let edit_string_id = id.with("edit_string");

        // 編集状態の取得
        let mut is_editing =
            ui.memory(|mem| mem.data.get_temp::<bool>(text_edit_id).unwrap_or(false));
        let text_rect = rect.scale_from_center(0.5);

        // --- インタラクション ---

        // 1. テキストエリアをクリックで編集モード
        let text_interaction = ui.interact(text_rect, id.with("text_area"), Sense::click());
        if text_interaction.clicked() && !is_editing {
            is_editing = true;
            ui.memory_mut(|mem| {
                mem.data.insert_temp(text_edit_id, true);
                mem.data
                    .insert_temp(edit_string_id, format!("{:.2}", self.param.value()));
            });
        }

        // 2. ダブルクリックでリセット（テキストエリア外）
        if response.double_clicked()
            && !text_rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default()))
        {
            self.param.set_value(self.param.info.default_plain);
            is_editing = false;
            ui.memory_mut(|mem| {
                mem.data.insert_temp(text_edit_id, false);
                mem.data.remove::<String>(edit_string_id);
            });
        }

        // 3. ドラッグによる値の変更（正規化トレイトを使用）
        if response.dragged() && !is_editing {
            let delta = (response.drag_delta().x / rect.width()) as f64;
            if delta != 0.0 {
                let current_norm = self.param.value_normalized();
                let new_norm = (current_norm + delta).clamp(0.0, 1.0);
                self.param.set_value_normalized(new_norm);
            }
        }

        // --- 描画 ---
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let visual_val = self.param.value_normalized() as f32;
            let bar_color = self.color.linear_multiply(0.6);

            // 背景
            painter.rect_filled(rect, 2.0, Color32::from_rgb(5, 5, 5));

            // 塗りつぶし (バー)
            let x_pos = rect.left() + (visual_val * rect.width());
            let fill_rect = Rect::from_min_max(rect.left_top(), egui::pos2(x_pos, rect.bottom()));
            painter.rect_filled(fill_rect, 1.0, bar_color);

            // 枠線
            painter.rect_stroke(
                rect,
                2.0,
                Stroke::new(1.0, Color32::from_gray(60)),
                egui::StrokeKind::Middle,
            );

            // ハンドル線
            let handle_x = x_pos.clamp(rect.left() + 1.0, rect.right() - 1.0);
            painter.line_segment(
                [
                    egui::pos2(handle_x, rect.top()),
                    egui::pos2(handle_x, rect.bottom()),
                ],
                Stroke::new(2.0, Color32::WHITE),
            );

            // テキスト表示・編集
            if is_editing {
                let mut value_text = ui.memory(|mem| {
                    mem.data
                        .get_temp::<String>(edit_string_id)
                        .unwrap_or_else(|| format!("{:.2}", self.param.value()))
                });

                let res = ui.put(
                    text_rect,
                    egui::TextEdit::singleline(&mut value_text)
                        .font(FontId::proportional(11.0))
                        .horizontal_align(egui::Align::Center)
                        .frame(false),
                );

                if res.changed() {
                    ui.memory_mut(|mem| mem.data.insert_temp(edit_string_id, value_text.clone()));
                }

                if res.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Ok(parsed) = value_text.parse::<f64>() {
                        self.param.set_value(parsed);
                    }
                    ui.memory_mut(|mem| {
                        mem.data.insert_temp(text_edit_id, false);
                        mem.data.remove::<String>(edit_string_id);
                    });
                } else {
                    res.request_focus();
                }
            } else {
                let display_text = format!(
                    "{}: {:.1} {}",
                    self.param.info.name,
                    self.param.value(),
                    self.param.info.unit.as_str()
                );
                let font_id = FontId::proportional(11.0);
                let text_pos = rect.center();

                // シャドウ
                painter.text(
                    text_pos + vec2(1.0, 1.0),
                    Align2::CENTER_CENTER,
                    &display_text,
                    font_id.clone(),
                    Color32::from_black_alpha(200),
                );
                // 通常文字 (背景部分)
                painter.text(
                    text_pos,
                    Align2::CENTER_CENTER,
                    &display_text,
                    font_id.clone(),
                    Color32::from_gray(180),
                );
                // 反転文字 (バーに重なっている部分)
                painter.with_clip_rect(fill_rect).text(
                    text_pos,
                    Align2::CENTER_CENTER,
                    &display_text,
                    font_id,
                    Color32::WHITE,
                );
            }
        }

        if response.dragged() || is_editing {
            ui.ctx().request_repaint();
        }

        response
    }
}
