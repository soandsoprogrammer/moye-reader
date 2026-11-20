use std::{fs, io::Write, path::Path};

use zip::{ZipWriter, write::SimpleFileOptions};

use crate::{
    book::{BookInfo, Format, Passage, PointInText, Target, parse_txt},
    epub::parse_epub,
    library::{Annotation, Library, Position, Tone},
};

fn info(format: Format) -> BookInfo {
    BookInfo {
        id: "0".repeat(64),
        title: "测试书籍".to_owned(),
        author: String::new(),
        format,
        cover: None,
    }
}

#[test]
fn mixed_reading_text_keeps_inline_code_in_its_own_font() {
    use crate::{
        book::{Block, Inline, Span},
        reading_view::reading_font_overrides,
    };
    let mut block = Block::paragraph("此 EPUB 用 e\u{301} 和 code 排版。");
    let code = block.text.find("code").unwrap();
    block.spans.push(Span {
        range: code..code + 4,
        style: Inline {
            code: true,
            ..Inline::default()
        },
    });
    let overrides = reading_font_overrides(&block);
    let runs = overrides
        .iter()
        .map(|(range, font)| (block.text[range.clone()].trim(), font.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(runs, [("code", "Menlo")]);
    assert!(reading_font_overrides(&Block::paragraph("EPUB files")).is_empty());
}

#[test]
fn shelf_folders_round_trip_and_keep_reading_records_when_removed() {
    let mut library: Library = serde_json::from_str("{}").unwrap();
    let book = info(Format::Txt);
    let key = book.id.clone();
    library.books.push(book);
    library
        .reading
        .entry(key.clone())
        .or_default()
        .position
        .item = 3;
    assert!(library.folders.is_empty() && library.book_folders.is_empty());

    let folder = library.name_folder(None, " Classics ").unwrap();
    assert!(library.name_folder(None, "classics").is_err());
    let mut another = info(Format::Epub);
    another.id = "1".repeat(64);
    let selection = [key.clone(), another.id.clone()].into_iter().collect();
    assert!(library.move_books(&selection, Some(folder)).is_err());
    assert!(library.book_folders.is_empty());
    library.books.push(another);
    assert!(library.move_books(&selection, Some(folder + 1)).is_err());
    library.move_books(&selection, Some(folder)).unwrap();
    library.name_folder(Some(folder), "古典小说").unwrap();
    let snapshot = serde_json::to_string(&library).unwrap();
    let mut reopened: Library = serde_json::from_str(&snapshot).unwrap();
    assert_eq!(reopened.folder(folder).unwrap().name, "古典小说");
    assert_eq!(reopened.book_folders.get(&key), Some(&folder));
    assert_eq!(reopened.book_folders.len(), 2);

    reopened.remove_folder(folder);
    assert!(!reopened.book_folders.contains_key(&key));
    assert_eq!(reopened.books.len(), 2);
    assert_eq!(reopened.books[0].id, key);
    assert_eq!(reopened.reading[&key].position.item, 3);
    assert!(reopened.name_folder(None, "诗词").unwrap() > folder);
}

#[test]
fn footnote_labels_can_be_raised_without_shrinking_annotated_words() {
    use crate::reading_view::is_note_marker_label;

    for label in ["[674]", "〔注〕", "［１２］", "(a)", "†"] {
        assert!(is_note_marker_label(label), "{label}");
    }
    for text in ["[]", "老剥皮", "〔校勘：另见旧刻〕", "See note 12"] {
        assert!(!is_note_marker_label(text), "{text}");
    }
}

#[test]
fn epub_gaiji_join_the_parent_paragraph_and_migrate_saved_locations() {
    use crate::{
        book::{BlockKind, Book, Section},
        epub::parse_page,
        library::{Bookmark, Reading},
    };
    let parsed = parse_page(
        "Text/chapter.xhtml",
        r##"<body>
          <blockquote>扑<img alt="造字" src="../Images/glyph.gif" style="height:1em;"/>蜡<a href="#footnote" epub:type="noteref">[19]</a>。</blockquote>
          <p>A <img class="gaiji" alt="𫠠" src="../Images/second.gif"/> B<br/><img alt="外字" src="../Images/third.gif"/>C</p>
          <p><img alt="地图" src="../Images/map.png" style="height:25em;"/></p>
        </body>"##,
    );
    assert_eq!(parsed.blocks.len(), 3);
    let block = &parsed.blocks[0];
    assert_eq!(block.kind, BlockKind::Quote);
    assert_eq!(block.text.as_ref(), "扑〓蜡[19]。");
    let glyph = &block.spans[0];
    assert_eq!(glyph.range, 3..6);
    assert_eq!(glyph.style.glyph.as_deref(), Some("Images/glyph.gif"));
    assert!(block.spans[1].style.note_ref);
    assert_eq!(parsed.blocks[1].text.as_ref(), "A 𫠠 B\n〓C");
    assert!(matches!(parsed.blocks[2].kind, BlockKind::Image { .. }));

    let book = Book::new(
        info(Format::Epub),
        vec![Section {
            blocks: parsed.blocks,
            legacy_rows: parsed.legacy_rows,
            ..Default::default()
        }],
    );
    let mut reading = Reading::default();
    reading.position.item = 3;
    reading.bookmarks.push(Bookmark {
        section: 0,
        item: 2,
        byte: 0,
    });
    reading.annotations.push(Annotation::new(
        1,
        Passage {
            section: 0,
            start: PointInText { block: 2, byte: 0 },
            end: PointInText { block: 2, byte: 3 },
        },
        "蜡".to_owned(),
        "旧批注".to_owned(),
        Tone::Amber,
    ));
    reading.migrate_glyph_anchors(&book);
    assert_eq!((reading.position.item, reading.position.byte), (1, 6));
    assert_eq!(
        reading.bookmarks[0],
        Bookmark {
            section: 0,
            item: 1,
            byte: 3
        }
    );
    assert_eq!(book.quote(&reading.annotations[0].passage), "蜡");
    let position = reading.position;
    reading.migrate_glyph_anchors(&book);
    assert_eq!(reading.position, position);
}

#[test]
fn epub_groups_paragraphs_inside_each_blockquote() {
    use crate::{book::BlockKind, epub::parse_page};
    let parsed = parse_page(
        "chapter.xhtml",
        r#"<p>诗曰：</p><blockquote><p>第一句。</p><p>第二句。</p><p>第三句。</p><p>第四句。</p></blockquote>
           <blockquote><p>另一段引用。</p></blockquote><p>接回正文。</p>"#,
    );
    let poetry = &parsed.blocks[1..5];
    let group = poetry[0].quote_group;
    assert!(group.is_some());
    assert!(
        poetry
            .iter()
            .all(|block| block.kind == BlockKind::Quote && block.quote_group == group)
    );
    assert_eq!(poetry[3].text.as_ref(), "第四句。");
    assert_ne!(parsed.blocks[5].quote_group, group);
    assert!(parsed.blocks[5].quote_group.is_some());
    assert_eq!(parsed.blocks[6].quote_group, None);
}

#[test]
fn txt_reads_chinese_encodings_and_splits_large_blocks() {
    let source = format!(
        "第一章 初见\r\n春风拂面。\r\n\r\n第二章 相逢\r\n{}",
        "仍是那条河。".repeat(1300)
    );
    let (encoded, _, errors) = encoding_rs::GB18030.encode(&source);
    assert!(!errors);
    let book = parse_txt(info(Format::Txt), &encoded).unwrap();
    assert_eq!(book.sections.len(), 2);
    assert_eq!(book.sections[0].blocks[0].text.as_ref(), "春风拂面。");
    assert_eq!(book.sections[1].title.as_ref(), "第二章 相逢");
    let joined: String = book.sections[1]
        .blocks
        .iter()
        .map(|block| block.text.as_ref())
        .collect();
    assert_eq!(joined, "仍是那条河。".repeat(1300));

    let mut utf16 = vec![0xff, 0xfe];
    utf16.extend("夜航\n次日".encode_utf16().flat_map(u16::to_le_bytes));
    let book = parse_txt(info(Format::Txt), &utf16).unwrap();
    assert_eq!(book.sections[0].blocks[1].text.as_ref(), "次日");
}

#[test]
fn epub3_resolves_native_cross_file_notes_and_nested_contents() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("测试.epub");
    epub_fixture(&path, true);
    let book = parse_epub(info(Format::Epub), &path).unwrap();
    assert_eq!(book.info.title, "江上行");
    assert_eq!(book.info.author, "陈舟");
    assert_eq!(book.linear.len(), 1);
    assert_eq!(book.contents.len(), 2);
    assert_eq!(book.contents[1].depth, 1);
    assert_ne!(book.contents[0].block, book.contents[1].block);

    let expected = Target {
        path: "OPS/Notes/尾注.xhtml".to_owned(),
        fragment: "endnote-7".to_owned(),
    };
    assert_eq!(book.sections[0].notes, vec![expected.clone()]);
    let note = &book.notes[&expected];
    assert_eq!(note.title.as_ref(), "尾注 7");
    assert_eq!(note.text.as_ref(), "第一行。\n\n第二行。");
    assert!(
        book.sections[0]
            .blocks
            .iter()
            .any(|block| block.spans.iter().any(|span| span.style.bold))
    );
}

#[test]
fn epub2_resolves_legacy_footnote_and_ncx() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("legacy.epub");
    epub_fixture(&path, false);
    let book = parse_epub(info(Format::Epub), &path).unwrap();
    assert_eq!(book.contents[0].title.as_ref(), "旧目录");
    assert_eq!(book.sections[0].notes.len(), 1);
    let note = &book.notes[&book.sections[0].notes[0]];
    assert_eq!(note.text.as_ref(), "旧版脚注内容。");
}

