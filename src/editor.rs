use egui::{self, Color32, Frame, NumExt, RichText, UiBuilder, Vec2, ecolor::Hsva};
use std::sync::Arc;
use truce::core::Editor;
use truce_egui::EguiEditor;

use crate::params::XrossBassAmpParams;
mod background;
mod knob;
mod linear;
mod logo;
mod speaker;

use background::Background;
use knob::Knob;
use linear::LinearSlider;
use logo::Logo;
use speaker::SpeakerVisualizer;

/// 鮮やかな虹色を生成（各セクションのノブに一貫性を待たせる）
fn get_vibrant_rainbow_color(index: usize, total: usize) -> Color32 {
    let h = (index as f32 / total as f32) * 0.85;
    Hsva::new(h, 1.0, 1.0, 1.0).into()
}

pub fn create_editor(params: Arc<XrossBassAmpParams>) -> Box<dyn Editor> {
    let width = 910;
    let height = 620;
    let bg = Background::new();

    let editor = EguiEditor::new((width, height), move |egui_ctx, _state| {
        egui::CentralPanel::default()
            .frame(Frame::NONE)
            .show(egui_ctx, |ui| {
                ui.set_max_width(width as f32);
                ui.set_max_height(height as f32);

                bg.draw(ui);

                let mut color_idx = 0;
                let total_knobs = 13; // Limiter分を追加

                let container_rect = ui.max_rect().shrink2(Vec2::new(12.0, 10.0));

                ui.allocate_new_ui(UiBuilder::new().max_rect(container_rect), |ui| {
                    ui.vertical(|ui| {
                        // --- ヘッダー ---
                        ui.vertical_centered(|ui| {
                            Logo::draw(ui, 28.0);
                        });
                        ui.add_space(2.0);

                        // --- 上段: アンプヘッドセクション ---
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 3.0;

                            // 1. GAIN & DRIVE
                            draw_section_weighted(ui, "GAIN / DRIVE", 2.0, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 2.0;
                                    for k in [
                                        &params.input_gain,
                                        &params.drive,
                                        &params.blend,
                                        &params.master_gain,
                                    ] {
                                        ui.add(Knob::new(
                                            k,
                                            get_vibrant_rainbow_color(color_idx, total_knobs),
                                        ));
                                        color_idx += 1;
                                    }
                                });
                            });

                            // 2. DYNAMICS & EQ
                            draw_section_weighted(ui, "DYNAMICS & EQ", 4.5, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 2.0;
                                    for k in [
                                        &params.compression,
                                        &params.sub_low,
                                        &params.low,
                                        &params.mid,
                                        &params.high,
                                        &params.presence,
                                    ] {
                                        ui.add(Knob::new(
                                            k,
                                            get_vibrant_rainbow_color(color_idx, total_knobs),
                                        ));
                                        color_idx += 1;
                                    }
                                });
                            });

                            // 3. UTILITY
                            draw_section_weighted(ui, "UTILITY", 4.0, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 2.0;
                                    // TIGHT, SAG に加え、LIMITER もノブとして配置
                                    for k in [&params.tight, &params.sag, &params.limiter_on] {
                                        ui.add(Knob::new(
                                            k,
                                            get_vibrant_rainbow_color(color_idx, total_knobs),
                                        ));
                                        color_idx += 1;
                                    }
                                });
                            });
                        });

                        ui.add_space(6.0);

                        // --- 下段: キャビネットセクション ---
                        let cab_height = ui.available_height();
                        draw_section_with_height(
                            ui,
                            "CABINET & DUAL MICROPHONES",
                            cab_height,
                            |ui| {
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 15.0;

                                        // MIC A (Dynamic)
                                        ui.vertical(|ui| {
                                            ui.label(
                                                egui::RichText::new("MIC A")
                                                    .color(Color32::from_rgb(0, 180, 255))
                                                    .strong()
                                                    .size(10.0),
                                            );
                                            ui.add(LinearSlider::new(
                                                &params.mic_a_axis,
                                                Color32::from_rgb(0, 180, 255),
                                            ));
                                            ui.add(LinearSlider::new(
                                                &params.mic_a_distance,
                                                Color32::from_rgb(0, 180, 255),
                                            ));
                                        });

                                        // MIC B (Ribbon)
                                        ui.vertical(|ui| {
                                            ui.label(
                                                egui::RichText::new("MIC B")
                                                    .color(Color32::from_rgb(255, 100, 0))
                                                    .strong()
                                                    .size(10.0),
                                            );
                                            ui.add(LinearSlider::new(
                                                &params.mic_b_axis,
                                                Color32::from_rgb(255, 100, 0),
                                            ));
                                            ui.add(LinearSlider::new(
                                                &params.mic_b_distance,
                                                Color32::from_rgb(255, 100, 0),
                                            ));
                                        });

                                        // CAB SETTINGS
                                        ui.vertical(|ui| {
                                            ui.label(
                                                egui::RichText::new("CAB SETTINGS")
                                                    .strong()
                                                    .size(10.0),
                                            );

                                            // スピーカー数（ここだけボタン形式を維持）
                                            ui.horizontal(|ui| {
                                                ui.spacing_mut().item_spacing.x = 4.0;
                                                for &count in &[1, 2, 4, 8, 16] {
                                                    let is_selected =
                                                        params.speaker_count.value() == count;
                                                    if ui
                                                        .selectable_label(
                                                            is_selected,
                                                            RichText::new(count.to_string())
                                                                .color(Color32::BLACK),
                                                        )
                                                        .clicked()
                                                    {
                                                        params.speaker_count.set_value(count);
                                                    }
                                                }
                                            });

                                            ui.add_space(4.0);

                                            ui.horizontal(|ui| {
                                                ui.add(LinearSlider::new(
                                                    &params.cab_bypass,
                                                    Color32::RED,
                                                ));
                                                ui.add(LinearSlider::new(
                                                    &params.speaker_size,
                                                    Color32::GOLD,
                                                ));
                                                ui.add(LinearSlider::new(
                                                    &params.room_mix,
                                                    Color32::WHITE,
                                                ));
                                            });
                                        });
                                    });

                                    ui.add_space(8.0);
                                    ui.separator();
                                    ui.add_space(4.0);

                                    // 下段：ビジュアライザー
                                    ui.vertical_centered(|ui| {
                                        let visualizer_area = ui.available_height() - 5.0;
                                        SpeakerVisualizer::new(&params)
                                            .draw(ui, visualizer_area.at_most(280.0));
                                    });
                                });
                            },
                        );
                    });
                });
            });
    });
    Box::new(editor)
}

fn draw_section_weighted(
    ui: &mut egui::Ui,
    title: &str,
    weight: f32,
    add_contents: impl FnMut(&mut egui::Ui),
) {
    let total_weight = 10.5;
    let spacing = ui.spacing().item_spacing.x;
    let available_width = (ui.available_width() - (spacing * 2.0)).max(0.0);
    let width = (available_width * (weight / total_weight)).floor();

    ui.allocate_ui(Vec2::new(width, 0.0), |ui| {
        draw_section_with_height(ui, title, 0.0, add_contents);
    });
}

fn draw_section_with_height(
    ui: &mut egui::Ui,
    title: &str,
    height: f32,
    mut add_contents: impl FnMut(&mut egui::Ui),
) {
    Frame::NONE
        .fill(Color32::from_white_alpha(140))
        .stroke(egui::Stroke::new(1.0, Color32::from_gray(180)))
        .corner_radius(6.0)
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.set_min_height(height);
            ui.set_width(ui.available_width());
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(title)
                        .strong()
                        .color(Color32::from_gray(220))
                        .size(10.0),
                );
                ui.add_space(6.0);
                add_contents(ui);
            });
        });
}
