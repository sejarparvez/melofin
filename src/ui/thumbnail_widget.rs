//! Shared fetch → decode → scale/crop → texture pipeline, used by both
//! `search_view.rs` (small row thumbnails) and `player_bar.rs` (the bigger
//! album art tile) instead of each duplicating it.
//!
//! Why this exists, beyond deduplication: `gtk::Picture` derives its
//! *preferred* size from the source image's own pixel dimensions —
//! `set_size_request` only sets a *minimum*, it doesn't cap how big the
//! widget wants to be. Handing a `Picture` a full 320x180 (or larger)
//! source image and expecting `content_fit: Cover` to keep it visually
//! small was the bug: GTK still budgets layout space closer to the
//! source's native size, which is exactly what was ballooning the player
//! bar. Scaling and center-cropping down to the *exact* target size here,
//! before a `Texture` ever gets created, means the texture's own pixel
//! dimensions already equal the widget's intended size — there's no
//! size-negotiation left for GTK to get wrong.

use adw::prelude::*;
use gtk::gdk_pixbuf::{InterpType, Pixbuf};
use gtk::gio::{Cancellable, MemoryInputStream};
use gtk::glib;
use std::cell::RefCell;
use std::thread;

use crate::search::Track;

/// Fetches `url` on a background thread (disk-cached — see
/// `crate::thumbnail::fetch`), then decodes and scales+center-crops it to
/// an exact `size`x`size` square on the main thread, and calls `on_ready`
/// with the finished `Texture`. Silently does nothing on any failure (bad
/// URL, corrupt image, decode error) — a missing thumbnail isn't worth
/// surfacing to the user over a whole row or the player bar.
///
/// Decoding stays on the main thread rather than the background one: the
/// `gdk_pixbuf`/`gdk` types involved aren't `Send`, so only the raw
/// network bytes (`Vec<u8>`, which is `Send`) cross the thread boundary.
pub fn spawn_fetch(url: String, size: i32, on_ready: impl FnOnce(gtk::gdk::Texture) + 'static) {
    let (sender, receiver) = async_channel::bounded::<anyhow::Result<Vec<u8>>>(1);
    thread::spawn(move || {
        let _ = sender.send_blocking(crate::thumbnail::fetch(&url));
    });
    glib::spawn_future_local(async move {
        let Ok(Ok(bytes)) = receiver.recv().await else {
            return;
        };
        match decode_and_crop(&bytes, size) {
            Some(pixbuf) => on_ready(gtk::gdk::Texture::for_pixbuf(&pixbuf)),
            None => tracing::warn!("failed to decode thumbnail ({} bytes)", bytes.len()),
        }
    });
}

/// Decodes raw image bytes and scales+center-crops the result to an exact
/// `size`x`size` square — matching what CSS `object-fit: cover` would do:
/// preserve aspect ratio, scale up until the image fully covers the
/// square, then crop the centered overflow rather than distorting the
/// image to fit.
fn decode_and_crop(bytes: &[u8], size: i32) -> Option<Pixbuf> {
    let stream = MemoryInputStream::from_bytes(&glib::Bytes::from(bytes));
    let pixbuf = Pixbuf::from_stream(&stream, Cancellable::NONE).ok()?;

    let (src_w, src_h) = (pixbuf.width(), pixbuf.height());
    if src_w <= 0 || src_h <= 0 {
        return None;
    }

    let scale = (f64::from(size) / f64::from(src_w)).max(f64::from(size) / f64::from(src_h));
    let scaled_w = ((f64::from(src_w) * scale).round() as i32).max(1);
    let scaled_h = ((f64::from(src_h) * scale).round() as i32).max(1);
    let scaled = pixbuf.scale_simple(scaled_w, scaled_h, InterpType::Bilinear)?;

    let crop_w = size.min(scaled_w);
    let crop_h = size.min(scaled_h);
    let x = (scaled_w - crop_w) / 2;
    let y = (scaled_h - crop_h) / 2;
    Some(scaled.new_subpixbuf(x, y, crop_w, crop_h))
}

/// A `gtk::Stack` that shows either a placeholder icon or a fetched
/// thumbnail, with built-in deduplication — `update()` only fetches when
/// the URL actually changes. Used by `PlayerBar` and `NowPlayingPanel`.
pub struct ThumbnailStack {
    stack: gtk::Stack,
    picture: gtk::Picture,
    current_url: RefCell<String>,
}

impl ThumbnailStack {
    pub fn new(placeholder_icon: &str, pixel_size: i32, size: i32) -> Self {
        let stack = gtk::Stack::new();
        let icon = gtk::Image::from_icon_name(placeholder_icon);
        icon.set_pixel_size(pixel_size);
        icon.set_halign(gtk::Align::Center);
        icon.set_valign(gtk::Align::Center);
        stack.add_named(&icon, Some("placeholder"));

        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_size_request(size, size);
        stack.add_named(&picture, Some("art"));
        stack.set_visible_child_name("placeholder");

        Self {
            stack,
            picture,
            current_url: RefCell::new(String::new()),
        }
    }

    pub fn widget(&self) -> &gtk::Stack {
        &self.stack
    }

    /// Updates the thumbnail. Only fetches if `url` differs from the
    /// currently displayed one. Pass `size` as the fetch/crop target
    /// (typically the same value passed to `new`).
    pub fn update(&self, url: &str, size: i32) {
        if *self.current_url.borrow() == url {
            return;
        }
        *self.current_url.borrow_mut() = url.to_string();
        if url.is_empty() {
            self.stack.set_visible_child_name("placeholder");
        } else {
            let stack = self.stack.clone();
            let picture = self.picture.clone();
            spawn_fetch(url.to_string(), size, move |texture| {
                picture.set_paintable(Some(&texture));
                stack.set_visible_child_name("art");
            });
        }
    }
}

/// Builds an `adw::ActionRow` for a track with a thumbnail prefix,
/// title, subtitle, and activatable state. Used by both `search_view`
/// and `liked_songs_view`.
pub fn build_track_row(track: &Track) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_title(&glib::markup_escape_text(&track.title));
    row.set_subtitle(&glib::markup_escape_text(&track.artist));
    row.set_activatable(true);

    let thumbnail = gtk::Picture::new();
    thumbnail.set_size_request(40, 40);
    thumbnail.set_content_fit(gtk::ContentFit::Cover);
    row.add_prefix(&thumbnail);
    if !track.thumbnail_url.is_empty() {
        let url = track.thumbnail_url.clone();
        spawn_fetch(url, 40, move |texture| {
            thumbnail.set_paintable(Some(&texture));
        });
    }

    row
}
