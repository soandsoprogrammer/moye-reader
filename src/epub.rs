use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{Read, Seek},
    path::Path,
    sync::LazyLock,
};

use anyhow::{Context, Result, bail, ensure};
use percent_encoding::percent_decode_str;
use scraper::{ElementRef, Html, Selector, node::Node};
use zip::ZipArchive;

use crate::book::{
    Block, BlockKind, Book, BookInfo, ContentsEntry, Inline, Link, NoteKind, Section, SourceNote,
    Span, Target, decode_text, split_block,
};

use crate::epub_style::{ColorSheet, read_colors, text_color};

const MAX_DOCUMENT: u64 = 12 * 1024 * 1024;
const MAX_PUBLICATION_TEXT: u64 = 160 * 1024 * 1024;
pub const MAX_IMAGE: u64 = 24 * 1024 * 1024;

pub fn open_zip(path: &Path) -> Result<ZipArchive<File>> {
    ZipArchive::new(File::open(path)?).context("无法打开 EPUB 压缩包")
}

pub fn read_entry(
    zip: &mut ZipArchive<impl Read + Seek>,
    name: &str,
    limit: u64,
) -> Result<Vec<u8>> {
    let entry = zip
        .by_name(name)
        .with_context(|| format!("EPUB 缺少资源 {name}"))?;
    ensure!(entry.size() <= limit, "EPUB 资源过大：{name}");
    let mut data = Vec::with_capacity(entry.size() as usize);
    entry.take(limit + 1).read_to_end(&mut data)?;
    ensure!(data.len() as u64 <= limit, "EPUB 资源过大：{name}");
    Ok(data)
}

fn read_document(zip: &mut ZipArchive<File>, path: &str) -> Result<String> {
    decode_text(&read_entry(zip, path, MAX_DOCUMENT)?)
}

fn decoded_url(segment: &str) -> String {
    percent_decode_str(segment).decode_utf8_lossy().into_owned()
}

fn local_target(base: &str, href: &str) -> Option<Target> {
    let href = href.trim();
    let (file, fragment) = href.split_once('#').unwrap_or((href, ""));
    let file = file.split('?').next().unwrap_or("");
    if file.contains(':') || file.starts_with("//") || file.contains('\\') {
        return None;
    }
    if file.is_empty() {
        return Some(Target {
            path: base.to_owned(),
            fragment: decoded_url(fragment),
        });
    }
    let mut parts: Vec<&str> = if file.starts_with('/') {
        vec![]
    } else {
        base.rsplit_once('/')
            .map_or_else(Vec::new, |(dir, _)| dir.split('/').collect())
    };
    let file = decoded_url(file);
    for part in file.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            value => parts.push(value),
        }
    }
    Some(Target {
        path: parts.join("/"),
        fragment: decoded_url(fragment),
    })
}

