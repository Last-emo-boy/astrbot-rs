use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontStyle {
    #[default]
    Regular,
    Bold,
    Italic,
    Monospace,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontFamily {
    Custom(PathBuf),
    Named(String),
    BuiltinDefault,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontRequest {
    pub size: u16,
    pub style: FontStyle,
}

impl FontRequest {
    pub fn new(size: u16) -> Self {
        Self {
            size,
            style: FontStyle::Regular,
        }
    }

    pub fn with_style(mut self, style: FontStyle) -> Self {
        self.style = style;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontSelection {
    pub request: FontRequest,
    pub family: FontFamily,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontCatalog {
    custom_font_path: Option<PathBuf>,
    regular_fallbacks: Vec<String>,
    bold_fallbacks: Vec<String>,
    italic_fallbacks: Vec<String>,
    monospace_fallbacks: Vec<String>,
}

impl Default for FontCatalog {
    fn default() -> Self {
        Self {
            custom_font_path: default_custom_font_path(),
            regular_fallbacks: vec![
                "msyh.ttc".to_string(),
                "NotoSansCJK-Regular.ttc".to_string(),
                "PingFang.ttc".to_string(),
                "Arial.ttf".to_string(),
                "DejaVuSans.ttf".to_string(),
            ],
            bold_fallbacks: vec![
                "msyhbd.ttc".to_string(),
                "Arial-Bold.ttf".to_string(),
                "DejaVuSans-Bold.ttf".to_string(),
            ],
            italic_fallbacks: vec![
                "msyhi.ttc".to_string(),
                "Arial-Italic.ttf".to_string(),
                "DejaVuSans-Oblique.ttf".to_string(),
            ],
            monospace_fallbacks: vec![
                "Consolas.ttf".to_string(),
                "Menlo.ttc".to_string(),
                "DejaVuSansMono.ttf".to_string(),
            ],
        }
    }
}

impl FontCatalog {
    pub fn with_custom_font(mut self, path: impl Into<PathBuf>) -> Self {
        self.custom_font_path = Some(path.into());
        self
    }

    pub fn without_custom_font(mut self) -> Self {
        self.custom_font_path = None;
        self
    }

    pub fn resolve(&self, request: FontRequest) -> FontSelection {
        if let Some(path) = &self.custom_font_path
            && path.exists()
        {
            return FontSelection {
                request,
                family: FontFamily::Custom(path.clone()),
            };
        }

        let fallback = match request.style {
            FontStyle::Regular => self.regular_fallbacks.first(),
            FontStyle::Bold => self.bold_fallbacks.first(),
            FontStyle::Italic => self.italic_fallbacks.first(),
            FontStyle::Monospace => self.monospace_fallbacks.first(),
        };

        FontSelection {
            request,
            family: fallback
                .cloned()
                .map(FontFamily::Named)
                .unwrap_or(FontFamily::BuiltinDefault),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextLayoutLine {
    pub text: String,
    pub width: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextMeasurer {
    average_width_ratio_percent: u16,
}

impl Default for TextMeasurer {
    fn default() -> Self {
        Self {
            average_width_ratio_percent: 60,
        }
    }
}

impl TextMeasurer {
    pub fn measure_width(&self, text: &str, selection: &FontSelection) -> u32 {
        let chars = text.chars().count() as u32;
        let size = u32::from(selection.request.size.max(1));
        (chars * size * u32::from(self.average_width_ratio_percent)).div_ceil(100)
    }

    pub fn wrap_text(
        &self,
        text: &str,
        selection: &FontSelection,
        max_width: u32,
    ) -> Vec<TextLayoutLine> {
        if text.is_empty() {
            return Vec::new();
        }

        let max_width = max_width.max(1);
        let mut lines = Vec::new();
        let mut current = String::new();
        for character in text.chars() {
            let candidate = format!("{current}{character}");
            if !current.is_empty() && self.measure_width(&candidate, selection) > max_width {
                let width = self.measure_width(&current, selection);
                lines.push(TextLayoutLine {
                    text: current,
                    width,
                });
                current = character.to_string();
            } else {
                current = candidate;
            }
        }
        if !current.is_empty() {
            let width = self.measure_width(&current, selection);
            lines.push(TextLayoutLine {
                text: current,
                width,
            });
        }
        lines
    }
}

fn default_custom_font_path() -> Option<PathBuf> {
    std::env::var_os("ASTRBOT_ROOT")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .map(|root| root.join("data").join("font.ttf"))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{FontCatalog, FontFamily, FontRequest, FontStyle, TextMeasurer};

    #[test]
    fn font_catalog_prefers_existing_custom_font() {
        let root = std::env::temp_dir().join(format!("astrbot_font_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let font = root.join("font.ttf");
        fs::write(&font, b"fake font").unwrap();

        let selection = FontCatalog::default()
            .with_custom_font(&font)
            .resolve(FontRequest::new(26));

        assert_eq!(selection.family, FontFamily::Custom(font));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn text_measurer_wraps_by_requested_font_size() {
        let catalog = FontCatalog::default().without_custom_font();
        let selection = catalog.resolve(FontRequest::new(10).with_style(FontStyle::Regular));
        let lines = TextMeasurer::default().wrap_text("abcdef", &selection, 18);

        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|line| line.width <= 18));
    }
}
