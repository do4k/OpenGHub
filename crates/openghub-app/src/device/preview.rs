use std::sync::Arc;
use std::time::Duration;

use openghub_core::v4l2::PIX_FMT_YUYV;

use rune::Tx;

use super::Device;
use crate::state::State;
#[cfg(unix)]
pub(crate) fn stop_camera_previews(s: &State) {
    for card in &s.cards {
        if let Device::Camera(_, ui) = &card.device {
            ui.preview.stop();
        }
    }
}

#[cfg(unix)]
pub(crate) fn bgra_to_rgba(bgra: &[u8]) -> Vec<u8> {
    let mut rgba = bgra.to_vec();
    for px in rgba.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
    rgba
}

/// Drives the live webcam preview: starts the selected camera's stream and
/// publishes each new frame as an [`Image`] for the view to draw.
#[cfg(unix)]
pub(crate) fn poll(s: &mut State, _clock: f64, tx: &mut Tx) -> bool {
    let Some(i) = s.selected else {
        stop_camera_previews(s);
        s.preview_image = None;
        s.preview_pending = None;
        return false;
    };

    let selected = match &s.cards[i].device {
        Device::Camera(_, ui) => Some(Arc::clone(&ui.preview)),
        _ => None,
    };
    for (j, card) in s.cards.iter().enumerate() {
        if j != i
            && let Device::Camera(_, ui) = &card.device
        {
            ui.preview.stop();
        }
    }

    let Some(preview) = selected else {
        s.preview_image = None;
        s.preview_pending = None;
        return false;
    };

    // Keep polling at ~15 fps while the camera page is open.
    tx.wake_after(Duration::from_millis(66));
    let _ = preview.start(PIX_FMT_YUYV);

    // Promote a freshly-decoded frame only once it's ready, so the view never
    // draws a pending (blank) image — the source of the flicker.
    let promoted = s.preview_pending.as_ref().is_some_and(|im| im.is_ready());
    if promoted {
        s.preview_image = s.preview_pending.take();
    }

    // Read the latest frame into the pending slot.
    if let Some(frame) = preview.try_get_frame()
        && frame.sequence != s.preview_seq
    {
        s.preview_seq = frame.sequence;
        if let Some(images) = &s.images {
            let image = if frame.is_jpeg {
                images.load(frame.pixels)
            } else {
                let rgba = bgra_to_rgba(&frame.pixels);
                images.rgba(frame.width as u32, frame.height as u32, rgba)
            };
            s.preview_pending = Some(image);
        }
    }

    promoted
}