fn link_for(base: &str, href: &str) -> Option<Link> {
    if let Some(target) = local_target(base, href) {
        Some(Link::Local(target))
    } else {
        let (scheme, _) = href.trim().split_once(':')?;
        (scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"))
            .then(|| Link::External(href.trim().to_owned()))
    }
}

#[derive(Clone)]
struct Resource {
    target: Target,
    media: String,
    flags: String,
}

impl Resource {
    fn html(&self) -> bool {
        matches!(self.media.as_str(), "application/xhtml+xml" | "text/html")
    }

    fn flag(&self, name: &str) -> bool {
        self.flags.split_ascii_whitespace().any(|flag| flag == name)
    }
}

struct NavEntry {
    title: String,
    target: Target,
    depth: usize,
}

pub fn parse_epub(mut info: BookInfo, path: &Path) -> Result<Book> {
    let mut zip = open_zip(path)?;
    let container = read_document(&mut zip, "META-INF/container.xml")?;
    let container = xml(&container).context("EPUB 的容器清单格式无效")?;
    let package_path = container
        .descendants()
        .find(|node| node.has_tag_name("rootfile"))
        .and_then(|node| node.attribute("full-path"))
        .context("EPUB 缺少书籍清单")?;
    let opf = read_document(&mut zip, package_path)?;
    let opf = xml(&opf).context("EPUB 的书籍清单格式无效")?;
    if let Some(metadata) = opf.descendants().find(|node| node.has_tag_name("metadata")) {
        if let Some(title) = metadata
            .children()
            .find(|node| node.has_tag_name("title"))
            .and_then(|node| node.text())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            info.title = title.to_owned();
        }
        info.author = metadata
            .children()
            .filter(|node| node.has_tag_name("creator"))
            .filter_map(|node| node.text())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("、");
    }

    let mut resources = HashMap::new();
    let mut order = vec![];
    for item in opf.descendants().filter(|node| node.has_tag_name("item")) {
        let Some(id) = item.attribute("id") else {
            continue;
        };
        let Some(target) = item
            .attribute("href")
            .and_then(|href| local_target(package_path, href))
        else {
            continue;
        };
        order.push(id);
        resources.insert(
            id,
            Resource {
                target,
                media: item.attribute("media-type").unwrap_or("").to_owned(),
                flags: item.attribute("properties").unwrap_or("").to_owned(),
            },
        );
    }

    let spine = opf
        .descendants()
        .find(|node| node.has_tag_name("spine"))
        .context("EPUB 缺少阅读顺序")?;
    let navigation = order
        .iter()
        .filter_map(|id| resources.get(id))
        .find(|resource| resource.flag("nav"))
        .and_then(|resource| {
            read_document(&mut zip, &resource.target.path)
                .ok()
                .map(|source| epub3_navigation(&resource.target.path, &source))
        })
        .filter(|items| !items.is_empty())
        .or_else(|| {
            let resource = spine
                .attribute("toc")
                .and_then(|id| resources.get(id))
                .or_else(|| {
                    order
                        .iter()
                        .filter_map(|id| resources.get(id))
                        .find(|resource| resource.media == "application/x-dtbncx+xml")
                })?;
            let source = read_document(&mut zip, &resource.target.path).ok()?;
            Some(ncx_navigation(&resource.target.path, &source))
        })
        .unwrap_or_default();
    let cover_id = opf
        .descendants()
        .find(|node| node.has_tag_name("meta") && node.attribute("name") == Some("cover"))
        .and_then(|node| node.attribute("content"));
    let cover = order
        .iter()
        .filter_map(|id| resources.get(id))
        .find(|item| item.flag("cover-image"))
        .or_else(|| cover_id.and_then(|id| resources.get(id)))
        .map(|item| item.target.path.clone());

    let mut pending = Vec::new();
    let mut seen = HashSet::new();
    for itemref in spine.children().filter(|node| node.has_tag_name("itemref")) {
        let Some(resource) = itemref.attribute("idref").and_then(|id| resources.get(id)) else {
            continue;
        };
        if (resource.html() || resource.media.starts_with("image/"))
            && seen.insert(resource.target.path.clone())
        {
            pending.push((resource, itemref.attribute("linear") != Some("no")));
        }
    }
    ensure!(!pending.is_empty(), "没有可以阅读的 EPUB 章节");
    let html_by_path: HashMap<_, _> = order
        .iter()
        .filter_map(|id| resources.get(id))
        .filter(|resource| resource.html())
        .map(|resource| (resource.target.path.as_str(), resource))
        .collect();
    let mut names = HashMap::new();
    for entry in &navigation {
        names
            .entry(entry.target.path.as_str())
            .or_insert(entry.title.as_str());
        if let Some(&resource) = html_by_path.get(entry.target.path.as_str())
            && seen.insert(entry.target.path.clone())
        {
            pending.push((resource, false));
        }
    }

    let mut sections = vec![];
    let mut linear = vec![];
    let mut destinations = HashMap::new();
    let mut notes = HashMap::new();
    let mut references = vec![];
    let mut bytes = 0;
    let mut next = 0;
    let mut warnings = Vec::new();
    let mut skipped = HashSet::new();
    let mut css = HashMap::<String, ColorSheet>::new();
    while next < pending.len() {
        let (resource, is_linear) = pending[next];
        let file = &resource.target.path;
        let mut parsed = if resource.media.starts_with("image/") {
            Parsed {
                blocks: vec![Block {
                    kind: BlockKind::Image {
                        path: file.clone(),
                        alt: String::new(),
                    },
                    ..Default::default()
                }],
                ..Default::default()
            }
        } else {
            let source = match read_document(&mut zip, file) {
                Ok(source) => source,
                Err(error) => {
                    warnings.push(format!("{error:#}"));
                    skipped.insert(file.clone());
                    next += 1;
                    continue;
                }
            };
            bytes += source.len() as u64;
            ensure!(bytes <= MAX_PUBLICATION_TEXT, "EPUB 正文超过可载入大小");
            parse_page_with_styles(file, &source, |stylesheet| {
                css.entry(stylesheet.to_owned())
                    .or_insert_with(|| {
                        read_entry(&mut zip, stylesheet, 512 * 1024)
                            .ok()
                            .and_then(|bytes| decode_text(&bytes).ok())
                            .map_or_else(ColorSheet::default, |source| read_colors(&source))
                    })
                    .clone()
            })
        };
        next += 1;
        let section = sections.len();
        let heading = parsed
            .blocks
            .first()
            .filter(|block| matches!(block.kind, BlockKind::Heading(_)));
        let title = names
            .get(file.as_str())
            .map(|title| (*title).to_owned())
            .or_else(|| heading.map(|block| block.text.to_string()))
            .or(parsed.title.take())
            .unwrap_or_else(|| format!("第 {} 节", section + 1));
        let header = heading
            .filter(|block| {
                block.text.trim() == title.trim()
                    && !block.spans.iter().any(|span| span.style.link.is_some())
            })
            .map(|_| 0);
        for (id, row) in parsed.anchors {
            destinations.insert(
                Target {
                    path: file.clone(),
                    fragment: id,
                },
                (section, row),
            );
        }
        destinations.insert(
            Target {
                path: file.clone(),
                fragment: String::new(),
            },
            (section, 0),
        );
        notes.extend(parsed.notes);
        let links: Vec<_> = parsed
            .blocks
            .iter()
            .enumerate()
            .flat_map(|(row, block)| {
                block
                    .spans
                    .iter()
                    .filter_map(move |span| match &span.style.link {
                        Some(Link::Local(target)) => {
                            Some((target.clone(), span.style.note_ref, row))
                        }
                        _ => None,
                    })
            })
            .collect();
        for (target, _, _) in &links {
            if let Some(&resource) = html_by_path.get(target.path.as_str())
                && seen.insert(target.path.clone())
            {
                pending.push((resource, false));
            }
        }
        let has_body = !parsed.blocks.is_empty();
        sections.push(Section {
            title: title.into(),
            blocks: parsed.blocks,
            header,
            notes: parsed.note_order,
            note_locations: HashMap::new(),
        });
        references.push(links);
        if is_linear && has_body {
            linear.push(section);
        }
    }
    if linear.is_empty() {
        linear.extend(
            sections
                .iter()
                .enumerate()
                .filter(|(_, data)| !data.blocks.is_empty())
                .map(|(ix, _)| ix),
        );
    }
    if linear.is_empty() {
        bail!(
            "{}",
            warnings
                .first()
                .map_or("EPUB 没有可阅读的正文", String::as_str)
        );
    }

    // EPUB 2 often marks the reference but gives the target only an ordinary id.
    let mut missing_notes: HashMap<&str, HashSet<&str>> = HashMap::new();
    for (target, marked, _) in references.iter().flatten() {
        if !target.fragment.is_empty()
            && !notes.contains_key(target)
            && !skipped.contains(&target.path)
            && (*marked || looks_like_note(&target.fragment))
        {
            missing_notes
                .entry(&target.path)
                .or_default()
                .insert(&target.fragment);
        }
    }
    for (file, ids) in missing_notes {
        if let Ok(source) = read_document(&mut zip, file) {
            let document = Html::parse_document(&source);
            for node in document.select(&selector("[id], a[name]")) {
                if let Some(id) = anchor_id(node).filter(|id| ids.contains(id)) {
                    let container = if matches!(node.value().name(), "a" | "span" | "sup") {
                        node.ancestors()
                            .filter_map(ElementRef::wrap)
                            .find(|parent| {
                                matches!(parent.value().name(), "li" | "p" | "aside" | "dd")
                            })
                            .unwrap_or(node)
                    } else {
                        node
                    };
                    if let Some(note) = make_note(container, id) {
                        notes.insert(
                            Target {
                                path: file.to_owned(),
                                fragment: id.to_owned(),
                            },
                            note,
                        );
                    }
                }
            }
        }
    }

    for (section, links) in sections.iter_mut().zip(references) {
        let own = std::mem::take(&mut section.notes);
        for (target, _, row) in links {
            if notes.contains_key(&target) && !section.note_locations.contains_key(&target) {
                section.note_locations.insert(target.clone(), row);
                section.notes.push(target);
            }
        }
        section.notes.extend(
            own.into_iter()
                .filter(|target| !section.note_locations.contains_key(target)),
        );
    }
    info.cover = cover;
    let mut book = Book::new(info, sections);
    book.skipped_sections = warnings.len();
    book.notes = notes;
    book.destinations = destinations;
    book.set_linear(linear);
    let mut described = HashSet::new();
    for entry in navigation {
        if let Some(&(section, block)) = book.destinations.get(&entry.target).or_else(|| {
            book.destinations.get(&Target {
                path: entry.target.path.clone(),
                fragment: String::new(),
            })
        }) {
            if book.sections[section].blocks.is_empty() {
                continue;
            }
            book.contents.push(ContentsEntry {
                title: entry.title.into(),
                section,
                block,
                depth: entry.depth,
            });
            described.insert(section);
        }
    }
    for &section in &book.linear {
        if !described.contains(&section) {
            let index = book
                .contents
                .iter()
                .position(|entry| entry.section > section)
                .unwrap_or(book.contents.len());
            book.contents.insert(
                index,
                ContentsEntry {
                    title: book.sections[section].title.clone(),
                    section,
                    block: 0,
                    depth: 0,
                },
            );
        }
    }
    Ok(book)
}

