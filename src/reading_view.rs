use std::{ops::Range, rc::Rc, sync::LazyLock};

use regex::Regex;

use gpui_kit::{prelude::FluentBuilder as _, *};

use crate::{
    book::{Block, BlockKind, DEMO_ID, Link, NoteKind, Passage, PointInText},
    library::Tone,
    pagination::{LineStart, Pagination, Row},
    reader::{Drag, ImageState, Reader},
    style::{self, Palette, SERIF},
};

const PAGE_MAX_WIDTH: Pixels = px(689.);
const PAGE_VERTICAL_INSET: Pixels = px(20.);
const SPINE_WIDTH: Pixels = px(1.);

impl Reader {
    pub fn page(&self, p: Palette, window: &Window, cx: &Context<Self>) -> AnyElement {
        let view = cx.entity();
        if let Some(layout) = self.pagination.as_ref().filter(|_| self.is_paged()) {
            let first = self.visible_pages().start;
            let leaves = div()
                .h(layout.area.height + PAGE_VERTICAL_INSET * 2.)
                .rounded(px(7.))
                .shadow_sm()
                .overflow_hidden()
                .bg(rgb(p.paper))
                .flex()
                .child(self.page_leaf(first, layout, p, &view, window))
                .when(self.is_spread(), |spread| {
                    spread
                        .child(div().w(SPINE_WIDTH).h_full().bg(rgb(p.line)))
                        .child(self.page_leaf(first + 1, layout, p, &view, window))
                });
            return div()
                .flex_1()
                .min_h_0()
                .w_full()
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .bg(rgb(p.rail))
                .child(leaves)
                .into_any_element();
        }
        list(self.list.clone(), move |index, window, app| {
            view.read(app).page_row(index, p, &view, window, None).0
        })
        .size_full()
        .flex_1()
        .min_h_0()
        .into_any_element()
    }

    fn page_leaf(
        &self,
        index: usize,
        layout: &Pagination,
        p: Palette,
        view: &Entity<Self>,
        window: &Window,
    ) -> impl IntoElement {
        let area = layout.area;
        let inset = self.page_inset(Some(area));
        let marker_left = ((area.width - PAGE_MAX_WIDTH) / 2.).max(px(0.)) + inset
            - if self.is_spread() { px(15.) } else { px(25.) };
        div()
            .id(("leaf", index))
            .w(area.width)
            .h_full()
            .py(PAGE_VERTICAL_INSET)
            .flex_shrink_0()
            .children(layout.pages.get(index).into_iter().flatten().map(|part| {
                let marked = self.bookmarks().iter().any(|mark| {
                    mark.section == self.section
                        && mark.item == part.start.item
                        && mark.byte >= part.start.byte
                        && part.end_byte.is_none_or(|end| mark.byte < end)
                });
                div()
                    .relative()
                    .w_full()
                    .h(part.height)
                    .overflow_hidden()
                    .child(
                        div().absolute().top(-part.offset).w_full().child(
                            self.page_row(part.start.item, p, view, window, Some(area))
                                .0,
                        ),
                    )
                    .when(marked, |el| {
                        el.child(
                            div()
                                .absolute()
                                .left(marker_left)
                                .top(px(8.))
                                .child(style::icon(
                                    style::BOOKMARKED,
                                    if self.is_spread() { 12. } else { 14. },
                                    p.green,
                                )),
                        )
                    })
            }))
    }

    fn page_inset(&self, area: Option<Size<Pixels>>) -> Pixels {
        match area.filter(|_| self.is_spread()) {
            Some(area) => (area.width * 0.068).clamp(px(17.), px(39.)),
            None => px(42.),
        }
    }

