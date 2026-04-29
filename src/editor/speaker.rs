use crate::params::XrossBassAmpParams;
use egui::{self, Color32, Pos2, Stroke, Vec2};

pub struct SpeakerVisualizer<'a> {
    params: &'a XrossBassAmpParams,
}

impl<'a> SpeakerVisualizer<'a> {
    pub fn new(params: &'a XrossBassAmpParams) -> Self {
        Self { params }
    }

    pub fn draw(&self, ui: &mut egui::Ui, height: f32) {
        let width = ui.available_width();
        let (rect, _) = ui.allocate_at_least(Vec2::new(width, height), egui::Sense::hover());
        let center = rect.center();
        let painter = ui.painter().with_clip_rect(rect); // はみ出しを確実にカット

        // --- パラメータ取得 ---
        let room_mix = self.params.room_mix.value();
        let speaker_count = self.params.speaker_count.value() as i32;
        let max_size = self.params.speaker_size.info.range.max() as f32;
        let size_factor = self.params.speaker_size.value() / max_size;

        // 1. 背景：明るいライトグレーのキャビネット
        painter.rect_filled(rect, 8.0, Color32::from_gray(245));

        // ROOM表現：豪華なGlow効果（ライトモード版）
        if room_mix > 0.01 {
            // 中心からの光の広がり（白から透明へのグラデーション風）
            let glow_steps = 10;
            for i in 0..glow_steps {
                let t = i as f32 / glow_steps as f32;
                let radius = height * (0.3 + t * 1.2);
                let alpha = (room_mix * 40.0 * (1.0 - t).powi(2)) as u8;
                painter.circle_filled(center, radius, Color32::from_white_alpha(alpha));
            }

            // 四隅の影（ビネット）：空間の奥行きを演出
            let v_steps = 6;
            for i in 1..=v_steps {
                let t = i as f32 / v_steps as f32;
                let thickness = height * 0.12 * t * room_mix;
                let alpha = (30.0 * t * room_mix) as u8;
                painter.rect_stroke(
                    rect,
                    8.0,
                    Stroke::new(thickness, Color32::from_black_alpha(alpha)),
                    egui::StrokeKind::Inside,
                );
            }
        }

        // 2. スピーカーユニットのレイアウト計算
        let mut positions = Vec::new();
        let unit_base_radius = match speaker_count {
            1 => {
                positions.push(center);
                height * 0.42
            }
            2 => {
                let off = height * 0.38;
                positions.push(center - Vec2::new(off, 0.0));
                positions.push(center + Vec2::new(off, 0.0));
                height * 0.35
            }
            8 => {
                let step_x = height * 0.28;
                let step_y = height * 0.24;
                for x in &[-1.5, -0.5, 0.5, 1.5] {
                    for y in &[-0.7, 0.7] {
                        positions.push(center + Vec2::new(x * step_x, y * step_y));
                    }
                }
                height * 0.12
            }
            16 => {
                let step_x = height * 0.22;
                let step_y = height * 0.22;
                for x in &[-2.0, -1.0, 1.0, 2.0] {
                    for y in &[-1.5, -0.5, 0.5, 1.5] {
                        positions.push(center + Vec2::new(x * step_x, y * step_y));
                    }
                }
                height * 0.1
            }
            _ => {
                // 4
                let off = height * 0.26;
                positions.push(center + Vec2::new(-off, -off));
                positions.push(center + Vec2::new(off, -off));
                positions.push(center + Vec2::new(-off, off));
                positions.push(center + Vec2::new(off, off));
                height * 0.25
            }
        };

        // スピーカー本体の描画（明るいトーン）
        for &pos in &positions {
            let r = unit_base_radius * size_factor;
            // 外枠
            painter.circle_stroke(pos, r, Stroke::new(1.5, Color32::from_gray(180)));
            // コーン紙
            painter.circle_filled(pos, r * 0.95, Color32::from_gray(210));
            // センターキャップ
            painter.circle_filled(pos, r * 0.35, Color32::from_gray(170));
            painter.circle_stroke(pos, r * 0.35, Stroke::new(1.0, Color32::from_gray(140)));
        }

        // 3. マイクの描画（A: 左軸, B: 右軸）
        self.draw_mic(
            &painter,
            center,
            width * 0.15,
            -self.params.mic_a_axis.value(),
            self.params.mic_a_distance.value(),
            Color32::from_rgb(0, 140, 255), // 明るい青
            room_mix,
        );

        self.draw_mic(
            &painter,
            center,
            width * 0.15,
            self.params.mic_b_axis.value(),
            self.params.mic_b_distance.value(),
            Color32::from_rgb(255, 110, 0), // 明るいオレンジ
            room_mix,
        );
    }

    fn draw_mic(
        &self,
        painter: &egui::Painter,
        center: Pos2,
        ref_width: f32,
        axis: f32,
        dist: f32,
        color: Color32,
        room_mix: f32,
    ) {
        // 遠近ロジック：dist 0.0 (密着) -> 手前に大きく / dist 1.0 (遠い) -> 奥に小さく
        // スピーカー位置を(0.0)として、手前に来るほどyをプラスし、スケールを大きくする
        let inv_dist = 1.0 - dist;
        let perspective_scale = 1.0 + (inv_dist * 0.8); // 1.0 ~ 1.8倍
        let mic_radius = 12.0 * perspective_scale;

        // X位置：遠くほど中心に寄るパース表現
        let x_pos = center.x + (axis * ref_width * (1.0 + inv_dist * 0.2));
        // Y位置：手前に来るほど下へ
        let y_pos = center.y + (inv_dist * 50.0);
        let mic_pos = Pos2::new(x_pos, y_pos);

        // 影：スピーカー面に投影（distに関わらずスピーカーの高さ center.y 付近に固定）
        let shadow_pos = Pos2::new(center.x + (axis * ref_width), center.y);
        let shadow_alpha = (100.0 * dist.max(0.2)) as u8;
        painter.circle_filled(
            shadow_pos,
            mic_radius * 0.7,
            Color32::from_black_alpha(shadow_alpha),
        );

        // 接続線（マイクと影を繋ぐガイド）
        painter.line_segment(
            [shadow_pos, mic_pos],
            Stroke::new(1.0, color.linear_multiply(0.3)),
        );

        // Room Mixに応じたオーラ表現（ギター版継承）
        if room_mix > 0.05 {
            for i in 1..=2 {
                let t = i as f32 / 2.0;
                let r = mic_radius * (1.1 + t * room_mix);
                painter.circle_stroke(
                    mic_pos,
                    r,
                    Stroke::new(1.0, color.linear_multiply(0.2 * room_mix)),
                );
            }
        }

        // マイク本体
        painter.circle_filled(mic_pos, mic_radius, color);
        painter.circle_stroke(mic_pos, mic_radius, Stroke::new(2.0, Color32::WHITE));

        // ハイライト
        painter.circle_filled(
            mic_pos - Vec2::new(mic_radius * 0.3, mic_radius * 0.3),
            mic_radius * 0.25,
            Color32::from_white_alpha(180),
        );
    }
}