fn xml(source: &str) -> Result<roxmltree::Document<'_>> {
    Ok(roxmltree::Document::parse_with_options(
        source,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )?)
}

fn selector(name: &str) -> Selector {
    Selector::parse(name).expect("valid built-in EPUB selector")
}

fn epub3_navigation(path: &str, html: &str) -> Vec<NavEntry> {
    let document = Html::parse_document(html);
    let nav_selector = selector("nav");
    let first = document.select(&nav_selector).next();
    let Some(nav) = document
        .select(&nav_selector)
        .find(|node| semantic(*node, "toc"))
        .or(first)
    else {
        return vec![];
    };
    nav.select(&selector("a[href]"))
        .filter_map(|a| {
            let title = visible_text(a);
            let target = local_target(path, a.value().attr("href")?)?;
            let depth = a
                .ancestors()
                .filter_map(ElementRef::wrap)
                .take_while(|node| node.id() != nav.id())
                .filter(|node| node.value().name() == "li")
                .count()
                .saturating_sub(1);
            (!title.is_empty()).then_some(NavEntry {
                title,
                target,
                depth,
            })
        })
        .collect()
}

fn ncx_navigation(path: &str, source: &str) -> Vec<NavEntry> {
    let Ok(doc) = xml(source) else { return vec![] };
    doc.descendants()
        .filter(|node| node.has_tag_name("navPoint"))
        .filter_map(|item| {
            let title = item
                .children()
                .find(|node| node.has_tag_name("navLabel"))?
                .descendants()
                .find(|node| node.has_tag_name("text"))?
                .text()?
                .trim()
                .to_owned();
            let target = local_target(
                path,
                item.children()
                    .find(|node| node.has_tag_name("content"))?
                    .attribute("src")?,
            )?;
            let depth = item
                .ancestors()
                .filter(|node| node.has_tag_name("navPoint"))
                .count()
                .saturating_sub(1);
            (!title.is_empty()).then_some(NavEntry {
                title,
                target,
                depth,
            })
        })
        .collect()
}

