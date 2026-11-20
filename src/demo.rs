use crate::{
    book::{Book, BookInfo, ContentsEntry, DEMO_ID, Format, Section},
    epub::parse_page,
};

pub fn info() -> BookInfo {
    BookInfo {
        id: DEMO_ID.to_owned(),
        title: "雾岸来信".to_owned(),
        author: "内置试读".to_owned(),
        format: Format::Epub,
        cover: None,
    }
}

pub fn book() -> Book {
    let mut sections = Vec::with_capacity(CHAPTERS.len());
    let mut notes = std::collections::HashMap::new();
    let mut destinations = std::collections::HashMap::new();
    for (index, &(title, html)) in CHAPTERS.iter().enumerate() {
        let path = format!("demo/{}.xhtml", index + 1);
        let page = parse_page(&path, html);
        let note_locations = page
            .blocks
            .iter()
            .enumerate()
            .flat_map(|(row, block)| {
                block.spans.iter().filter_map(move |span| {
                    if let Some(crate::book::Link::Local(target)) = &span.style.link {
                        Some((target.clone(), row))
                    } else {
                        None
                    }
                })
            })
            .collect();
        sections.push(Section {
            title: title.into(),
            blocks: page.blocks,
            header: None,
            notes: page.note_order,
            note_locations,
            legacy_rows: page.legacy_rows,
        });
        for (fragment, block) in page.anchors {
            destinations.insert(
                crate::book::Target {
                    path: path.clone(),
                    fragment,
                },
                (index, block),
            );
        }
        notes.extend(page.notes);
    }
    let mut book = Book::new(info(), sections);
    book.notes = notes;
    book.destinations = destinations;
    book.contents = CHAPTERS
        .iter()
        .enumerate()
        .map(|(section, &(title, _))| ContentsEntry {
            title: title.into(),
            section,
            block: 0,
            depth: 0,
        })
        .collect();
    book
}

static CHAPTERS: &[(&str, &str)] = &[
    (
        "潮水以前",
        r##"
        <p>我到雾岸的那天，海上正落着很轻的雨。客船还没靠稳，就有一群鸥从船尾拔起，转眼消失在低垂的云里。船夫说，镇上的邮局早在三年前就搬走了。可我口袋里那封信的邮戳，分明是上周四。</p>
        <p>下船后，我沿着石堤往东走。潮水贴着堤脚，一下一下地翻动，像有人在暗处翻一本书。堤外浮着几艘小舢板<a href="#fn1" epub:type="noteref">〔1〕</a>，漆成浅绿，缆绳上结满了去年的盐。</p>
        <p>码头的尽头有一间修伞店。门没关，一盏油灯从屋里照出来。坐在柜台后的老人把眼镜推上额头，看了看我手里的信，说：“你可以先到北边的灯塔去。从前，等信的人都在那里。”</p>
        <p>我问他从前是什么时候。老人想了很久，最后说，大概是海还没有学会把东西还回来的时候。</p>
        <blockquote>有人说，潮水从来不带走什么。它只是替人保管，直到有人愿意回来认领。</blockquote>
        <p>我把信重新叠好，塞回大衣内侧。雨在这时停了一会儿，远处传来一次回潮前的钟声<a href="#fn2" role="doc-noteref">〔2〕</a>。那声音很薄，在水面上走得比任何一艘船都远。</p>
        <p>石堤越往北越窄。路旁长着低矮的海桐，叶背积着一层白盐。我摘了一片，用手指擦干净。远处刚好有人点了灯，于是那一点微弱的光，也在叶子上停了片刻。</p>
        <aside id="fn1" epub:type="footnote"><p>舢板是一种浅吃水的小木船。这里的雾岸居民用它在石堤与停在外海的渔船之间往返。</p></aside>
        <aside id="fn2" role="doc-endnote"><p>回潮前的钟在本文中是镇上的旧规矩。雾重的日子，钟声也用来为归船指路。</p></aside>
    "##,
    ),
    (
        "灯塔守夜人",
        r##"
        <p>灯塔底下的门是蓝色的。一道细细的水迹从门楣流过来，刚好经过锁孔。门上钉着一个铜牌，写着：有风时请敲两下。</p>
        <p>现在没有风。我敲了一下，里面立刻有人回答：“还没煮好。”门打开的时候，一股干草和热水的气味涌出来。守夜人姓沈，穿一件袖口磨破的羊毛衫。</p>
        <p>他借着炉火读完我带来的信，没有问寄信的人叫什么。他从架子上取下一本账簿，指着一处抹掉了字的空白：“你看，这个位子一直给你留着。”</p>
        <blockquote>我靠近火才明白，有些事情等得太久，就会变成房子里的家具。</blockquote>
        <p>楼上的透镜转动起来，鞋底下立刻传来细微的震颤。沈师傅往火里添了块木头，告诉我，亮得最远的东西，未必站得最高。</p>
    "##,
    ),
    (
        "邮戳与干花",
        r##"
        <p>翌晨，我在窗台上找到一枚旧邮戳和一束晒干了的洋甘菊。邮戳的木柄摸起来很温暖；花瓣碰一碰就碎了，碎在掌纹里，像极小的纸片。</p>
        <p>信封背面有一道几乎看不见的折痕。我顺着折痕把纸展开，才发现里侧还有一行字：到了之后，先别急着问我在哪儿。</p>
        <p>那字迹像我自己的。小时候我给自己写过许多清单，列下想去的地方，却从没有写过回来的路。</p>
    "##,
    ),
    (
        "北岸的房间",
        r##"
        <p>房里只有一把椅子和一张小桌。桌面朝海，窗子压得很低，坐下时海平线恰好齐着眼睛。抽屉里有成捆的信，分别用不同颜色的线扎好。</p>
        <p>珊瑚红那一沓的日期在二十年以前，苔绿那一沓只有一个信封。它没有写收件人，邮票却已经贴好。</p>
        <p>我在房间里坐到天色发蓝，终于把那封一直带在口袋里的信放到桌上。两只信封挨在一起，大小刚好一样。</p>
    "##,
    ),
    (
        "寄往没有雾的地方",
        r##"
        <p>离开雾岸的早上晴得出奇。客船从码头外面绕回来，船舷上的白漆在太阳下亮得叫人眯眼。</p>
        <p>我把新写的信交给修伞的老人。他没有看信封，把它放进柜台下的抽屉。抽屉合起来时，我听见里面还有许多纸张轻轻地动了一下。</p>
        <p>老人往堤外指了指。潮正在退，原先覆满水的石阶露了出来。我头一次看见，最下面那一级还刻着一个浅浅的“归”字。</p>
    "##,
    ),
];
