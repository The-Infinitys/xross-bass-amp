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

        let mut is_editing_text =
            ui.memory(|mem| mem.data.get_temp::<bool>(text_edit_id).unwrap_or(false));

        let text_rect = rect.shrink(2.0);

        // ====================== インタラクション処理 ======================
        if response.secondary_clicked() {
            self.param.set_value(self.param.info.default_plain);
            is_editing_text = false;
            ui.memory_mut(|mem| {
                mem.data.insert_temp(text_edit_id, false);
                mem.data.remove::<String>(edit_string_id);
            });
        }

        let text_interaction = ui.interact(text_rect, id.with("text_area"), Sense::click());
        if text_interaction.clicked() && !is_editing_text {
            is_editing_text = true;
            ui.memory_mut(|mem| {
                mem.data.insert_temp(text_edit_id, true);
                mem.data
                    .insert_temp(edit_string_id, format!("{:.2}", self.param.value()));
            });
        }

        if response.dragged() && !is_editing_text {
            let val = self.param.value_normalized();
            let delta = (response.drag_delta().x / rect.width()) as f64;
            if delta != 0.0 {
                let new_val = (val + delta).clamp(0.0, 1.0);
                self.param.set_value_normalized(new_val);
            }
        }

        // ====================== 描画 ======================
        if ui.is_rect_visible(rect) {
            let visual_val = self.param.value_normalized() as f32;
            // ライトモード用に彩度を少し維持
            let bar_color = self.color.gamma_multiply(0.8);

            let painter = ui.painter();

            // 1. スライダー背景（溝の表現）
            painter.rect_filled(rect, 2.0, Color32::from_gray(225));
            painter.rect_stroke(
                rect,
                2.0,
                Stroke::new(1.0, Color32::from_gray(190)),
                egui::StrokeKind::Inside,
            );

            // 2. プログレスバー（塗りつぶし）
            let fill_rect = {
                let x_pos = rect.left() + (visual_val * rect.width());
                Rect::from_min_max(rect.left_top(), egui::pos2(x_pos, rect.bottom()))
            };
            if visual_val > 0.0 {
                painter.rect_filled(fill_rect, 2.0, bar_color);
            }

            // 3. ハンドル（垂直線）
            let handle_x = (rect.left() + visual_val * rect.width())
                .clamp(rect.left() + 1.0, rect.right() - 1.0);
            let handle_rect = Rect::from_center_size(
                egui::pos2(handle_x, rect.center().y),
                vec2(3.0, rect.height() + 2.0), // 少し上下にはみ出させて視認性アップ
            );
            painter.rect_filled(handle_rect, 1.0, Color32::from_gray(80));

            // 4. テキスト描画
            if is_editing_text {
                let mut value_text = ui.memory(|mem| {
                    mem.data
                        .get_temp::<String>(edit_string_id)
                        .unwrap_or_else(|| format!("{:.2}", self.param.value()))
                });

                let output = ui.put(
                    text_rect,
                    egui::TextEdit::singleline(&mut value_text)
                        .font(FontId::proportional(11.0))
                        .text_color(Color32::BLACK) // 入力中は黒
                        .horizontal_align(egui::Align::Center)
                        .frame(false),
                );

                if output.changed() {
                    ui.memory_mut(|mem| mem.data.insert_temp(edit_string_id, value_text.clone()));
                }

                if output.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Ok(parsed) = value_text.parse::<f64>() {
                        self.param.set_value(parsed);
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
                let text = format!("{}: {:.1}", self.param.info.name, self.param.value());
                let font_id = FontId::proportional(10.5); // 少しだけ小さくして余白を確保
                let text_pos = rect.center();

                // 未充填部分のテキスト（濃いグレー）
                painter.text(
                    text_pos,
                    Align2::CENTER_CENTER,
                    &text,
                    font_id.clone(),
                    Color32::from_gray(60),
                );

                // バーに重なっている部分のテキスト（白抜き）
                painter.with_clip_rect(fill_rect).text(
                    text_pos,
                    Align2::CENTER_CENTER,
                    &text,
                    font_id,
                    Color32::WHITE,
                );
            }
        }

        if response.dragged() || is_editing_text {
            ui.ctx().request_repaint();
        }

        response
    }
}
