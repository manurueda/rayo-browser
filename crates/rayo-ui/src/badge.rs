//! SVG badge generation for health scores.
//!
//! Produces shields.io-style flat badges that can be embedded in READMEs
//! or served from a dashboard.

use std::path::Path;

/// Character width estimate for badge text sizing.
const CHAR_WIDTH: f64 = 6.5;
/// Horizontal padding on each side of a badge section.
const PADDING: f64 = 10.0;

/// Returns the badge color hex string for a given health score.
fn score_color(health_score: u32) -> &'static str {
    if health_score >= 80 {
        "#4c1"
    } else if health_score >= 50 {
        "#dfb317"
    } else {
        "#e05d44"
    }
}

/// Calculate the pixel width for a text section.
fn text_width(text: &str) -> f64 {
    (text.len() as f64) * CHAR_WIDTH + PADDING * 2.0
}

/// Generate a shields.io-style SVG badge for the given health score.
///
/// The badge has `label` on the left (dark background) and the score
/// percentage on the right (colored by threshold: green >= 80, yellow >= 50,
/// red < 50).
///
/// # Examples
///
/// ```
/// let svg = rayo_ui::badge::generate_badge(92, "QA Health");
/// assert!(svg.contains("QA Health"));
/// assert!(svg.contains("92%"));
/// ```
pub fn generate_badge(health_score: u32, label: &str) -> String {
    let value = format!("{health_score}%");
    let color = score_color(health_score);

    let label_width = text_width(label);
    let value_width = text_width(&value);
    let total_width = label_width + value_width;

    let label_x = label_width / 2.0;
    let value_x = label_width + value_width / 2.0;

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{total_width}" height="20">
  <linearGradient id="b" x2="0" y2="100%">
    <stop offset="0" stop-color="#bbb" stop-opacity=".1"/>
    <stop offset="1" stop-opacity=".1"/>
  </linearGradient>
  <clipPath id="a">
    <rect width="{total_width}" height="20" rx="3" fill="#fff"/>
  </clipPath>
  <g clip-path="url(#a)">
    <rect width="{label_width}" height="20" fill="#555"/>
    <rect x="{label_width}" width="{value_width}" height="20" fill="{color}"/>
    <rect width="{total_width}" height="20" fill="url(#b)"/>
  </g>
  <g fill="#fff" text-anchor="middle" font-family="DejaVu Sans,Verdana,Geneva,sans-serif" font-size="11">
    <text x="{label_x}" y="15" fill="#010101" fill-opacity=".3">{label}</text>
    <text x="{label_x}" y="14">{label}</text>
    <text x="{value_x}" y="15" fill="#010101" fill-opacity=".3">{value}</text>
    <text x="{value_x}" y="14">{value}</text>
  </g>
</svg>
"##
    )
}

/// Return a Markdown image snippet for a health-score badge.
///
/// # Examples
///
/// ```
/// let md = rayo_ui::badge::generate_badge_markdown(92, ".rayo/badge.svg");
/// assert_eq!(md, "![QA Health: 92%](.rayo/badge.svg)");
/// ```
pub fn generate_badge_markdown(health_score: u32, badge_path: &str) -> String {
    format!("![QA Health: {health_score}%]({badge_path})")
}

/// Generate an SVG badge and write it to `output_path`.
///
/// Parent directories are created automatically if they do not exist.
pub fn save_badge(health_score: u32, output_path: &Path) -> Result<(), std::io::Error> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let svg = generate_badge(health_score, "QA Health");
    std::fs::write(output_path, svg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn badge_green_score() {
        let svg = generate_badge(95, "QA Health");
        assert!(svg.contains("95%"));
        assert!(svg.contains("#4c1")); // green
        assert!(svg.contains("QA Health"));
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("</svg>"));
    }

    #[test]
    fn badge_yellow_score() {
        let svg = generate_badge(60, "QA Health");
        assert!(svg.contains("60%"));
        assert!(svg.contains("#dfb317")); // yellow
    }

    #[test]
    fn badge_red_score() {
        let svg = generate_badge(30, "QA Health");
        assert!(svg.contains("30%"));
        assert!(svg.contains("#e05d44")); // red
    }

    #[test]
    fn badge_boundary_80_is_green() {
        let svg = generate_badge(80, "QA Health");
        assert!(svg.contains("#4c1"));
    }

    #[test]
    fn badge_boundary_50_is_yellow() {
        let svg = generate_badge(50, "QA Health");
        assert!(svg.contains("#dfb317"));
    }

    #[test]
    fn badge_boundary_49_is_red() {
        let svg = generate_badge(49, "QA Health");
        assert!(svg.contains("#e05d44"));
    }

    #[test]
    fn markdown_output() {
        let md = generate_badge_markdown(92, ".rayo/badge.svg");
        assert_eq!(md, "![QA Health: 92%](.rayo/badge.svg)");
    }

    #[test]
    fn save_badge_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path: PathBuf = dir.path().join("nested/dir/badge.svg");

        save_badge(85, &path).unwrap();

        assert!(path.exists());
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("85%"));
        assert!(contents.contains("#4c1"));
        assert!(contents.contains("QA Health"));
    }
}