#[test]
fn epub_toc_keeps_publication_chapters_and_spine_keeps_unlisted_pages() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("章节.epub");
    let mut writer = ZipWriter::new(fs::File::create(&path).unwrap());
    for (name, text) in [
        (
            "META-INF/container.xml",
            r#"<container><rootfiles><rootfile full-path="book.opf"/></rootfiles></container>"#,
        ),
        (
            "book.opf",
            r#"<package><manifest>
          <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
          <item id="poster" href="poster.xhtml" media-type="application/xhtml+xml"/>
          <item id="chapter1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
          <item id="continued" href="continued.xhtml" media-type="application/xhtml+xml"/>
          <item id="chapter2" href="chapter2.xhtml" media-type="application/xhtml+xml"/>
          </manifest><spine><itemref idref="poster"/><itemref idref="chapter1"/>
          <itemref idref="continued"/><itemref idref="chapter2"/></spine></package>"#,
        ),
        (
            "nav.xhtml",
            r#"<nav epub:type="toc"><ol><li><a href="chapter1.xhtml">第一回</a></li>
          <li><a href="chapter2.xhtml">第二回</a></li></ol></nav>"#,
        ),
        ("poster.xhtml", r#"<img src="poster.jpg" alt="海报"/>"#),
        ("chapter1.xhtml", "<h1>第一回</h1><p>千里江山。</p>"),
        ("continued.xhtml", "<p>江山未尽。</p>"),
        ("chapter2.xhtml", "<h1>第二回</h1><p>再见群山。</p>"),
    ] {
        writer
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(text.as_bytes()).unwrap();
    }
    writer.finish().unwrap();

    let book = parse_epub(info(Format::Epub), &path).unwrap();
    assert_eq!(book.linear, [0, 1, 2, 3]);
    assert!(book.sections[0].title.is_empty());
    assert_eq!(
        book.contents
            .iter()
            .map(|e| e.title.as_ref())
            .collect::<Vec<_>>(),
        ["第一回", "第二回"]
    );
    assert_eq!(book.contents_index(0, 0), None);
    assert_eq!(book.contents_index(2, 0), Some(0));
    assert_eq!(book.title_at(2, 0).as_ref(), "第一回");
    assert_eq!(book.neighboring_chapter(1, 0, true).unwrap().section, 3);
    assert_eq!(book.neighboring_chapter(0, 0, true).unwrap().section, 1);
    assert_eq!(book.neighboring_chapter(3, 0, false).unwrap().section, 1);
}