    pub fn sync_pagination(
        &mut self,
        pane: Size<Pixels>,
        p: Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_paged() {
            if let Some(anchor) = self.pending_scroll_anchor.take() {
                let (mut row, text) = self.page_row(anchor.item, p, &cx.entity(), window, None);
                let metric = measure_row(&mut row, text, pane.width, window, cx);
                self.list.scroll_to(ListOffset {
                    item_ix: anchor.item,
                    offset_in_item: metric.offset_for(anchor.byte),
                });
                self.capture_position();
                self.schedule_save(window, cx);
            }
            return;
        }
        let page_width = if self.is_spread() {
            ((pane.width - px(24.) - SPINE_WIDTH) / 2.).min(px(570.))
        } else {
            pane.width.min(px(770.))
        };
        let area = size(page_width, (pane.height - px(66.)).max(px(120.)));
        let font_size = self.library.settings.size;
        let scroll_offset = self.pending_page_offset.take();
        if let Some(offset) = scroll_offset {
            let (mut row, text) =
                self.page_row(self.page_anchor.item, p, &cx.entity(), window, None);
            let metric = measure_row(&mut row, text, pane.width, window, cx);
            self.page_anchor.byte = metric.byte_for(offset);
        }
        let changed = self
            .pagination
            .as_ref()
            .is_none_or(|layout| layout.area != area || layout.font_size != font_size);
        if changed {
            let view = cx.entity();
            let rows = (0..=self.data().blocks.len())
                .map(|item| {
                    if item > 0
                        && matches!(self.data().blocks[item - 1].kind, BlockKind::Image { .. })
                    {
                        return Row {
                            height: area.height,
                            lines: Vec::new(),
                        };
                    }
                    let (mut row, text) = self.page_row(item, p, &view, window, Some(area));
                    measure_row(&mut row, text, area.width, window, cx)
                })
                .collect();
            self.pagination = Some(Pagination::new(area, font_size, rows));
        }
        if changed || self.pending_page_end || scroll_offset.is_some() {
            let layout = self.pagination.as_ref().unwrap();
            self.page_index = if std::mem::take(&mut self.pending_page_end) {
                let last = layout.pages.len() - 1;
                let first =
                    crate::pagination::page_range(last, layout.pages.len(), self.pages_per_turn())
                        .start;
                if let Some(fragment) = layout.pages[first].first() {
                    self.page_anchor = fragment.start;
                }
                first
            } else {
                layout.page_for(self.page_anchor)
            };
            self.capture_position();
            self.schedule_save(window, cx);
        }
    }

