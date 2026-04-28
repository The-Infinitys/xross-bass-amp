use nih_plug::prelude::*;
use nih_plug_egui::{
    EguiState, create_egui_editor,
    egui::{self, Color32, Frame, UiBuilder, Vec2, ecolor::Hsva},
};
use std::sync::Arc;

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

fn get_vibrant_rainbow_color(index: usize, total: usize) -> Color32 {
    let h = (index as f32 / total as f32) * 0.85;
    Hsva::new(h, 0.8, 0.8, 1.0).into() // 少し落ち着かせた色
}

pub fn create_editor(params: Arc<XrossBassAmpParams>) -> Option<Box<dyn Editor>> {
    let width = 1400;
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
                    let total_knobs = 13;

                    let container_rect = ui.max_rect().shrink2(Vec2::new(40.0, 30.0));
                    ui.allocate_new_ui(UiBuilder::new().max_rect(container_rect), |ui| {
                        ui.vertical(|ui| {
                            ui.vertical_centered(|ui| {
                                Logo::draw(ui, 50.0);
                            });
                            ui.add_space(15.0);

                            // --- 上段: アンプヘッド ---
                            ui.horizontal_top(|ui| {
                                ui.spacing_mut().item_spacing.x = 15.0;

                                draw_section_weighted(ui, "GAIN / DIST", 5.0, |ui| {
                                    let p = &params.gain_section;
                                    ui.horizontal(|ui| {
                                        for k in [
                                            &p.input_gain,
                                            &p.drive,
                                            &p.grind,
                                            &p.blend,
                                            &p.master_gain,
                                        ] {
                                            ui.add(Knob::new(
                                                k,
                                                setter,
                                                get_vibrant_rainbow_color(color_idx, total_knobs),
                                            ));
                                            color_idx += 1;
                                        }
                                    });
                                });

                                draw_section_weighted(ui, "BASS EQUALIZER", 5.0, |ui| {
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

                                draw_section_weighted(ui, "DYNAMICS / FX", 3.0, |ui| {
                                    let p = &params.fx_section;
                                    ui.horizontal(|ui| {
                                        for k in [&p.compressor, &p.tight, &p.noise_gate] {
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
                                "BASS CABINET & DUAL MICROPHONES",
                                cab_height,
                                |ui| {
                                    ui.vertical(|ui| {
                                        // 上部コントロール類
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 40.0;

                                            let mic_colors = [
                                                Color32::from_rgb(0, 150, 220),
                                                Color32::from_rgb(220, 100, 0),
                                            ];
                                            let mic_data = [
                                                (
                                                    "MIC A (Dynamic)",
                                                    &params.cab_section.mic_a_axis,
                                                    &params.cab_section.mic_a_distance,
                                                ),
                                                (
                                                    "MIC B (Condenser)",
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
                                                    egui::RichText::new("Cabinet / Room")
                                                        .color(Color32::from_gray(80))
                                                        .strong(),
                                                );
                                                ui.add(LinearSlider::new(
                                                    &params.cab_section.speaker_size,
                                                    setter,
                                                    Color32::from_rgb(180, 160, 0),
                                                ));
                                                ui.add(LinearSlider::new(
                                                    &params.cab_section.room_mix,
                                                    setter,
                                                    Color32::from_gray(100),
                                                ));
                                            });

                                            ui.vertical(|ui| {
                                                ui.label(
                                                    egui::RichText::new("Speaker Count")
                                                        .color(Color32::from_gray(80))
                                                        .strong(),
                                                );
                                                ui.add_space(5.0);
                                                ui.horizontal(|ui| {
                                                    for &count in &[1, 2, 4, 8] {
                                                        let is_selected = params
                                                            .cab_section
                                                            .speaker_count
                                                            .value()
                                                            == count;
                                                        let btn =
                                                            egui::Button::new(count.to_string())
                                                                .fill(if is_selected {
                                                                    Color32::from_rgb(200, 200, 220)
                                                                } else {
                                                                    Color32::from_gray(230)
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
    let total_weight = 13.0; // 5 + 5 + 3
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
        .fill(Color32::from_white_alpha(180)) // 白背景
        .stroke(egui::Stroke::new(1.0, Color32::from_gray(200))) // シルバーの縁
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
                            .color(Color32::from_gray(80)) // 濃いグレー
                            .size(13.0),
                    );
                });

                ui.add_space(13.0);
                add_contents(ui);
            });
        });
}