#[test]
fn annotation_round_trip_keeps_multiblock_utf8_offsets() {
    let book = parse_txt(info(Format::Txt), "松风过岭。\n江水向东。".as_bytes()).unwrap();
    let start = PointInText {
        block: 0,
        byte: "松风".len(),
    };
    let end = PointInText {
        block: 1,
        byte: "江水".len(),
    };
    let passage = Passage::new(0, end, start).unwrap();
    assert_eq!(book.quote(&passage), "过岭。\n江水");
    let mut library = Library::default();
    let reading = library.reading.entry(book.info.id.clone()).or_default();
    reading.position = Position {
        item: 2,
        offset: 10.,
        progress: 0.4,
        ..Default::default()
    };
    reading.annotations.push(Annotation::new(
        1,
        passage.clone(),
        book.quote(&passage),
        "复读".to_owned(),
        Tone::Sage,
    ));
    let output = serde_json::to_vec(&library).unwrap();
    let reopened: Library = serde_json::from_slice(&output).unwrap();
    let reading = &reopened.reading[&book.info.id];
    assert_eq!(reading.annotations[0].body, "复读");
    assert_eq!(reading.annotations[0].passage, passage);
    assert_eq!(reading.annotations[0].tone, Tone::Sage);
    assert_eq!(reading.position.item, 2);
}

