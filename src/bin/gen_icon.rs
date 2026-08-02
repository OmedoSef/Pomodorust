//! Génère l'icône de l'application (une tomate stylisée) en pur Rust,
//! sans dépendre d'un rasteriseur SVG externe.
//!
//! Utilisation : `cargo run --bin gen-icon`
//! Produit : `assets/icon.png` (128x128, RGBA, fond transparent)

use image::{Rgba, RgbaImage};

const SIZE: u32 = 128;

fn main() {
    let mut img = RgbaImage::from_pixel(SIZE, SIZE, Rgba([0, 0, 0, 0]));

    draw_tomato_body(&mut img);
    draw_leaves(&mut img);

    let out_dir = std::path::Path::new("assets");
    std::fs::create_dir_all(out_dir).expect("impossible de créer le dossier assets/");
    let out_path = out_dir.join("icon.png");
    img.save(&out_path).expect("échec de l'écriture de icon.png");

    println!("Icône générée : {}", out_path.display());
}

/// Dessine le corps rond et rouge de la tomate à l'aide d'un simple test
/// de distance au centre (cercle plein), avec un anti-aliasing léger sur
/// le contour et un dégradé subtil pour donner du volume.
fn draw_tomato_body(img: &mut RgbaImage) {
    let center_x = SIZE as f32 / 2.0;
    let center_y = SIZE as f32 / 2.0 + 6.0;
    let radius = SIZE as f32 * 0.40;

    let base = (214u8, 48u8, 40u8);
    let shade = (168u8, 30u8, 24u8);
    let highlight = (255u8, 120u8, 100u8);

    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - center_x;
            let dy = y as f32 - center_y;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist > radius + 1.0 {
                continue;
            }

            // Anti-aliasing sur le bord externe.
            let coverage = if dist > radius - 1.0 {
                (radius + 1.0 - dist).clamp(0.0, 2.0) / 2.0
            } else {
                1.0
            };

            // Petit dégradé : plus sombre en bas, un reflet en haut-gauche.
            let highlight_dist = ((dx + radius * 0.35).powi(2) + (dy + radius * 0.35).powi(2)).sqrt();
            let (r, g, b) = if highlight_dist < radius * 0.35 {
                let t = 1.0 - (highlight_dist / (radius * 0.35));
                lerp_color(base, highlight, t * 0.6)
            } else if dy > radius * 0.2 {
                let t = ((dy - radius * 0.2) / (radius * 0.8)).clamp(0.0, 1.0);
                lerp_color(base, shade, t * 0.5)
            } else {
                base
            };

            let alpha = (255.0 * coverage) as u8;
            if alpha == 0 {
                continue;
            }
            blend_pixel(img, x, y, Rgba([r, g, b, alpha]));
        }
    }
}

/// Dessine 3 petites feuilles vertes (triangles) formant le calice de la
/// tomate, en haut de l'icône.
fn draw_leaves(img: &mut RgbaImage) {
    let green = (54u8, 140u8, 60u8);
    let center_x = SIZE as f32 / 2.0;
    let top_y = SIZE as f32 * 0.20;

    let leaves = [
        // (tip_x, tip_y, base_left, base_right, base_y)
        (center_x, top_y - 14.0, center_x - 22.0, center_x + 2.0, top_y + 10.0),
        (center_x - 20.0, top_y - 4.0, center_x - 38.0, center_x - 6.0, top_y + 12.0),
        (center_x + 20.0, top_y - 4.0, center_x + 6.0, center_x + 38.0, top_y + 12.0),
    ];

    for (tip_x, tip_y, base_l, base_r, base_y) in leaves {
        fill_triangle(
            img,
            (tip_x, tip_y),
            (base_l, base_y),
            (base_r, base_y),
            Rgba([green.0, green.1, green.2, 255]),
        );
    }

    // Petite tige au centre.
    for y in (top_y as i32 - 18)..(top_y as i32 + 6) {
        for x in (center_x as i32 - 3)..(center_x as i32 + 3) {
            if x >= 0 && y >= 0 && (x as u32) < SIZE && (y as u32) < SIZE {
                blend_pixel(img, x as u32, y as u32, Rgba([green.0, green.1, green.2, 255]));
            }
        }
    }
}

fn lerp_color(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    (
        (a.0 as f32 + (b.0 as f32 - a.0 as f32) * t) as u8,
        (a.1 as f32 + (b.1 as f32 - a.1 as f32) * t) as u8,
        (a.2 as f32 + (b.2 as f32 - a.2 as f32) * t) as u8,
    )
}

fn blend_pixel(img: &mut RgbaImage, x: u32, y: u32, color: Rgba<u8>) {
    if x >= img.width() || y >= img.height() {
        return;
    }
    let existing = *img.get_pixel(x, y);
    let src_a = color[3] as f32 / 255.0;
    let dst_a = existing[3] as f32 / 255.0;
    let out_a = src_a + dst_a * (1.0 - src_a);
    if out_a <= 0.0 {
        img.put_pixel(x, y, Rgba([0, 0, 0, 0]));
        return;
    }
    let blend = |src: u8, dst: u8| -> u8 {
        (((src as f32 * src_a) + (dst as f32 * dst_a * (1.0 - src_a))) / out_a) as u8
    };
    img.put_pixel(
        x,
        y,
        Rgba([
            blend(color[0], existing[0]),
            blend(color[1], existing[1]),
            blend(color[2], existing[2]),
            (out_a * 255.0) as u8,
        ]),
    );
}

/// Remplissage de triangle par balayage de lignes (bounding box + test de
/// coordonnées barycentriques).
fn fill_triangle(img: &mut RgbaImage, p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), color: Rgba<u8>) {
    let min_x = p0.0.min(p1.0).min(p2.0).floor().max(0.0) as u32;
    let max_x = p0.0.max(p1.0).max(p2.0).ceil().min(SIZE as f32 - 1.0) as u32;
    let min_y = p0.1.min(p1.1).min(p2.1).floor().max(0.0) as u32;
    let max_y = p0.1.max(p1.1).max(p2.1).ceil().min(SIZE as f32 - 1.0) as u32;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let p = (x as f32 + 0.5, y as f32 + 0.5);
            if point_in_triangle(p, p0, p1, p2) {
                blend_pixel(img, x, y, color);
            }
        }
    }
}

fn point_in_triangle(p: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
    let sign = |p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)| -> f32 {
        (p1.0 - p3.0) * (p2.1 - p3.1) - (p2.0 - p3.0) * (p1.1 - p3.1)
    };
    let d1 = sign(p, a, b);
    let d2 = sign(p, b, c);
    let d3 = sign(p, c, a);

    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;

    !(has_neg && has_pos)
}
