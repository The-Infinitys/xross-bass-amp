use nih_plug::prelude::*;
use nih_plug_egui::{
    EguiState, create_egui_editor,
    egui::{self, Color32, Frame, UiBuilder, Vec2, ecolor::Hsva},
};
use std::sync::Arc;

use crate::params::XrossGuitarAmpParams;
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

fn get_vibrant_rainbow_color(index: usize, total: usize) -> Color32 {
    let h = (index as f32 / total as f32) * 0.85;
    Hsva::new(h, 1.0, 1.0, 1.0).into()
}

pub fn create_editor(params: Arc<XrossGuitarAmpParams>) -> Option<Box<dyn Editor>> {
    let width = 1280;
    let height = 900;
    let bg = Background::new();

    create_egui_editor(
        EguiState::from_size(width, height),
        (),
        |_cx, _state| {},
        move |egui_ctx, setter, _state| {
            egui::CentralPanel::default()
                .frame(Frame::NONE)
                .show(egui_ctx, |ui| {
                    bg.draw(ui);

                    let mut color_idx = 0;
                    let total_knobs = 12;

                    let container_rect = ui.max_rect().shrink2(Vec2::new(40.0, 30.0));
                    ui.allocate_new_ui(UiBuilder::new().max_rect(container_rect), |ui| {
                        ui.vertical(|ui| {
                            ui.vertical_centered(|ui| {
                                Logo::draw(ui, 50.0);
                            });
                            ui.add_space(15.0);

                            // --- 上段: アンプヘッド (ノブの数に合わせて幅を可変にする) ---
                            ui.horizontal_top(|ui| {
                                ui.spacing_mut().item_spacing.x = 15.0;

                                draw_section_weighted(ui, "GAIN", 4.0, |ui| {
                                    let p = &params.gain_section;
                                    ui.horizontal(|ui| {
                                        for k in
                                            [&p.input_gain, &p.drive, &p.distortion, &p.master_gain]
                                        {
                                            ui.add(Knob::new(
                                                k,
                                                setter,
                                                get_vibrant_rainbow_color(color_idx, total_knobs),
                                            ));
                                            color_idx += 1;
                                        }
                                    });
                                });

                                draw_section_weighted(ui, "EQUALIZER", 5.0, |ui| {
                                    let p = &params.eq_section;
                                    ui.horizontal(|ui| {
                                        for k in
                                            [&p.low, &p.mid, &p.high, &p.presence, &p.resonance]
                                        {
                                            ui.add(Knob::new(
                                                k,
                                                setter,
                                                get_vibrant_rainbow_color(color_idx, total_knobs),
                                            ));
                                            color_idx += 1;
                                        }
                                    });
                                });

                                draw_section_weighted(ui, "EFFECT", 3.0, |ui| {
                                    let p = &params.fx_section;
                                    ui.horizontal(|ui| {
                                        for k in [&p.sag, &p.tight, &p.reverb_mix] {
                                            ui.add(Knob::new(
                                                k,
                                                setter,
                                                get_vibrant_rainbow_color(color_idx, total_knobs),
                                            ));
                                            color_idx += 1;
                                        }
                                    });
                                });
                            });

                            ui.add_space(20.0);

                            // --- 下段: キャビネットセクション ---
                            let cab_height = ui.available_height() - 20.0;
                            draw_section_with_height(
                                ui,
                                "CABINET & DUAL MICROPHONES",
                                cab_height,
                                |ui| {
                                    ui.vertical(|ui| {
                                        // 上部コントロール類
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 40.0;

                                            let mic_colors = [
                                                Color32::from_rgb(0, 180, 255),
                                                Color32::from_rgb(255, 100, 0),
                                            ];
                                            let mic_data = [
                                                (
                                                    "MIC A (Left)",
                                                    &params.cab_section.mic_a_axis,
                                                    &params.cab_section.mic_a_distance,
                                                ),
                                                (
                                                    "MIC B (Right)",
                                                    &params.cab_section.mic_b_axis,
                                                    &params.cab_section.mic_b_distance,
                                                ),
                                            ];

                                            for (i, (label, axis, dist)) in
                                                mic_data.iter().enumerate()
                                            {
                                                ui.vertical(|ui| {
                                                    ui.label(
                                                        egui::RichText::new(*label)
                                                            .color(mic_colors[i])
                                                            .strong(),
                                                    );
                                                    ui.add(LinearSlider::new(
                                                        axis,
                                                        setter,
                                                        mic_colors[i],
                                                    ));
                                                    ui.add(LinearSlider::new(
                                                        dist,
                                                        setter,
                                                        mic_colors[i],
                                                    ));
                                                });
                                            }

                                            ui.vertical(|ui| {
                                                ui.label(
                                                    egui::RichText::new("Cabinet / Room").strong(),
                                                );
                                                ui.add(LinearSlider::new(
                                                    &params.cab_section.speaker_size,
                                                    setter,
                                                    Color32::GOLD,
                                                ));
                                                ui.add(LinearSlider::new(
                                                    &params.cab_section.room_mix,
                                                    setter,
                                                    Color32::WHITE,
                                                ));
                                            });

                                            ui.vertical(|ui| {
                                                ui.label(
                                                    egui::RichText::new("Speaker Count").strong(),
                                                );
                                                ui.add_space(5.0);
                                                ui.horizontal(|ui| {
                                                    for &count in &[1, 2, 4, 6, 8] {
                                                        let is_selected = params
                                                            .cab_section
                                                            .speaker_count
                                                            .value()
                                                            == count;
                                                        let btn =
                                                            egui::Button::new(count.to_string())
                                                                .fill(if is_selected {
                                                                    Color32::from_rgb(0, 128, 0)
                                                                } else {
                                                                    Color32::from_gray(60)
                                                                })
                                                                .min_size(Vec2::new(35.0, 25.0));
                                                        if ui.add(btn).clicked() {
                                                            setter.begin_set_parameter(
                                                                &params.cab_section.speaker_count,
                                                            );
                                                            setter.set_parameter_normalized(
                                                                &params.cab_section.speaker_count,
                                                                params
                                                                    .cab_section
                                                                    .speaker_count
                                                                    .preview_normalized(count),
                                                            );
                                                            setter.end_set_parameter(
                                                                &params.cab_section.speaker_count,
                                                            );
                                                        }
                                                    }
                                                });
                                            });
                                        });

                                        ui.add_space(15.0);
                                        ui.separator();

                                        // --- ビジュアライザー ---
                                        // available_height一杯に広げ、SpeakerVisualizer側で全スピーカーを描画
                                        let visualizer_area = ui.available_height() - 10.0;
                                        ui.vertical_centered(|ui| {
                                            SpeakerVisualizer::new(&params.cab_section)
                                                .draw(ui, visualizer_area.min(450.0));
                                        });
                                    });
                                },
                            );
                        });
                    });
                });
        },
    )
}

/// 重み付けされた幅でセクションを描画する
fn draw_section_weighted(
    ui: &mut egui::Ui,
    title: &str,
    weight: f32,
    add_contents: impl FnMut(&mut egui::Ui),
) {
    // 全体の重み合計を12（4+5+3）として計算
    let total_weight = 12.0;
    let spacing = ui.spacing().item_spacing.x;
    let available_width = ui.available_width() - (spacing * 2.0);
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
        .fill(Color32::from_black_alpha(150))
        .stroke(egui::Stroke::new(1.0, Color32::from_gray(70)))
        .corner_radius(10.0)
        .inner_margin(15.0)
        .show(ui, |ui| {
            ui.set_min_height(height);
            ui.set_width(ui.available_width());
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(title)
                            .strong()
                            .color(Color32::from_gray(180))
                            .size(13.0),
                    );
                });

                ui.add_space(13.0);
                add_contents(ui);
            });
        });
}