    fn page_row(
        &self,
        index: usize,
        p: Palette,
        view: &Entity<Self>,
        window: &Window,
        page_area: Option<Size<Pixels>>,
    ) -> (AnyElement, Option<(TextLayout, Pixels)>) {
        let frame = || div().w_full().flex().flex_col().items_center();
        let marked = page_area.is_none()
            && self
                .bookmarks()
                .iter()
                .any(|mark| mark.section == self.section && mark.item == index);
        let column = || {
            div()
                .w_full()
                .max_w(PAGE_MAX_WIDTH)
                .px(self.page_inset(page_area))
                .relative()
                .when(marked, |row| {
                    row.child(
                        div()
                            .absolute()
                            .left(px(17.))
                            .top(px(if index == 0 { 50. } else { 8. }))
                            .child(style::icon(style::BOOKMARKED, 14., p.green)),
                    )
                })
        };
        if index == 0 {
            let spread = page_area.is_some() && self.is_spread();
            let number = match self.book.linear.iter().position(|ix| *ix == self.section) {
                Some(number) => format!("第 {:02} 章", number + 1),
                None => "补充章节".to_owned(),
            };
            return (
                frame()
                    .child(
                        column()
                            .pt(px(if spread { 34. } else { 49. }))
                            .pb(px(if spread { 26. } else { 32. }))
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(9.))
                                    .child(div().w(px(16.)).h(px(1.)).bg(rgb(p.green)))
                                    .child(
                                        style::label(number, 11., p.green)
                                            .font_weight(FontWeight::SEMIBOLD),
                                    ),
                            )
                            .child(
                                style::label(
                                    self.data().title.clone(),
                                    if spread { 31. } else { 38. },
                                    p.ink,
                                )
                                .font_family(SERIF)
                                .font_weight(FontWeight::BOLD)
                                .line_height(px(if spread { 43. } else { 54. }))
                                .mt(px(10.))
                                .when(page_area.is_some(), |el| {
                                    el.line_clamp(if spread { 4 } else { 3 })
                                }),
                            )
                            .child(
                                style::label(
                                    if self.book.info.id == DEMO_ID {
                                        "雾岸来信  /  内置试读".to_owned()
                                    } else if self.book.info.author.is_empty() {
                                        self.book.info.title.clone()
                                    } else {
                                        format!(
                                            "{}  /  {}",
                                            self.book.info.title, self.book.info.author
                                        )
                                    },
                                    12.,
                                    p.dim,
                                )
                                .mt(px(7.))
                                .when(page_area.is_some(), |el| el.line_clamp(1)),
                            )
                            .child(div().mt(px(25.)).child(style::line(p.line))),
                    )
                    .into_any_element(),
                None,
            );
        }
        let block_index = index - 1;
        let Some(block) = self.data().blocks.get(block_index) else {
            let name = self
                .neighbor(true)
                .map(|ix| self.book.sections[ix].title.clone());
            let view = view.downgrade();
            return (
                frame()
                    .child(
                        column()
                            .pt(px(14.))
                            .pb(px(42.))
                            .child(if let Some(name) = name {
                                div()
                                    .id("end-next")
                                    .w_full()
                                    .px(px(14.))
                                    .py(px(12.))
                                    .rounded(px(8.))
                                    .border_1()
                                    .border_color(rgb(p.line))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(p.hover)))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(style::label("下一章", 12., p.dim))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.))
                                            .child(style::label(name, 13., p.green))
                                            .child(style::icon(style::RIGHT, 14., p.green)),
                                    )
                                    .on_click(move |_, _, app| {
                                        let _ =
                                            view.update(app, |reader, cx| reader.step(true, cx));
                                    })
                                    .into_any_element()
                            } else {
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(12.))
                                    .child(div().w(px(25.)).h(px(1.)).bg(rgb(p.line)))
                                    .child(style::label("全书完", 12., p.dim))
                                    .child(div().w(px(25.)).h(px(1.)).bg(rgb(p.line)))
                                    .into_any_element()
                            }),
                    )
                    .into_any_element(),
                None,
            );
        };
        if self.data().header == Some(block_index) {
            return (div().h_0().into_any_element(), None);
        }
        if let BlockKind::Image { path, alt } = &block.kind {
            let child = self.picture_view(path, alt, p, view, window, page_area);
            return (
                frame()
                    .child(
                        column()
                            .when_some(page_area, |el, area| {
                                el.h(area.height).flex().flex_col().justify_center()
                            })
                            .when(page_area.is_none(), |el| el.pb(px(18.)))
                            .child(child),
                    )
                    .into_any_element(),
                None,
            );
        }
        if block.kind == BlockKind::Rule {
            return (
                frame()
                    .child(column().py(px(21.)).child(style::line(p.line)))
                    .into_any_element(),
                None,
            );
        }
        let (paragraph, layout) = self.paragraph(block, block_index, p, view);
        (
            frame().child(column().child(paragraph)).into_any_element(),
            Some(layout),
        )
    }

    fn picture_view(
        &self,
        path: &str,
        alt: &str,
        p: Palette,
        view: &Entity<Self>,
        window: &Window,
        page_area: Option<Size<Pixels>>,
    ) -> AnyElement {
        if self.picture(&self.book.info, path).is_none() {
            let view = view.downgrade();
            let book = self.book.info.clone();
            let path = path.to_owned();
            window.on_next_frame(move |_, app| {
                let _ = view.update(app, |reader, cx| reader.request_picture(book, path, cx));
            });
        }
        match self.picture(&self.book.info, path) {
            Some(ImageState::Ready(picture)) => {
                let width = picture.width as f32;
                let height = picture.height as f32;
                let (max_width, max_height) = page_area.map_or((530., 540.), |area| {
                    let caption = if alt.is_empty() { 0. } else { 46. };
                    (
                        f32::from(area.width.min(PAGE_MAX_WIDTH) - self.page_inset(page_area) * 2.)
                            .max(1.),
                        f32::from(area.height).min(850.) - caption,
                    )
                });
                let factor = (max_height / height).min(max_width / width).min(1.);
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        img(picture.art.clone())
                            .w(px(width * factor))
                            .max_w_full()
                            .h(px(height * factor))
                            .object_fit(ObjectFit::Contain),
                    )
                    .when(!alt.is_empty(), |el| {
                        el.child(
                            style::label(alt.to_owned(), 12., p.dim)
                                .when(page_area.is_some(), |el| {
                                    el.line_height(px(18.)).line_clamp(2)
                                }),
                        )
                    })
                    .into_any_element()
            }
            Some(ImageState::Failed) => style::label(
                if alt.is_empty() {
                    "无法显示此插图".to_owned()
                } else {
                    format!("插图 · {alt}")
                },
                12.,
                p.dim,
            )
            .into_any_element(),
            _ => style::label("正在载入插图…", 12., p.dim)
                .py(px(25.))
                .into_any_element(),
        }
    }

    fn paragraph(
        &self,
        block: &Block,
        block_index: usize,
        p: Palette,
        view: &Entity<Self>,
    ) -> (AnyElement, (TextLayout, Pixels)) {
        let font_size = f32::from(self.library.settings.size);
        let selected = self
            .selection
            .as_ref()
            .and_then(|passage| passage.in_block(self.section, block_index, block.text.len()));
        let mut underlines = vec![];
        for note in self.annotations().iter().rev() {
            if self.unlocated_annotations.contains(&note.id) {
                continue;
            }
            if let Some(range) = note
                .passage
                .in_block(self.section, block_index, block.text.len())
            {
                underlines.push((range, note.tone, self.revealed == Some(note.id)));
            }
        }
        let highlights = self.highlights(block, selected.as_ref(), &underlines, p);
        let font_overrides = block
            .spans
            .iter()
            .filter(|span| span.style.code)
            .map(|span| (span.range.clone(), SharedString::from("Menlo")));
        let text = StyledText::new(block.text.clone())
            .with_highlights(highlights)
            .with_font_family_overrides(font_overrides);
        let layout = text.layout().clone();
        let measured_layout = layout.clone();
        let motion_layout = layout.clone();
        let links = Rc::new(
            block
                .spans
                .iter()
                .filter_map(|span| {
                    span.style
                        .link
                        .clone()
                        .map(|link| (span.range.clone(), link))
                })
                .collect::<Vec<_>>(),
        );
        let watch = view.downgrade();
        let track = view.downgrade();
        let text_len = block.text.len();
        let actual_text = block.text.clone();
        let motion_text = actual_text.clone();
        let b = div()
            .id(("block", block_index))
            .w_full()
            .text_color(rgb(p.text))
            .font_family(SERIF)
            .text_size(px(font_size))
            .line_height(px(font_size * 1.84))
            .cursor(CursorStyle::IBeam)
            .pb(px(17.))
            .child(text)
            .on_mouse_down(MouseButton::Left, move |event, window, app| {
                let hit = layout.index_for_position(event.position);
                let byte = nearest_boundary(&actual_text, hit.unwrap_or_else(|ix| ix));
                let link = hit.ok().and_then(|ix| {
                    links
                        .iter()
                        .find(|(range, _)| range.contains(&ix))
                        .map(|(_, link)| link.clone())
                });
                let _ = watch.update(app, |reader, cx| {
                    if reader.editor.is_some() || reader.busy {
                        return;
                    }
                    reader.focus.focus(window, cx);
                    reader.settings_open = false;
                    reader.anchor = None;
                    reader.selection = None;
                    let point = PointInText {
                        block: block_index,
                        byte,
                    };
                    if event.click_count > 1 && (link.is_none() || event.click_count > 2) {
                        let range = if event.click_count > 2 {
                            0..text_len
                        } else {
                            word_at(&actual_text, byte)
                        };
                        reader.selection = Passage::new(
                            reader.section,
                            PointInText {
                                block: block_index,
                                byte: range.start,
                            },
                            PointInText {
                                block: block_index,
                                byte: range.end,
                            },
                        );
                        reader.anchor = Some(event.position);
                        reader.drag = None;
                    } else {
                        reader.drag = Some(Drag {
                            start: point,
                            end: point,
                            link,
                            origin: event.position,
                            moved: false,
                        });
                    }
                    cx.notify();
                });
            })
            .on_mouse_move(move |event, _, app| {
                if event.pressed_button != Some(MouseButton::Left) {
                    return;
                }
                let byte = nearest_boundary(
                    &motion_text,
                    motion_layout
                        .index_for_position(event.position)
                        .unwrap_or_else(|ix| ix),
                );
                let _ = track.update(app, |reader, cx| {
                    let Some(drag) = reader.drag.as_mut() else {
                        return;
                    };
                    let point = PointInText {
                        block: block_index,
                        byte,
                    };
                    if drag.end == point {
                        return;
                    }
                    drag.end = point;
                    drag.moved = true;
                    reader.selection = Passage::new(reader.section, drag.start, point);
                    cx.notify();
                });
            });
        let stylized = match block.kind {
            BlockKind::Heading(level) => b
                .pt(px(8.))
                .text_size(px(font_size + if level <= 2 { 7. } else { 3. }))
                .line_height(px(font_size * 2.1))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(p.ink)),
            BlockKind::Quote => b
                .pl(px(16.))
                .pr(px(12.))
                .pt(px(5.))
                .pb(px(9.))
                .mb(px(17.))
                .border_l_2()
                .border_color(rgb(p.green))
                .bg(rgb(p.green_bg))
                .text_color(rgb(p.green)),
            BlockKind::List => b.pl(px(22.)),
            BlockKind::Verse => b.pl(px(22.)).italic(),
            BlockKind::Code => b
                .font_family("Menlo")
                .text_size(px(font_size * 0.84))
                .p(px(11.))
                .mb(px(17.))
                .rounded(px(5.))
                .bg(rgb(p.rail)),
            _ => b,
        };
        let text_top = match block.kind {
            BlockKind::Heading(_) => 8.,
            BlockKind::Quote => 5.,
            BlockKind::Code => 11.,
            _ => 0.,
        };
        (stylized.into_any_element(), (measured_layout, px(text_top)))
    }

    fn highlights(
        &self,
        block: &Block,
        selected: Option<&Range<usize>>,
        underlines: &[(Range<usize>, Tone, bool)],
        p: Palette,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        let comments = bracketed_notes(&block.text);
        let mut bounds = vec![];
        for range in block
            .spans
            .iter()
            .map(|span| &span.range)
            .chain(underlines.iter().map(|(range, _, _)| range))
            .chain(comments.iter().map(|(range, _)| range))
            .chain(selected)
        {
            if block.text.is_char_boundary(range.start) && block.text.is_char_boundary(range.end) {
                bounds.extend([range.start, range.end]);
            }
        }
        bounds.sort_unstable();
        bounds.dedup();
        let mut highlights = Vec::new();
        let mut spans = block.spans.iter().peekable();
        for indices in bounds.windows(2) {
            let range = indices[0]..indices[1];
            let mut style = HighlightStyle::default();
            while spans
                .peek()
                .is_some_and(|span| span.range.end <= range.start)
            {
                spans.next();
            }
            if let Some((_, kind)) = comments
                .iter()
                .find(|(comment, _)| comment.contains(&range.start))
            {
                style.color = Some(rgb(p.note_color(*kind)).into());
            }
            if let Some(span) = spans
                .peek()
                .filter(|span| span.range.contains(&range.start))
            {
                let inline = &span.style;
                let source_note = match &inline.link {
                    Some(Link::Local(target)) => self.book.notes.get(target),
                    _ => None,
                };
                let is_note = inline.note_ref || source_note.is_some();
                if inline.bold || is_note {
                    style.font_weight = Some(FontWeight::SEMIBOLD);
                }
                if inline.italic {
                    style.font_style = Some(FontStyle::Italic);
                }
                if let Some(kind) = inline.note_kind {
                    style.color = Some(rgb(p.note_color(kind)).into());
                }
                if let Some(color) = inline.color {
                    let color = p.epub_ink(color);
                    if color.s >= 0.13 || inline.note_kind.is_none() {
                        style.color = Some(color);
                    }
                }
                if is_note {
                    let accent =
                        rgb(p.note_color(source_note.map_or(NoteKind::Footnote, |note| note.kind)));
                    style.color = Some(accent.into());
                    if source_note.is_some() {
                        let tint: Hsla = accent.into();
                        style.background_color = Some(tint.opacity(0.11));
                    }
                } else if inline.link.is_some() {
                    style.color = Some(rgb(p.green).into());
                }
                if inline.marked {
                    style.background_color = Some(rgb(p.tone(Tone::Amber).1).into());
                }
                if (inline.link.is_some() && !is_note) || inline.underline {
                    style.underline = Some(UnderlineStyle {
                        thickness: px(1.),
                        color: Some(rgb(p.green).into()),
                        wavy: false,
                    });
                }
                if inline.strike {
                    style.strikethrough = Some(StrikethroughStyle {
                        thickness: px(1.),
                        color: None,
                    });
                }
            }
            if let Some((_, tone, revealed)) = underlines
                .iter()
                .find(|(line, _, _)| line.contains(&range.start))
            {
                let (color, shade) = p.tone(*tone);
                style.underline = Some(UnderlineStyle {
                    thickness: if *revealed { px(2.) } else { px(1.4) },
                    color: Some(rgb(color).into()),
                    wavy: false,
                });
                style.background_color = Some(rgb(shade).into());
            }
            if selected.is_some_and(|selection| selection.contains(&range.start)) {
                style.background_color = Some(rgb(p.selection).into());
            }
            if style != HighlightStyle::default() {
                highlights.push((range, style));
            }
        }
        highlights
    }

    pub fn end_drag(&mut self, event: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.take() else { return };
        let difference = event.position - drag.origin;
        if !drag.moved && difference.x.abs() <= px(4.) && difference.y.abs() <= px(4.) {
            if let Some(link) = drag.link {
                self.follow(link, window, cx);
            }
        } else if self.selection.is_some() {
            self.anchor = Some(event.position);
        }
        cx.notify();
    }
}

