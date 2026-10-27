//! Fences selected together by drawing a rectangle on the empty desktop: every fence the
//! marquee touches is selected as it is drawn (Explorer's rubber band), Ctrl or Shift adds to
//! the selection, and a click on the desktop or on any fence ends it. Selected fences wear a
//! white ring, like the other drag feedback; a title drag of one of them moves them all
//! (`fence_window::group`). A rectangle drawn with the right button instead offers a new fence
//! there, as in Fences, unless 「在桌面空白处按住右键拖出矩形时提供“在此新建栅栏”」 is off.

use super::*;
use crate::anchor::{MARQUEE_MIN_PX, MarqueeEvent};
use pecofence_platform::rdrag;

impl App {
    /// Watches right-button drags on the desktop (a rectangle that selects, as on Windows' own
    /// desktop, and offers a new fence on empty space while that offer is on).
    pub(super) fn sync_right_drag_watch(&mut self) {
        if self.right_drag.is_none() {
            self.right_drag = rdrag::watch(self.control.hwnd(), WM_APP_RIGHT_DRAG);
        }
    }

    /// A right-button drag on the bare desktop (`rdrag`), as on Windows' desktop: the band
    /// selects the free icons and fences it touches, like the left one; let go, the selected
    /// icons get their menu, and a large enough rectangle that selected nothing offers a new
    /// fence there (Fences).
    pub(super) fn on_desktop_right_drag(&mut self, kind: usize, pt: POINT) {
        let rect = |a: POINT, b: POINT| RECT {
            left: a.x.min(b.x),
            top: a.y.min(b.y),
            right: a.x.max(b.x),
            bottom: a.y.max(b.y),
        };
        match kind {
            rdrag::RDRAG_PRESSED => {
                self.right_drag_origin = Some(pt);
                let additive = window::key_down_async(msg::VK_CONTROL)
                    || window::key_down_async(msg::VK_SHIFT);
                self.on_desktop_marquee(MarqueeEvent::Pressed { additive });
            }
            rdrag::RDRAG_MOVED => {
                // Moves are coalesced: only the newest position matters.
                let pt = rdrag::latest_move();
                if let Some(origin) = self.right_drag_origin {
                    self.on_desktop_marquee(MarqueeEvent::Moved(rect(origin, pt)));
                }
            }
            rdrag::RDRAG_RELEASED => {
                self.hide_marquee_band();
                let Some(origin) = self.right_drag_origin.take() else {
                    return;
                };
                let r = rect(origin, pt);
                let base = std::mem::take(&mut self.desk_marquee_base);
                self.desk.select_in_rect(r, &base);
                let fences = self.marquee_selection(r);
                self.marquee_base = None;
                let fences_touched = !self.fences_touching(r).is_empty();
                self.set_fence_selection(fences);
                let icons = self.desk.selection();
                tracing::info!(?r, icons = icons.len(), "desktop right-button rectangle");
                if !icons.is_empty() {
                    self.desk.take_keyboard_for_selection();
                    if let Some(inbox) = self.state.inbox_id() {
                        self.show_item_menu(inbox, icons, pt.x, pt.y);
                    }
                } else if !fences_touched
                    && r.right - r.left >= MARQUEE_MIN_PX
                    && r.bottom - r.top >= MARQUEE_MIN_PX
                    && self.state.config.settings.offer_new_fence
                {
                    self.offer_new_fence(r);
                }
            }
            _ => {
                if self.right_drag_origin.take().is_some() {
                    self.on_desktop_marquee(MarqueeEvent::Cancelled);
                }
            }
        }
    }

    pub(super) fn on_desktop_marquee(&mut self, event: MarqueeEvent) {
        match event {
            MarqueeEvent::Pressed { additive } => {
                let kept = if additive {
                    self.ctx.behavior.selection.borrow().clone()
                } else {
                    Vec::new()
                };
                self.set_fence_selection(kept.clone());
                self.marquee_base = Some(kept);
                // Free icons: the same rubber band selects them, as on the Windows desktop.
                self.desk_marquee_base = if additive {
                    self.desk.selection()
                } else {
                    self.desk.clear_selection();
                    Vec::new()
                };
                self.hide_marquee_band();
                self.marquee_draws_band = pecofence_platform::shell_icons::desktop_icons_hidden();
            }
            MarqueeEvent::Moved(rect) => {
                let selection = self.marquee_selection(rect);
                self.set_fence_selection(selection);
                let base = self.desk_marquee_base.clone();
                self.desk.select_in_rect(rect, &base);
                if self.marquee_draws_band {
                    self.show_marquee_band(rect);
                }
            }
            MarqueeEvent::Released(rect) => {
                self.hide_marquee_band();
                let base = std::mem::take(&mut self.desk_marquee_base);
                let icons_touched = self.desk.select_in_rect(rect, &base) > base.len();
                let touched = !self.fences_touching(rect).is_empty() || icons_touched;
                let selection = self.marquee_selection(rect);
                self.marquee_base = None;
                let count = selection.len();
                self.set_fence_selection(selection);
                // Only selects: a new fence is drawn with the right button (Fences).
                if touched {
                    tracing::info!(count, "fences selected on the desktop");
                }
                if icons_touched {
                    self.desk.take_keyboard_for_selection();
                }
            }
            MarqueeEvent::Cancelled => {
                self.hide_marquee_band();
                self.marquee_base = None;
            }
        }
    }

