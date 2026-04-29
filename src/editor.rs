use egui::{self, Color32, Frame, NumExt, UiBuilder, Vec2, ecolor::Hsva};
use std::sync::Arc;
use truce::core::Editor;
use truce_egui::EguiEditor;

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
pub fn create_editor(params: Arc<XrossGuitarAmpParams>) -> Box<dyn Editor> {
    // ターゲットサイズを定義（このサイズをベースに描画）
    let width = 840;
    let height = 600;
    let bg = Background::new();

    let editor = EguiEditor::new((width, height), move |egui_ctx, _state| {
        egui::CentralPanel::default()
            .frame(Frame::NONE)
            .show(egui_ctx, |ui| {
                // 【重要】2048制限を絶対に超えないよう、1024にクランプ
                ui.set_max_width(width as f32);
                ui.set_max_height(height as f32);

                bg.draw(ui);

                let mut color_idx = 0;
                let total_knobs = 12;

                // 外周の余白をさらにタイトに (20, 15 -> 12, 10)
                let container_rect = ui.max_rect().shrink2(Vec2::new(12.0, 10.0));

                ui.allocate_new_ui(UiBuilder::new().max_rect(container_rect), |ui| {
                    ui.vertical(|ui| {
                        // --- ヘッダー（ロゴ） ---
                        ui.vertical_centered(|ui| {
                            Logo::draw(ui, 30.0); // さらに小型化
                        });
                        ui.add_space(2.0); // 隙間を最小限に

                        // --- 上段: アンプヘッド (コントロール類) ---
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0; // セクション間を詰める

                            draw_section_weighted(ui, "GAIN", 3.0, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 2.0; // ノブ間を極限まで詰める
                                    for k in [
                                        &params.input_gain,
                                        &params.drive,
                                        &params.distortion,
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

                            draw_section_weighted(ui, "EQUALIZER", 5.0, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 2.0;
                                    for k in [
                                        &params.low,
                                        &params.mid,
                                        &params.high,
                                        &params.presence,
                                        &params.resonance,
                                    ] {
                                        ui.add(Knob::new(
                                            k,
                                            get_vibrant_rainbow_color(color_idx, total_knobs),
                                        ));
                                        color_idx += 1;
                                    }
                                });
                            });

                            draw_section_weighted(ui, "EFFECT", 3.0, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 2.0;
                                    for k in [&params.sag, &params.tight, &params.reverb_mix] {
                                        ui.add(Knob::new(
                                            k,
                                            get_vibrant_rainbow_color(color_idx, total_knobs),
                                        ));
                                        color_idx += 1;
                                    }
                                });
                            });
                        });

                        ui.add_space(6.0); // 段落間のスペース

                        // --- 下段: キャビネットセクション ---
                        // 高さが足りない場合に備え、最低限の高さを確保しつつ余白を使う
                        let cab_height = ui.available_height().at_most(400.0);
                        draw_section_with_height(
                            ui,
                            "CABINET & DUAL MICROPHONES",
                            cab_height,
                            |ui| {
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 15.0; // 横の密度を上げる

                                        let mic_colors = [
                                            Color32::from_rgb(0, 180, 255),
                                            Color32::from_rgb(255, 100, 0),
                                        ];
                                        let mic_data = [
                                            ("MIC A", &params.mic_a_axis, &params.mic_a_distance),
                                            ("MIC B", &params.mic_b_axis, &params.mic_b_distance),
                                        ];

                                        for (i, (label, axis, dist)) in mic_data.iter().enumerate()
                                        {
                                            ui.vertical(|ui| {
                                                ui.label(
                                                    egui::RichText::new(*label)
                                                        .color(mic_colors[i])
                                                        .strong()
                                                        .size(10.0),
                                                );
                                                ui.add(LinearSlider::new(axis, mic_colors[i]));
                                                ui.add(LinearSlider::new(dist, mic_colors[i]));
                                            });
                                        }

                                        ui.vertical(|ui| {
                                            ui.label(
                                                egui::RichText::new("Cab/Room").strong().size(10.0),
                                            );
                                            ui.add(LinearSlider::new(
                                                &params.speaker_size,
                                                Color32::GOLD,
                                            ));
                                            ui.add(LinearSlider::new(
                                                &params.room_mix,
                                                Color32::WHITE,
                                            ));
                                        });

                                        ui.vertical(|ui| {
                                            ui.label(
                                                egui::RichText::new("Speakers").strong().size(10.0),
                                            );
                                            ui.add_space(2.0);
                                            ui.horizontal(|ui| {
                                                ui.spacing_mut().item_spacing.x = 2.0;
                                                for &count in &[1, 2, 4, 6, 8] {
                                                    let is_selected =
                                                        params.speaker_count.value() == count;
                                                    let btn = egui::Button::new(count.to_string())
                                                        .fill(if is_selected {
                                                            Color32::from_rgb(0, 80, 0)
                                                        } else {
                                                            Color32::from_gray(40)
                                                        })
                                                        .min_size(Vec2::new(24.0, 18.0));
                                                    if ui.add(btn).clicked() {
                                                        params.speaker_count.set_value(count);
                                                    }
                                                }
                                            });
                                        });
                                    });

                                    ui.add_space(4.0);
                                    ui.separator();

                                    // ビジュアライザーも少し控えめなサイズに
                                    let visualizer_area = ui.available_height() - 5.0;
                                    ui.vertical_centered(|ui| {
                                        SpeakerVisualizer::new(&params)
                                            .draw(ui, visualizer_area.min(300.0));
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
    let total_weight = 10.0;
    let spacing = ui.spacing().item_spacing.x;
    let num_elements = 3.0;
    let available_width = (ui.available_width() - (spacing * (num_elements - 1.0))).max(0.0);
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
        .fill(Color32::from_black_alpha(160)) // 背景を少し濃くして視認性確保
        .stroke(egui::Stroke::new(1.0, Color32::from_gray(60)))
        .corner_radius(6.0) // 角丸も少し控えめに
        .inner_margin(8.0) // 15.0 -> 8.0 大幅削減
        .show(ui, |ui| {
            ui.set_min_height(height);
            ui.set_width(ui.available_width());
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(title)
                        .strong()
                        .color(Color32::from_gray(160))
                        .size(10.0),
                );
                ui.add_space(6.0);
                add_contents(ui);
            });
        });
}