pub(crate) fn bracketed_notes(text: &str) -> Vec<(Range<usize>, NoteKind)> {
    static BRACKETS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"[〖【〔]([^〖〗【】〔〕
：:]{1,24})[：:][^〖〗【】〔〕]*[〗】〕]",
        )
        .unwrap()
    });
    if !text.contains(['〖', '【', '〔']) {
        return vec![];
    }
    BRACKETS
        .captures_iter(text)
        .filter_map(|capture| {
            let label = &capture[1];
            let kind = if ["校记", "校勘", "异文", "校記", "異文"]
                .iter()
                .any(|word| label.contains(word))
            {
                NoteKind::Variant
            } else if [
                "夹批", "夹评", "旁批", "旁评", "眉批", "总批", "回评", "评点", "評點", "夾批",
                "夾評", "旁評", "總批", "回評",
            ]
            .iter()
            .any(|word| label.contains(word))
            {
                NoteKind::Commentary
            } else if matches!(
                label.trim(),
                "注" | "註" | "夹注" | "夾注" | "词注" | "詞註"
            ) {
                NoteKind::Gloss
            } else {
                return None;
            };
            Some((capture.get(0)?.range(), kind))
        })
        .collect()
}

fn measure_row(
    row: &mut AnyElement,
    text: Option<(TextLayout, Pixels)>,
    width: Pixels,
    window: &mut Window,
    cx: &mut App,
) -> Row {
    let height = row
        .layout_as_root(
            size(AvailableSpace::Definite(width), AvailableSpace::MinContent),
            window,
            cx,
        )
        .height;
    let mut lines = Vec::new();
    if let Some((layout, top)) = text {
        let line_height = layout.line_height();
        let mut offset = top;
        let mut byte = 0;
        for line in layout.line_layouts() {
            if byte > 0 {
                lines.push(LineStart { byte, offset });
            }
            for wrap in line.wrap_boundaries() {
                offset += line_height;
                let glyph = &line.unwrapped_layout.runs[wrap.run_ix].glyphs[wrap.glyph_ix];
                lines.push(LineStart {
                    byte: byte + glyph.index,
                    offset,
                });
            }
            byte += line.len() + 1;
            offset += line_height;
        }
    }
    Row { height, lines }
}

fn nearest_boundary(text: &str, mut index: usize) -> usize {
    index = index.min(text.len());
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn word_at(text: &str, byte: usize) -> Range<usize> {
    let Some(ch) = text.get(byte..).and_then(|tail| tail.chars().next()) else {
        return text
            .char_indices()
            .next_back()
            .map_or(0..0, |(last, _)| last..text.len());
    };
    if ch.is_ascii_alphanumeric() {
        let start = text[..byte]
            .bytes()
            .rev()
            .take_while(|byte| byte.is_ascii_alphanumeric())
            .count();
        let len = text[byte..]
            .bytes()
            .take_while(|byte| byte.is_ascii_alphanumeric())
            .count();
        (byte - start)..(byte + len)
    } else {
        byte..(byte + ch.len_utf8())
    }
}