fn epub_fixture(path: &Path, modern: bool) {
    let mut writer = ZipWriter::new(fs::File::create(path).unwrap());
    let mut add = |name: &str, body: &str| {
        writer
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    };
    add("mimetype", "application/epub+zip");
    add(
        "META-INF/container.xml",
        r#"<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OPS/book.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#,
    );
    let nav_item = if modern {
        r#"<item id="nav" href="toc.xhtml" media-type="application/xhtml+xml" properties="nav"/>"#
    } else {
        r#"<item id="nav" href="toc.ncx" media-type="application/x-dtbncx+xml"/>"#
    };
    add(
        "OPS/book.opf",
        &format!(
            r#"<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
      <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>江上行</dc:title><dc:creator>陈舟</dc:creator></metadata>
      <manifest>{nav_item}<item id="main" href="Text/ch.xhtml" media-type="application/xhtml+xml"/>
      <item id="annotations" href="Notes/%E5%B0%BE%E6%B3%A8.xhtml" media-type="application/xhtml+xml"/></manifest>
      <spine toc="nav"><itemref idref="main"/></spine></package>"#
        ),
    );
    if modern {
        add(
            "OPS/toc.xhtml",
            r##"<nav epub:type="toc"><ol><li><a href="Text/ch.xhtml#top">第一章</a>
          <ol><li><a href="Text/ch.xhtml#part2">第二节</a></li></ol></li></ol></nav>"##,
        );
        add(
            "OPS/Text/ch.xhtml",
            r##"<h1 id="top">第一章</h1><p>江上有<strong>归舟</strong><a role="doc-noteref" href="../Notes/%E5%B0%BE%E6%B3%A8.xhtml#endnote-7">[7]</a>。</p>
          <h2 id="part2">第二节</h2><p>天色已晚。</p>"##,
        );
        add(
            "OPS/Notes/尾注.xhtml",
            r##"<section epub:type="endnotes"><aside id="endnote-7" epub:type="endnote">
          <p>第一行。</p><p>第二行。 <a href="../Text/ch.xhtml#top" role="doc-backlink">返回</a></p></aside></section>"##,
        );
    } else {
        add(
            "OPS/toc.ncx",
            r#"<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/"><navMap><navPoint id="n">
          <navLabel><text>旧目录</text></navLabel><content src="Text/ch.xhtml"/></navPoint></navMap></ncx>"#,
        );
        add(
            "OPS/Text/ch.xhtml",
            r##"<p>旧版正文。<sup><a href="#annotationQ">[1]</a></sup></p>
          <p><a name="annotationQ"></a>旧版脚注内容。 <a href="#fnref1">↩</a></p>"##,
        );
    }
    writer.finish().unwrap();
}

