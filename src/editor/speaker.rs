use crate::params::CabParams;
use nih_plug::prelude::FloatRange;
use nih_plug_egui::egui::{self, Color32, Pos2, Stroke, Vec2};

pub struct SpeakerVisualizer<'a> {
    params: &'a CabParams,
}

impl<'a> SpeakerVisualizer<'a> {
    pub fn new(params: &'a CabParams) -> Self {
        Self { params }
    }

    pub fn draw(&self, ui: &mut egui::Ui, height: f32) {
        let width = ui.available_width();
        let (rect, _) = ui.allocate_at_least(Vec2::new(width, height), egui::Sense::hover());
        let painter = ui.painter();
        let center = rect.center();

        // --- パラメータ取得 ---
        let room_mix = self.params.room_mix.value();

        // 1. 背景（ベース）
        painter.rect_filled(rect, 8.0, Color32::from_rgb(10, 10, 10));
        // 背景描画の直後、スピーカーやマイクを描画する前に挿入
        let painter = painter.with_clip_rect(rect);
        if room_mix > 0.01 {
            // 1. 中心のソフトグロー（空間の広がり）
            // ループ回数を増やして滑らかに
            let glow_steps = 12;
            for i in 0..glow_steps {
                let t = i as f32 / glow_steps as f32;
                let radius = height * (0.3 + t * 1.0); // 外側へ広がる
                // 外側ほど透明にする（非線形に減衰させて馴染ませる）
                let alpha = (room_mix * 35.0 * (1.0 - t).powi(3)) as u8;

                painter.circle_filled(center, radius, Color32::from_white_alpha(alpha));
            }

            // 2. 多段階ヴィニエット（周辺減光）
            // 四隅をじんわり暗くして中央を浮かび上がらせる
            let v_steps = 8;
            for i in 1..=v_steps {
                let t = i as f32 / v_steps as f32;
                // 厚みと暗さを段階的に重ねる
                let thickness = height * 0.15 * t * room_mix;
                let alpha = (50.0 * t * room_mix) as u8;

                painter.rect_stroke(
                    rect,
                    8.0,
                    Stroke::new(thickness, Color32::from_black_alpha(alpha)),
                    egui::StrokeKind::Inside,
                );
            }
        }
        let count = self.params.speaker_count.value(); // --- レイアウト計算 ---
        // 1: 1x1, 2: 2x1, 4: 2x2
        let mut positions = Vec::new();
        let max_speaker_radius = match self.params.speaker_size.range() {
            FloatRange::Linear { max, .. } => max,
            FloatRange::Skewed { max, .. } => max,
            FloatRange::SymmetricalSkewed { max, .. } => max,
            _ => 1.0,
        };
        let speaker_radius_per = self.params.speaker_size.value() / max_speaker_radius;
        let speaker_radius = match count {
            1 => {
                positions.push(center);
                height * 0.4
            }
            2 => {
                let offset_x = height * 0.45;
                positions.push(center - Vec2::new(offset_x, 0.0));
                positions.push(center + Vec2::new(offset_x, 0.0));
                height * 0.38
            }
            6 => {
                // 2段 x 3列 (横に広い構成)
                let off_x = height * 0.5; // 横の広がり
                let off_y = height * 0.22; // 上下の間隔
                let spacing_x = off_x;

                for x_idx in &[-1.0, 0.0, 1.0] {
                    for y_idx in &[-1.0, 1.0] {
                        positions.push(center + Vec2::new(x_idx * spacing_x, y_idx * off_y));
                    }
                }
                height * 0.18
            }
            8 => {
                // 2段 x 4列 (圧倒的な横幅の壁)
                let off_x = height * 0.65; // さらに横に広げる
                let off_y = height * 0.22;
                let spacing_x = off_x * 0.66; // 4列にするための間隔調整

                for i in 0..4 {
                    let x = -off_x + (i as f32 * spacing_x);
                    for y in &[-off_y, off_y] {
                        positions.push(center + Vec2::new(x, *y));
                    }
                }
                height * 0.16
            }
            _ => {
                // 4 (2x2) デフォルト
                let offset_x = height * 0.35;
                let offset_y = height * 0.25;
                positions.push(center + Vec2::new(-offset_x, -offset_y));
                positions.push(center + Vec2::new(offset_x, -offset_y));
                positions.push(center + Vec2::new(-offset_x, offset_y));
                positions.push(center + Vec2::new(offset_x, offset_y));
                height * 0.24
            }
        };

        // スピーカーユニットの描画
        for &pos in &positions {
            let speaker_radius = speaker_radius_per * speaker_radius;

            painter.circle_stroke(
                pos,
                speaker_radius,
                Stroke::new(2.0, Color32::from_gray(60)),
            );
            painter.circle_filled(pos, speaker_radius * 0.9, Color32::from_gray(25));
            painter.circle_filled(pos, speaker_radius * 0.25, Color32::from_gray(45));

            // コーンの立体感を出すためのライン
            painter.circle_stroke(
                pos,
                speaker_radius * 0.45,
                Stroke::new(1.0, Color32::from_gray(35)),
            );
        }

        // --- マイク描画 ---
        // 複数スピーカーがある場合、マイクは「メイン（最初）のスピーカー」を狙うように設定
        // axis=0.0 でセンター、1.0 でエッジ

        // マイク A (Blue)
        self.draw_mic(
            ui,
            center,
            width / 5.0,
            -self.params.mic_a_axis.value(),
            self.params.mic_a_distance.value(),
            Color32::from_rgb(0, 180, 255),
        );

        // マイク B (Orange)
        self.draw_mic(
            ui,
            center,
            width / 5.0,
            self.params.mic_b_axis.value(),
            self.params.mic_b_distance.value(),
            Color32::from_rgb(255, 100, 0),
        );
    }
    fn draw_mic(
        &self,
        ui: &mut egui::Ui,
        center: Pos2,
        reference_width: f32,
        axis: f32,
        dist: f32, // 0.0 (密着) ～ 1.0 (手前)
        color: Color32,
    ) {
        let painter = ui.painter();

        // --- 1. 影（接地位置）の計算 ---
        // 影は常にスピーカーの面上にあるため、移動幅は一定
        let shadow_x = axis * reference_width * 0.8;
        let shadow_pos = center + Vec2::new(shadow_x, 0.0);

        // --- 2. マイク本体のパース計算 ---
        // 【重要】手前に来る（distが大きい）ほど、左右の移動幅（axisの影響）を倍増させる
        // 1.0 + dist * 1.5 により、手前では最大 2.5倍 の移動幅になる
        let perspective_factor = 1.0 + (dist * 1.5);
        let mic_x = axis * reference_width * 0.8 * perspective_factor;

        // 手前に来るほど「上」に大きく浮かせ、かつ少し「下」にずらすことでカメラへの接近を表現
        let mic_y_float = -(dist * 60.0); // 垂直方向の浮き上がり
        let mic_y_push = dist * 20.0; // 手前に来ることによるパースの沈み込み
        let mic_pos = center + Vec2::new(mic_x, mic_y_float + mic_y_push);

        // --- 3. サイズの動的変化 ---
        // 手前に来るほどマイクを大きく（12.0 -> 24.0）
        let mic_radius = 12.0 * (1.0 + dist * 1.0);
        // 影は逆に、離れるほどボケて大きく、薄くする
        let shadow_radius = 10.0 * (1.0 + dist * 0.5);
        let shadow_alpha = (160.0 * (1.0 - dist * 0.8)) as u8;

        // --- 4. 描画 ---
        // コーン上の影
        painter.circle_filled(
            shadow_pos,
            shadow_radius,
            Color32::from_black_alpha(shadow_alpha),
        );

        // 接地点とマイクを繋ぐガイド線（これがあると高さ関係がすぐわかる）
        painter.line_segment(
            [shadow_pos, mic_pos],
            Stroke::new(1.0, color.linear_multiply(0.2)),
        );

        // マイク本体のドロップシャドウ（マイク自体の浮遊感）
        painter.circle_filled(
            mic_pos + Vec2::new(3.0, 3.0),
            mic_radius,
            Color32::from_black_alpha(100),
        );
        let room_mix = self.params.room_mix.value();
        // draw_mic 内
        if room_mix > 0.1 {
            for i in 1..=3 {
                let t = i as f32 / 3.0;
                let ring_radius = mic_radius * (1.1 + t * room_mix * 2.0);
                let ring_alpha = (30.0 * (1.0 - t) * room_mix) as u8;
                painter.circle_stroke(
                    mic_pos,
                    ring_radius,
                    Stroke::new(1.0, color.linear_multiply(ring_alpha as f32 / 255.0)),
                );
            }
        } // 本体
        painter.circle_filled(mic_pos, mic_radius, color.linear_multiply(0.8));
        painter.circle_stroke(
            mic_pos,
            mic_radius,
            Stroke::new(2.5 * (1.0 + dist * 0.5), color),
        );

        // ハイライト（手前に来るほど光沢を強く）
        painter.circle_filled(
            mic_pos - Vec2::new(mic_radius * 0.3, mic_radius * 0.3),
            mic_radius * 0.2,
            Color32::WHITE.linear_multiply(0.4 + dist * 0.2),
        );
    }
}