    fn show_marquee_band(&mut self, rect: RECT) {
        if self.marquee_band.is_none() {
            match crate::marquee_band::MarqueeBand::create() {
                Ok(band) => self.marquee_band = Some(band),
                Err(e) => {
                    tracing::warn!(error = %e, "marquee band unavailable");
                    self.marquee_draws_band = false;
                    return;
                }
            }
        }
        let host = self
            .anchor
            .borrow()
            .as_ref()
            .and_then(|a| a.host())
            .map(|h| h.host);
        let dpi = monitors::monitor_from_point(rect.left, rect.top);
        let dpi = monitors::query(dpi).map_or(monitors::system_dpi(), |m| m.dpi);
        if let Some(band) = self.marquee_band.as_mut() {
            band.show(rect, dpi, host);
        }
    }

    fn hide_marquee_band(&mut self) {
        if let Some(band) = self.marquee_band.as_mut() {
            band.hide();
        }
    }

    /// The marquee band's rect while it shows (test dumps).
    pub(super) fn marquee_band_shown(&self) -> Option<RECT> {
        self.marquee_band.as_ref().and_then(|b| b.shown())
    }

    /// The selection a marquee over `rect` stands for: what a Ctrl / Shift marquee kept, plus
    /// every fence it touches.
    fn marquee_selection(&self, rect: RECT) -> Vec<(HWND, FenceId)> {
        let mut selection = self.marquee_base.clone().unwrap_or_default();
        for hit in self.fences_touching(rect) {
            if !selection.contains(&hit) {
                selection.push(hit);
            }
        }
        selection
    }

    /// Showing, unlocked fence windows that `rect` (screen px) overlaps.
    fn fences_touching(&self, rect: RECT) -> Vec<(HWND, FenceId)> {
        let mut hits: Vec<(HWND, FenceId)> = self
            .fences
            .iter()
            .filter(|(id, w)| {
                let r = w.rect();
                desktop::is_visible(w.hwnd())
                    && !self.state.fence(**id).is_some_and(|f| f.locked)
                    && r.left < rect.right
                    && rect.left < r.right
                    && r.top < rect.bottom
                    && rect.top < r.bottom
            })
            .map(|(id, w)| (w.hwnd(), *id))
            .collect();
        // Stable order (the map's is not): top to bottom, then left to right.
        hits.sort_by_key(|(h, _)| {
            let r = window::window_rect(*h);
            (r.top, r.left)
        });
        hits
    }

    /// Replaces the selection, updating the rings of the fences that joined or left it.
    pub(super) fn set_fence_selection(&mut self, selection: Vec<(HWND, FenceId)>) {
        let old = self.ctx.behavior.selection.replace(selection.clone());
        if old == selection {
            return;
        }
        for (hwnd, id) in &old {
            if !selection.contains(&(*hwnd, *id))
                && self.settings_ring != Some(*id)
                && let Some(w) = self.fences.get(id)
                && w.hwnd() == *hwnd
            {
                w.set_group_selected(false);
            }
        }
        for (hwnd, id) in &selection {
            if !old.contains(&(*hwnd, *id))
                && let Some(w) = self.fences.get(id)
                && w.hwnd() == *hwnd
            {
                w.set_group_selected(true);
            }
        }
    }

    /// Settings › 「栅栏」 rings the fence it edits (`host`, None off the page or when Settings
    /// closes) without joining the desktop selection that group drags move.
    pub(super) fn set_settings_ring(&mut self, host: Option<FenceId>) {
        let selected = self.ctx.behavior.selection.borrow().clone();
        if let Some(old) = self.settings_ring.take()
            && host != Some(old)
            && !selected.iter().any(|(_, id)| *id == old)
            && let Some(w) = self.fences.get(&old)
        {
            w.set_group_selected(false);
        }
        if let Some(w) = host.and_then(|h| self.fences.get(&h)) {
            w.set_group_selected(true);
            self.settings_ring = host;
        }
    }

    /// A fence window going away leaves the selection.
    pub(super) fn forget_selected_window(&mut self, hwnd: HWND) {
        self.ctx
            .behavior
            .selection
            .borrow_mut()
            .retain(|(h, _)| *h != hwnd);
        if let Some(base) = self.marquee_base.as_mut() {
            base.retain(|(h, _)| *h != hwnd);
        }
    }
}