#[test]
fn epub_keeps_image_note_markers_and_editorial_styles_in_flow() {
    use crate::book::{BlockKind, Link, NoteKind};
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("many-notes.epub");
    let mut writer = ZipWriter::new(fs::File::create(&path).unwrap());
    let mut add = |name: &str, body: &str| {
        writer
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    };
    add("mimetype", "application/epub+zip");
    add(
        "META-INF/container.xml",
        r#"<container><rootfiles><rootfile full-path="OPS/book.opf"/></rootfiles></container>"#,
    );
    add(
        "OPS/book.opf",
        r#"<package><metadata><title>观潮录</title></metadata><manifest>
        <item id="chapter" href="chapter.xhtml" media-type="application/xhtml+xml"/>
        <item id="css" href="styles/comment.css" media-type="text/css"/>
        </manifest><spine><itemref idref="chapter"/></spine></package>"#,
    );
    add(
        "OPS/styles/comment.css",
        "span.jiapi { color: #914; } .variant { color: rgb(112, 80, 44); }",
    );
    add(
        "OPS/chapter.xhtml",
        r##"
        <html><head><link rel="stylesheet" href="styles/comment.css"/></head>
        <body><p>潮至石阶<a href="#section-x"><img src="ask.png" alt="注"/></a>。<span class="jiapi">执笔者评曰：以声带景。</span></p>
        <p>木船系柳。<span class="variant">旧抄作「桥」。</span><abbr title="古时小木船">舢板</abbr>。<ruby>橹<rt>lǔ</rt></ruby>。</p>
        <aside id="section-x" hidden><p>涨潮时海水越过一层石阶。</p><p>见本章第二段。</p></aside>
        </body></html>
    "##,
    );
    writer.finish().unwrap();

    let book = parse_epub(info(Format::Epub), &path).unwrap();
    let blocks = &book.sections[0].blocks;
    assert_eq!(
        blocks[0].text.as_ref(),
        "潮至石阶〔注〕。执笔者评曰：以声带景。"
    );
    assert!(
        !blocks
            .iter()
            .any(|block| matches!(block.kind, BlockKind::Image { .. }))
    );
    assert!(blocks[0].spans.iter().any(|span| {
        span.style.note_kind == Some(NoteKind::Commentary) && span.style.color == Some(0x991144)
    }));
    assert!(blocks[1].spans.iter().any(|span| {
        span.style.note_kind == Some(NoteKind::Variant) && span.style.color == Some(0x70502c)
    }));
    let target = blocks[0]
        .spans
        .iter()
        .find_map(|span| match &span.style.link {
            Some(Link::Local(target)) => Some(target),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        book.notes[target].text.as_ref(),
        "涨潮时海水越过一层石阶。\n\n见本章第二段。"
    );
    assert!(
        book.notes
            .values()
            .any(|note| note.kind == NoteKind::Gloss && note.text.as_ref() == "古时小木船")
    );
    assert!(book.notes.values().any(|note| note.text.as_ref() == "lǔ"));
    assert!(!blocks[1].text.contains("lǔ"));
}

#[test]
fn legacy_bookmarks_default_empty_and_round_trip() {
    use crate::library::{Bookmark, ReadingMode};
    let mut state: Library = serde_json::from_str(r#"{"reading":{"book":{"position":{"section":3,"item":4,"offset":0,"progress":0.2},"annotations":[]}}}"#).unwrap();
    assert!(state.reading["book"].bookmarks.is_empty());
    assert_eq!(state.reading["book"].position.byte, 0);
    assert_eq!(state.settings.reading_mode, ReadingMode::Scroll);
    let single_page: Library =
        serde_json::from_str(r#"{"settings":{"reading_mode":"paged"}}"#).unwrap();
    assert_eq!(single_page.settings.reading_mode, ReadingMode::Paged);
    state.settings.reading_mode = ReadingMode::Spread;
    let legacy_bookmark: Bookmark = serde_json::from_str(r#"{"section":3,"item":4}"#).unwrap();
    assert_eq!(legacy_bookmark.byte, 0);
    let bookmark = Bookmark {
        section: 3,
        item: 4,
        byte: 12,
    };
    state
        .reading
        .get_mut("book")
        .unwrap()
        .bookmarks
        .push(bookmark);
    let saved: Library = serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
    assert_eq!(saved.reading["book"].bookmarks, vec![bookmark]);
    assert_eq!(saved.settings.reading_mode, ReadingMode::Spread);
}

#[test]
fn facing_pages_keep_their_pairing_when_jumping_to_either_leaf() {
    use crate::pagination::page_range;

    assert_eq!(page_range(0, 5, 2), 0..2);
    assert_eq!(page_range(1, 5, 2), 0..2);
    assert_eq!(page_range(2, 5, 2), 2..4);
    assert_eq!(page_range(3, 5, 2), 2..4);
    assert_eq!(page_range(4, 5, 2), 4..5);
    assert_eq!(page_range(0, 1, 2), 0..1);
    assert_eq!(page_range(1, 5, 1), 1..2);
}

#[test]
fn pagination_splits_a_paragraph_only_at_line_boundaries() {
    use crate::pagination::{Anchor, LineStart, Pagination, Row};
    use gpui_kit::{px, size};

    let layout = Pagination::new(
        size(px(400.), px(100.)),
        19,
        vec![
            Row {
                height: px(45.),
                lines: Vec::new(),
            },
            Row {
                height: px(105.),
                lines: [(3, 30.), (6, 60.), (9, 90.)]
                    .map(|(byte, offset)| LineStart {
                        byte,
                        offset: px(offset),
                    })
                    .into(),
            },
            Row {
                height: px(0.),
                lines: Vec::new(),
            },
            Row {
                height: px(100.),
                lines: Vec::new(),
            },
        ],
    );
    assert_eq!(layout.pages.len(), 3);
    let first = &layout.pages[0][1];
    let rest = &layout.pages[1][0];
    assert_eq!((first.offset, first.height), (px(0.), px(30.)));
    assert_eq!(first.end_byte, Some(rest.start.byte));
    assert_eq!((rest.offset, rest.height), (px(30.), px(75.)));
    assert_eq!(layout.page_for(Anchor { item: 1, byte: 0 }), 0);
    assert_eq!(layout.page_for(Anchor { item: 1, byte: 6 }), 1);
    assert_eq!(layout.pages[2][0].start, Anchor { item: 3, byte: 0 });
}

#[test]
fn old_annotation_is_aligned_using_its_original_excerpt() {
    let book = parse_txt(
        info(Format::Txt),
        "一段新注。\n风停在岸边。\n夜灯依旧。".as_bytes(),
    )
    .unwrap();
    let old = Passage::new(
        0,
        PointInText { block: 0, byte: 0 },
        PointInText {
            block: 1,
            byte: "夜灯".len(),
        },
    )
    .unwrap();
    let restored = book.restore_passage(&old, "风停在岸边。\n夜灯").unwrap();
    assert_eq!(restored.start.block, 1);
    assert_eq!(restored.end.block, 2);
    assert!(book.restore_passage(&old, "另一处原文").is_none());
    let editorial = "正文字句。〖李本旁批：以声带景。〗【校记：旧抄作桥。】";
    let notes = crate::reading_view::bracketed_notes(editorial);
    assert_eq!(notes[0].1, crate::book::NoteKind::Commentary);
    assert_eq!(notes[1].1, crate::book::NoteKind::Variant);
}
