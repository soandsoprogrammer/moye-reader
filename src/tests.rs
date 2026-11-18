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