fn semantic(node: ElementRef<'_>, name: &str) -> bool {
    node.value()
        .attrs()
        .filter(|(key, _)| matches!(*key, "epub:type" | "type" | "role"))
        .any(|(_, values)| {
            values
                .split_ascii_whitespace()
                .any(|value| value == name || value.strip_prefix("doc-") == Some(name))
        })
}

fn anchor_id<'a>(node: ElementRef<'a>) -> Option<&'a str> {
    node.value()
        .attr("id")
        .or_else(|| node.value().attr("name"))
}

fn looks_like_note(id: &str) -> bool {
    let id = id.trim_start_matches('_').to_ascii_lowercase();
    ["footnot", "endnot", "rearnot", "ftn", "annotation", "fnote"]
        .iter()
        .any(|prefix| id.starts_with(prefix))
        || ["fn", "note"].iter().any(|prefix| {
            id.strip_prefix(prefix).is_some_and(|suffix| {
                suffix.starts_with(['-', '_', '.'])
                    || suffix.starts_with(|ch: char| ch.is_ascii_digit())
            })
        })
}

fn inline_note_kind(node: ElementRef<'_>) -> Option<NoteKind> {
    let classes = |names: &[&str]| {
        node.value().classes().any(|name| {
            let name = name.to_ascii_lowercase();
            names.iter().any(|key| {
                name == *key
                    || name
                        .strip_prefix(key)
                        .is_some_and(|suffix| suffix.starts_with('-') || suffix.starts_with('_'))
            })
        })
    };
    if semantic(node, "variant")
        || classes(&["variant", "collation", "jiaokan", "校记", "校勘", "异文"])
    {
        Some(NoteKind::Variant)
    } else if classes(&["gloss", "interlinear", "jiazhu", "夹注", "夾注"]) {
        Some(NoteKind::Gloss)
    } else if semantic(node, "annotation")
        || semantic(node, "commentary")
        || classes(&[
            "annotation",
            "commentary",
            "jiapi",
            "pangpi",
            "meipi",
            "huiping",
            "pingdian",
            "夹批",
            "夾批",
            "旁批",
            "眉批",
            "总批",
            "總批",
            "评点",
            "評點",
            "回评",
            "回評",
        ])
    {
        Some(NoteKind::Commentary)
    } else {
        None
    }
}

