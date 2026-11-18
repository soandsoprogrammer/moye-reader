use std::{sync::Arc, sync::LazyLock};

use regex::Regex;
use scraper::{ElementRef, Selector};

pub type ColorSheet = Arc<[ColorRule]>;

pub struct ColorRule {
    selector: Selector,
    specificity: (usize, usize, usize),
    color: u32,
}

static COMMENTS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)/\*.*?\*/").unwrap());
static RULES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([^{}]+)\{([^{}]*)\}").unwrap());

pub fn read_colors(css: &str) -> ColorSheet {
    let css = COMMENTS.replace_all(css, "");
    let mut rules = Vec::new();
    for rule in RULES.captures_iter(&css) {
        let Some(color) = declaration_color(&rule[2]) else {
            continue;
        };
        for value in rule[1].split(',').map(str::trim) {
            if value.len() > 180 || value.starts_with('@') || value.contains([':', '[', '|', '*']) {
                continue;
            }
            if let Ok(selector) = Selector::parse(value) {
                let classes = value.matches('.').count();
                let ids = value.matches('#').count();
                let tags = value
                    .split(|ch: char| ch.is_ascii_whitespace() || matches!(ch, '>' | '+' | '~'))
                    .filter(|part| part.starts_with(|ch: char| ch.is_ascii_alphabetic()))
                    .count();
                rules.push(ColorRule {
                    selector,
                    specificity: (ids, classes, tags),
                    color,
                });
                if rules.len() == 160 {
                    return rules.into();
                }
            }
        }
    }
    rules.into()
}

pub fn text_color(node: ElementRef<'_>, sheets: &[ColorSheet]) -> Option<u32> {
    node.value()
        .attr("style")
        .and_then(declaration_color)
        .or_else(|| {
            sheets
                .iter()
                .flat_map(|sheet| sheet.iter())
                .filter(|rule| rule.selector.matches(&node))
                .max_by_key(|rule| rule.specificity)
                .map(|rule| rule.color)
        })
        .or_else(|| {
            (node.value().name() == "font")
                .then(|| node.value().attr("color"))
                .flatten()
                .and_then(parse_color)
        })
}

fn declaration_color(style: &str) -> Option<u32> {
    style
        .split(';')
        .filter_map(|part| part.split_once(':'))
        .filter(|(name, _)| name.trim().eq_ignore_ascii_case("color"))
        .filter_map(|(_, color)| parse_color(color.split('!').next()?.trim()))
        .next_back()
}

fn parse_color(value: &str) -> Option<u32> {
    let value = value.trim().to_ascii_lowercase();
    if let Some(hex) = value.strip_prefix('#') {
        return match hex.len() {
            3 => u32::from_str_radix(hex, 16).ok().map(|color| {
                (((color >> 8) * 17) << 16)
                    | ((((color >> 4) & 15) * 17) << 8)
                    | ((color & 15) * 17)
            }),
            6 => u32::from_str_radix(hex, 16).ok(),
            _ => None,
        };
    }
    if let Some(rgb) = value
        .strip_prefix("rgb(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let mut channels = rgb
            .split(|ch: char| ch == ',' || ch.is_ascii_whitespace())
            .filter(|ch| !ch.is_empty());
        let mut color = 0;
        for _ in 0..3 {
            let channel = channels.next()?;
            let channel = if let Some(percent) = channel.strip_suffix('%') {
                (percent.parse::<f32>().ok()?.clamp(0., 100.) * 2.55).round() as u8
            } else {
                channel.parse::<u8>().ok()?
            };
            color = (color << 8) | u32::from(channel);
        }
        return channels.next().is_none().then_some(color);
    }
    Some(match value.as_str() {
        "black" => 0x000000,
        "white" => 0xffffff,
        "gray" | "grey" => 0x808080,
        "red" => 0xff0000,
        "darkred" => 0x8b0000,
        "maroon" => 0x800000,
        "crimson" => 0xdc143c,
        "brown" => 0xa52a2a,
        "firebrick" => 0xb22222,
        "green" => 0x008000,
        "darkgreen" => 0x006400,
        "teal" => 0x008080,
        "blue" => 0x0000ff,
        "navy" => 0x000080,
        "purple" => 0x800080,
        "orange" => 0xffa500,
        "darkorange" => 0xff8c00,
        _ => return None,
    })
}
