use std::{io::Cursor, path::Path, sync::Arc};

use anyhow::Result;
use gpui_kit::RenderImage;
use image::{Frame, ImageReader, Limits};
use smallvec::smallvec;

use crate::epub::{MAX_IMAGE, open_zip, read_entry};

pub struct Picture {
    pub art: Arc<RenderImage>,
    pub width: u32,
    pub height: u32,
}

pub fn read_picture(archive: &Path, name: &str) -> Result<Picture> {
    let bytes = read_entry(&mut open_zip(archive)?, name, MAX_IMAGE)?;
    decode_picture(&bytes, 1380)
}

pub fn read_shelf_scene() -> Result<Picture> {
    decode_picture(include_bytes!("../assets/misty-coast.jpg"), 1680)
}

fn decode_picture(bytes: &[u8], max_side: u32) -> Result<Picture> {
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(96 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader.decode()?;
    let mut image = if decoded.width().max(decoded.height()) > max_side {
        decoded.thumbnail(max_side, max_side).into_rgba8()
    } else {
        decoded.into_rgba8()
    };
    for pixel in image.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    let (width, height) = image.dimensions();
    let art = Arc::new(RenderImage::new(smallvec![Frame::new(image)]));
    Ok(Picture { art, width, height })
}