fn note_node(node: ElementRef<'_>) -> bool {
    ["footnote", "endnote", "rearnote", "glossdef"]
        .iter()
        .any(|name| semantic(node, name))
        || (semantic(node, "annotation")
            && !matches!(
                node.value().name(),
                "span" | "small" | "em" | "i" | "cite" | "font"
            ))
        || (matches!(node.value().name(), "aside" | "li" | "dd" | "div" | "p")
            && node.value().classes().any(|class| {
                matches!(
                    class.to_ascii_lowercase().as_str(),
                    "footnote"
                        | "endnote"
                        | "rearnote"
                        | "footnote-text"
                        | "endnote-text"
                        | "footnote-body"
                        | "endnote-body"
                )
            }))
}

fn note_reference(node: ElementRef<'_>, in_sup: bool) -> bool {
    static SUP: LazyLock<Selector> = LazyLock::new(|| selector("sup"));
    !backlink(node)
        && (["noteref", "annoref", "glossref", "biblioref"]
            .iter()
            .any(|name| semantic(node, name))
            || node.value().classes().any(|class| {
                let class = class.to_ascii_lowercase();
                matches!(
                    class.as_str(),
                    "noteref"
                        | "note-ref"
                        | "fnref"
                        | "fn-ref"
                        | "footnote-reference"
                        | "annotation-ref"
                ) || looks_like_note(&class)
            })
            || in_sup
            || node.select(&SUP).next().is_some())
}

fn image_note_marker(node: ElementRef<'_>, inline: &Inline) -> bool {
    let Some(Link::Local(target)) = &inline.link else {
        return false;
    };
    !target.fragment.is_empty()
        && (inline.note_ref
            || looks_like_note(&target.fragment)
            || node.value().attr("alt").is_some_and(|alt| {
                matches!(
                    alt.trim(),
                    "注" | "註" | "注释" | "脚注" | "尾注" | "校" | "评" | "評"
                )
            })
            || node.value().classes().any(|name| {
                matches!(
                    name.to_ascii_lowercase().as_str(),
                    "noteref" | "note-icon" | "footnote-icon" | "note-ref"
                )
            })
            || node.value().attr("src").is_some_and(|src| {
                let name = src
                    .rsplit('/')
                    .next()
                    .unwrap_or(src)
                    .split('.')
                    .next()
                    .unwrap_or_default();
                matches!(
                    name.to_ascii_lowercase().as_str(),
                    "note"
                        | "noteref"
                        | "note-icon"
                        | "annotation"
                        | "footnote"
                        | "zhushi"
                        | "zhu"
                        | "注"
                        | "註"
                )
            }))
}

