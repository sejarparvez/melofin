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

use gtk::gdk_pixbuf::{InterpType, Pixbuf};
use gtk::gio::{Cancellable, MemoryInputStream};
use gtk::glib;
use std::thread;

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