fn hidden(node: ElementRef<'_>) -> bool {
    matches!(
        node.value().name(),
        "script"
            | "style"
            | "head"
            | "template"
            | "noscript"
            | "iframe"
            | "audio"
            | "video"
            | "rt"
            | "rp"
    ) || node.value().attr("hidden").is_some()
}

fn backlink(node: ElementRef<'_>) -> bool {
    node.value().name() == "a"
        && (semantic(node, "backlink")
            || node
                .value()
                .classes()
                .any(|class| matches!(class, "footnote-backref" | "reversefootnote"))
            || node
                .value()
                .attr("href")
                .and_then(|href| href.split_once('#'))
                .is_some_and(|(_, id)| id.starts_with("fnref") || id.starts_with("_ftnref"))
            || matches!(
                node.text().collect::<String>().trim(),
                "↩" | "↵" | "↑" | "⤴" | "↶"
            ))
}

fn collect_visible(node: ElementRef<'_>, output: &mut String, preserve_blocks: bool, depth: usize) {
    if depth > 180 || (hidden(node) && !(preserve_blocks && depth == 0)) || backlink(node) {
        return;
    }
    for child in node.children() {
        if let Some(child) = ElementRef::wrap(child) {
            let block = preserve_blocks
                && matches!(
                    child.value().name(),
                    "p" | "li" | "br" | "div" | "blockquote" | "dd"
                );
            if block && !output.ends_with('\n') && !output.is_empty() {
                output.push('\n');
            }
            collect_visible(child, output, preserve_blocks, depth + 1);
            if block && child.value().name() != "br" && !output.ends_with('\n') {
                output.push('\n');
            }
        } else if let Node::Text(text) = child.value() {
            if preserve_blocks {
                output.push_str(&text.replace(['\r', '\n', '\t'], " "));
            } else {
                output.push_str(text);
            }
        }
    }
}

fn visible_text(node: ElementRef<'_>) -> String {
    let mut text = String::new();
    collect_visible(node, &mut text, false, 0);
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn make_note(node: ElementRef<'_>, id: &str) -> Option<SourceNote> {
    let mut text = String::new();
    collect_visible(node, &mut text, true, 0);
    let text = text
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if text.is_empty() {
        return None;
    }
    let suffix: String = id
        .chars()
        .rev()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    let id = id.to_ascii_lowercase();
    let kind = if semantic(node, "endnote")
        || semantic(node, "rearnote")
        || id.starts_with("endnote")
        || node
            .value()
            .classes()
            .any(|class| class.eq_ignore_ascii_case("endnote"))
    {
        NoteKind::Endnote
    } else if semantic(node, "glossdef") {
        NoteKind::Gloss
    } else {
        inline_note_kind(node).unwrap_or(NoteKind::Footnote)
    };
    let label = kind.label();
    let title = if suffix.is_empty() {
        label.into()
    } else {
        format!("{label} {suffix}").into()
    };
    Some(SourceNote {
        title,
        text: text.into(),
        kind,
    })
}

#[derive(Default)]
pub(crate) struct Parsed {
    pub blocks: Vec<Block>,
    pub notes: HashMap<Target, SourceNote>,
    pub note_order: Vec<Target>,
    pub anchors: HashMap<String, usize>,
    pub title: Option<String>,
}

pub(crate) fn parse_page(path: &str, source: &str) -> Parsed {
    parse_page_with_styles(path, source, |_| ColorSheet::default())
}

fn parse_page_with_styles(
    path: &str,
    source: &str,
    mut read_styles: impl FnMut(&str) -> ColorSheet,
) -> Parsed {
    let html = Html::parse_document(source);
    static STYLES: LazyLock<Selector> = LazyLock::new(|| selector("style, link[href]"));
    let styles: Vec<_> = html
        .select(&STYLES)
        .take(32)
        .filter_map(|node| {
            if node.value().name() == "style" {
                Some(read_colors(&node.text().collect::<String>()))
            } else if node.value().attr("rel").is_some_and(|rel| {
                rel.split_ascii_whitespace()
                    .any(|value| value.eq_ignore_ascii_case("stylesheet"))
            }) {
                let sheet = local_target(path, node.value().attr("href")?)?;
                (sheet.path != path).then(|| read_styles(&sheet.path))
            } else {
                None
            }
        })
        .filter(|sheet| !sheet.is_empty())
        .collect();
    let Some(body) = html.select(&selector("body")).next() else {
        return Parsed::default();
    };
    let mut builder = PageBuilder {
        path,
        result: Parsed::default(),
        text: String::new(),
        spans: vec![],
        pending_space: false,
        styles: &styles,
        attribute_note_count: 0,
    };
    builder.result.title = html
        .select(&selector("title"))
        .next()
        .map(visible_text)
        .filter(|s| !s.is_empty());
    for node in body.select(&selector("[id], a[name]")) {
        if (note_node(node) || semantic(node, "annotation"))
            && let Some(id) = anchor_id(node)
            && let Some(note) = make_note(node, id)
        {
            let target = Target {
                path: path.to_owned(),
                fragment: id.to_owned(),
            };
            if builder.result.notes.insert(target.clone(), note).is_none() {
                builder.result.note_order.push(target);
            }
        }
    }
    builder.walk(body, &Inline::default(), &BlockKind::Paragraph, 0, false);
    builder.flush(&BlockKind::Paragraph);
    let last = builder.result.blocks.len().saturating_sub(1);
    for block in builder.result.anchors.values_mut() {
        *block = (*block).min(last);
    }
    builder.result
}

struct PageBuilder<'a> {
    path: &'a str,
    result: Parsed,
    text: String,
    spans: Vec<Span>,
    pending_space: bool,
    styles: &'a [ColorSheet],
    attribute_note_count: usize,
}

impl PageBuilder<'_> {
    fn append(&mut self, text: &str, inline: &Inline, pre: bool) {
        let start = self.text.len();
        if pre {
            self.text.push_str(text);
        } else {
            for ch in text.chars() {
                if matches!(ch, ' ' | '\n' | '\r' | '\t' | '\x0c') {
                    self.pending_space = !self.text.is_empty() && !self.text.ends_with('\n');
                } else {
                    if std::mem::take(&mut self.pending_space) {
                        self.text.push(' ');
                    }
                    self.text.push(ch);
                }
            }
        }
        if *inline != Inline::default() && start < self.text.len() {
            let range = start..self.text.len();
            if let Some(last) = self
                .spans
                .last_mut()
                .filter(|last| last.style == *inline && last.range.end == start)
            {
                last.range.end = range.end;
            } else {
                self.spans.push(Span {
                    range,
                    style: inline.clone(),
                });
            }
        }
    }

    fn flush(&mut self, kind: &BlockKind) {
        let text = self.text.trim_end_matches('\n');
        if !text.is_empty() {
            let mut offset = 0;
            let mut span_start = 0;
            for chunk in split_block(text, 4096) {
                let end = offset + chunk.len();
                while self
                    .spans
                    .get(span_start)
                    .is_some_and(|span| span.range.end <= offset)
                {
                    span_start += 1;
                }
                let spans = self.spans[span_start..]
                    .iter()
                    .take_while(|span| span.range.start < end)
                    .map(|span| Span {
                        range: (span.range.start.max(offset) - offset)
                            ..(span.range.end.min(end) - offset),
                        style: span.style.clone(),
                    })
                    .collect();
                self.result.blocks.push(Block {
                    text: chunk.to_owned().into(),
                    kind: kind.clone(),
                    spans,
                });
                offset = end;
            }
        }
        self.text.clear();
        self.spans.clear();
        self.pending_space = false;
    }

    fn walk(
        &mut self,
        node: ElementRef<'_>,
        inline: &Inline,
        parent: &BlockKind,
        depth: usize,
        in_sup: bool,
    ) {
        if depth > 180
            || hidden(node)
            || note_node(node)
            || ["footnotes", "endnotes", "rearnotes"]
                .iter()
                .any(|name| semantic(node, name))
        {
            return;
        }
        let tag = node.value().name();
        let block = match tag {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                Some(BlockKind::Heading(tag.as_bytes()[1] - b'0'))
            }
            "blockquote" => Some(BlockKind::Quote),
            "pre" => Some(BlockKind::Code),
            "li" => Some(BlockKind::List),
            "p" if node
                .value()
                .classes()
                .any(|class| matches!(class, "verse" | "poem" | "stanza")) =>
            {
                Some(BlockKind::Verse)
            }
            "p" | "section" | "article" | "div" | "header" | "footer" | "nav" | "aside"
            | "figure" | "figcaption" | "ol" | "ul" | "dl" | "dt" | "dd" | "table" | "tr"
            | "address" => Some(parent.clone()),
            _ => None,
        };
        if matches!(tag, "img" | "image") && image_note_marker(node, inline) {
            let mut marker = inline.clone();
            marker.note_ref = true;
            self.append("〔注〕", &marker, false);
            return;
        }
        if block.is_some() || matches!(tag, "img" | "image" | "hr") {
            self.flush(parent);
        }
        if let Some(id) = anchor_id(node) {
            self.result
                .anchors
                .entry(id.to_owned())
                .or_insert(self.result.blocks.len());
        }
        let kind = block.as_ref().unwrap_or(parent);
        let mut inline = inline.clone();
        if let Some(color) = text_color(node, self.styles) {
            inline.color = Some(color);
        }
        if let Some(kind) = inline_note_kind(node) {
            inline.note_kind = Some(kind);
        }
        let start = self.text.len();
        match tag {
            "img" | "image" => {
                let alt = node.value().attr("alt").unwrap_or("").to_owned();
                let src = node
                    .value()
                    .attr("src")
                    .or_else(|| node.value().attr("href"))
                    .or_else(|| node.value().attr("xlink:href"));
                if let Some(target) = src
                    .and_then(|src| local_target(self.path, src))
                    .filter(|target| target.path != self.path)
                {
                    self.result.blocks.push(Block {
                        kind: BlockKind::Image {
                            path: target.path,
                            alt,
                        },
                        ..Default::default()
                    });
                } else if !alt.is_empty() {
                    self.result.blocks.push(Block::paragraph(alt));
                }
                return;
            }
            "hr" => {
                self.result.blocks.push(Block {
                    kind: BlockKind::Rule,
                    ..Default::default()
                });
                return;
            }
            "br" => {
                self.pending_space = false;
                self.text.push('\n');
                return;
            }
            "td" | "th" if !self.text.is_empty() => self.append("　│　", &Inline::default(), false),
            "strong" | "b" | "th" => inline.bold = true,
            "em" | "i" | "cite" => inline.italic = true,
            "u" => inline.underline = true,
            "s" | "strike" | "del" => inline.strike = true,
            "code" | "kbd" => inline.code = true,
            "mark" => inline.marked = true,
            "a" => {
                inline.link = node
                    .value()
                    .attr("href")
                    .and_then(|href| link_for(self.path, href));
                inline.note_ref = note_reference(node, in_sup);
            }
            _ => {}
        }
        if inline.link.is_none() {
            static RUBY_TEXT: LazyLock<Selector> = LazyLock::new(|| selector("rt"));
            let note = if tag == "ruby" {
                node.select(&RUBY_TEXT)
                    .map(|ruby| ruby.text().collect::<String>())
                    .filter(|reading| !reading.trim().is_empty())
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                node.value()
                    .attr("data-note")
                    .or_else(|| {
                        matches!(tag, "abbr" | "dfn")
                            .then(|| node.value().attr("title"))
                            .flatten()
                    })
                    .unwrap_or_default()
                    .to_owned()
            };
            let note = note.trim();
            if !note.is_empty() {
                let kind = inline.note_kind.unwrap_or(NoteKind::Gloss);
                let target = Target {
                    path: self.path.to_owned(),
                    fragment: format!("\0moye-note-{}", self.attribute_note_count),
                };
                self.attribute_note_count += 1;
                self.result.notes.insert(
                    target.clone(),
                    SourceNote {
                        kind,
                        title: kind.label().into(),
                        text: note.chars().take(16_384).collect::<String>().into(),
                    },
                );
                self.result.note_order.push(target.clone());
                inline.link = Some(Link::Local(target));
                inline.note_ref = true;
            }
        }
        for child in node.children() {
            if let Some(el) = ElementRef::wrap(child) {
                self.walk(el, &inline, kind, depth + 1, in_sup || tag == "sup");
            } else if let Node::Text(text) = child.value() {
                self.append(text, &inline, *kind == BlockKind::Code);
            }
        }
        if tag == "a"
            && self.text.len() == start
            && inline.note_ref
            && matches!(&inline.link, Some(Link::Local(target)) if !target.fragment.is_empty())
        {
            self.append("〔注〕", &inline, false);
        }
        if block.is_some() {
            self.flush(kind);
        }
    }
}
