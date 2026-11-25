// SPEC-HAL-303: OrderBookBody — Row painter for the order book ladder.
//
// Lives inside a ScrollArea. Paints all ask rows, spread indicator,
// and bid rows. ScrollArea handles clipping and scroll offset.

use crate::core::event::{Event, EventPhase, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::{Color, LAYER_CONTENT, ZIndex};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::model::order_book::BookSideKind;
use crate::utils::id::WidgetInternalId;
use crate::widgets::order_book::{
    OrderBookEvent, OrderBookState, PriceDirection, find_flash, format_price, format_qty,
    format_total,
};
use std::cell::RefCell;
use std::rc::Rc;

const ORDER_BOOK_TEXT_LAYER: ZIndex = LAYER_CONTENT + 7;

pub struct OrderBookBody {
    id: WidgetInternalId,
    pub(crate) state: Rc<RefCell<OrderBookState>>,
}

impl OrderBookBody {
    pub(crate) fn new(state: Rc<RefCell<OrderBookState>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            state,
        }
    }
}

impl Widget for OrderBookBody {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let state = self.state.borrow();
        let row_h = state.row_h;
        let spread_h = state.spread_h;

        let ask_rows = state.asks.levels.len();
        let bid_rows = state.bids.levels.len();
        let content_h = ask_rows as f32 * row_h + spread_h + bid_rows as f32 * row_h;

        let width = constraints.max_width;
        LayoutResult::new(Size::new(width, content_h), Size::new(width, content_h))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let state = self.state.borrow();
        let row_h = state.row_h;
        let spread_h = state.spread_h;
        let font_size = state.config.font_size;
        let metrics = cx.text_engine.font_metrics(font_size);
        let text_color = cx.theme.colors.typography.primary;

        let pad = 4.0;
        let col_price_left = rect.x + pad;
        let col_amount_right = rect.x + rect.width * 0.62;
        let col_total_right = rect.x + rect.width - pad;

        let ask_count = state.asks.levels.len();
        let bid_count = state.bids.levels.len();

        // Max cumulative for bar scaling
        let max_ask_cum: f64 = state
            .asks
            .levels
            .iter()
            .map(|l| l.quantity)
            .sum::<f64>()
            .max(0.001);
        let max_bid_cum: f64 = state
            .bids
            .levels
            .iter()
            .map(|l| l.quantity)
            .sum::<f64>()
            .max(0.001);
        let max_cum = max_ask_cum.max(max_bid_cum);

        // Pre-compute ask cumulative (index 0 = best ask)
        let mut ask_cum: Vec<f64> = Vec::with_capacity(ask_count);
        {
            let mut c = 0.0;
            for l in &state.asks.levels {
                c += l.quantity;
                ask_cum.push(c);
            }
        }

        let mut y = rect.y;

        // ── Ask rows (reversed: worst at top, best at bottom) ──
        for display_i in 0..ask_count {
            let ask_idx = ask_count - 1 - display_i;
            let level = &state.asks.levels[ask_idx];
            let cum = ask_cum[ask_idx];

            // Zebra stripe
            if display_i % 2 == 1 {
                cx.scene.push_rect(
                    RectF::new(rect.x, y, rect.width, row_h),
                    cx.theme.colors.list.row_background_alt,
                    LAYER_CONTENT,
                    None,
                );
            }

            // Cumulative bar (full width)
            let bar_frac = (cum / max_cum).min(1.0) as f32;
            let bar_w = rect.width * bar_frac;
            let alpha = 0.15 + 0.40 * bar_frac;
            cx.scene.push_rect(
                RectF::new(rect.x + rect.width - bar_w, y, bar_w, row_h),
                state.config.ask_color.with_alpha(alpha),
                LAYER_CONTENT + 1,
                None,
            );

            // Text
            let text_y = y + (row_h - (metrics.ascent + metrics.descent)) / 2.0;
            let new_t = find_flash(&state.new_flashes, level.price, BookSideKind::Ask);
            let qty_t = find_flash(&state.qty_flashes, level.price, BookSideKind::Ask);
            let price_c = if new_t > 0.0 {
                state.config.ask_color.lerp(Color::WHITE, new_t)
            } else {
                state.config.ask_color
            };
            let amount_c = if qty_t > 0.0 {
                text_color.lerp(Color::WHITE, qty_t)
            } else {
                text_color
            };

            let run = cx
                .text_engine
                .shape(&format_price(&state.config, level.price), font_size);
            cx.text_engine.draw(
                &run,
                col_price_left,
                text_y,
                price_c,
                ORDER_BOOK_TEXT_LAYER,
                cx.scene,
            );

            let atxt = format_qty(&state.config, level.quantity);
            let (aw, _) = cx.text_engine.measure(&atxt, font_size);
            let run = cx.text_engine.shape(&atxt, font_size);
            cx.text_engine.draw(
                &run,
                col_amount_right - aw,
                text_y,
                amount_c,
                ORDER_BOOK_TEXT_LAYER,
                cx.scene,
            );

            let ttxt = format_total(&state.config, cum);
            let (tw, _) = cx.text_engine.measure(&ttxt, font_size);
            let run = cx.text_engine.shape(&ttxt, font_size);
            cx.text_engine.draw(
                &run,
                col_total_right - tw,
                text_y,
                text_color,
                ORDER_BOOK_TEXT_LAYER,
                cx.scene,
            );

            // Hover: blue tint + left edge bar (reads through red/green bars)
            if state.hovered_row == Some(display_i) {
                let hover_tint = cx.theme.colors.accents.primary.with_alpha(0.25);
                cx.scene.push_rect(
                    RectF::new(rect.x, y, rect.width, row_h),
                    hover_tint,
                    LAYER_CONTENT + 5,
                    None,
                );
                let edge = cx.theme.colors.accents.primary.with_alpha(0.9);
                cx.scene.push_rect(
                    RectF::new(rect.x, y, 3.0, row_h),
                    edge,
                    LAYER_CONTENT + 6,
                    None,
                );
            }

            y += row_h;
        }

        // ── Spread row: ratio bar + delta ──
        let total_bid_qty: f64 = state.bids.levels.iter().map(|l| l.quantity).sum();
        let total_ask_qty: f64 = state.asks.levels.iter().map(|l| l.quantity).sum();
        let total = total_bid_qty + total_ask_qty;
        let bid_frac = if total > 0.0 {
            (total_bid_qty / total) as f32
        } else {
            0.5
        };

        let bid_w = rect.width * bid_frac;
        cx.scene.push_rect(
            RectF::new(rect.x, y, bid_w, spread_h),
            state.config.bid_color.with_alpha(0.35),
            LAYER_CONTENT + 1,
            None,
        );
        cx.scene.push_rect(
            RectF::new(rect.x + bid_w, y, rect.width - bid_w, spread_h),
            state.config.ask_color.with_alpha(0.35),
            LAYER_CONTENT + 1,
            None,
        );

        let spread_color = match state.last_direction {
            PriceDirection::Up => state.config.bid_color,
            PriceDirection::Down => state.config.ask_color,
            PriceDirection::Unchanged => text_color,
        };
        let best_ask = state.asks.levels.first().map(|l| l.price);
        let best_bid = state.bids.levels.first().map(|l| l.price);
        let spread_text = match (best_ask, best_bid) {
            (Some(a), Some(b)) => format_price(&state.config, a - b),
            _ => "\u{2014}".into(),
        };
        let (stw, _) = cx.text_engine.measure(&spread_text, font_size);
        let run = cx.text_engine.shape(&spread_text, font_size);
        cx.text_engine.draw(
            &run,
            rect.x + (rect.width - stw) / 2.0,
            y + (spread_h - font_size) / 2.0,
            spread_color,
            ORDER_BOOK_TEXT_LAYER,
            cx.scene,
        );

        // Spread percentages at edges
        let dim_text = cx.theme.colors.typography.secondary;
        let pct_pad = 4.0_f32;

        let bid_pct = format!("{:.1}%", bid_frac * 100.0);
        let run = cx.text_engine.shape(&bid_pct, font_size);
        cx.text_engine.draw(
            &run,
            rect.x + pct_pad,
            y + (spread_h - font_size) / 2.0,
            dim_text,
            ORDER_BOOK_TEXT_LAYER,
            cx.scene,
        );

        let ask_pct = format!("{:.1}%", (1.0 - bid_frac) * 100.0);
        let (apw, _) = cx.text_engine.measure(&ask_pct, font_size);
        let run = cx.text_engine.shape(&ask_pct, font_size);
        cx.text_engine.draw(
            &run,
            rect.x + rect.width - apw - pct_pad,
            y + (spread_h - font_size) / 2.0,
            dim_text,
            ORDER_BOOK_TEXT_LAYER,
            cx.scene,
        );

        y += spread_h;

        // ── Bid rows (best at top, worst at bottom) ──
        let mut bid_cum = 0.0;
        for display_i in 0..bid_count {
            let level = &state.bids.levels[display_i];
            bid_cum += level.quantity;

            // Zebra stripe — offset by ask rows + spread so stripes are continuous
            if (ask_count + 1 + display_i) % 2 == 1 {
                cx.scene.push_rect(
                    RectF::new(rect.x, y, rect.width, row_h),
                    cx.theme.colors.list.row_background_alt,
                    LAYER_CONTENT,
                    None,
                );
            }

            // Cumulative bar (full width)
            let bar_frac = (bid_cum / max_cum).min(1.0) as f32;
            let bar_w = rect.width * bar_frac;
            let alpha = 0.15 + 0.40 * bar_frac;
            cx.scene.push_rect(
                RectF::new(rect.x + rect.width - bar_w, y, bar_w, row_h),
                state.config.bid_color.with_alpha(alpha),
                LAYER_CONTENT + 1,
                None,
            );

            // Text
            let text_y = y + (row_h - (metrics.ascent + metrics.descent)) / 2.0;
            let new_t = find_flash(&state.new_flashes, level.price, BookSideKind::Bid);
            let qty_t = find_flash(&state.qty_flashes, level.price, BookSideKind::Bid);
            let price_c = if new_t > 0.0 {
                state.config.bid_color.lerp(Color::WHITE, new_t)
            } else {
                state.config.bid_color
            };
            let amount_c = if qty_t > 0.0 {
                text_color.lerp(Color::WHITE, qty_t)
            } else {
                text_color
            };

            let run = cx
                .text_engine
                .shape(&format_price(&state.config, level.price), font_size);
            cx.text_engine.draw(
                &run,
                col_price_left,
                text_y,
                price_c,
                ORDER_BOOK_TEXT_LAYER,
                cx.scene,
            );

            let atxt = format_qty(&state.config, level.quantity);
            let (aw, _) = cx.text_engine.measure(&atxt, font_size);
            let run = cx.text_engine.shape(&atxt, font_size);
            cx.text_engine.draw(
                &run,
                col_amount_right - aw,
                text_y,
                amount_c,
                ORDER_BOOK_TEXT_LAYER,
                cx.scene,
            );

            let ttxt = format_total(&state.config, bid_cum);
            let (tw, _) = cx.text_engine.measure(&ttxt, font_size);
            let run = cx.text_engine.shape(&ttxt, font_size);
            cx.text_engine.draw(
                &run,
                col_total_right - tw,
                text_y,
                text_color,
                ORDER_BOOK_TEXT_LAYER,
                cx.scene,
            );

            let row_idx = ask_count + 1 + display_i;
            if state.hovered_row == Some(row_idx) {
                let hover_tint = cx.theme.colors.accents.primary.with_alpha(0.25);
                cx.scene.push_rect(
                    RectF::new(rect.x, y, rect.width, row_h),
                    hover_tint,
                    LAYER_CONTENT + 5,
                    None,
                );
                let edge = cx.theme.colors.accents.primary.with_alpha(0.9);
                cx.scene.push_rect(
                    RectF::new(rect.x, y, 3.0, row_h),
                    edge,
                    LAYER_CONTENT + 6,
                    None,
                );
            }

            y += row_h;
        }
    }

    fn needs_animation(&self) -> bool {
        self.state.borrow().has_active_flashes()
    }

    fn handle_event(&mut self, _cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }
        match event {
            Event::Pointer(pe) => {
                let mut state = self.state.borrow_mut();
                let row_h = state.row_h;
                if row_h <= 0.0 {
                    return false;
                }

                let (_, ly) = pe.local_pos;
                let ask_count = state.asks.levels.len();
                let spread_h = state.spread_h;

                let ask_rows_h = ask_count as f32 * row_h;
                let row_idx = if ly < 0.0 {
                    None
                } else if ly < ask_rows_h {
                    Some((ly / row_h).floor() as usize)
                } else if ly < ask_rows_h + spread_h {
                    None // spread
                } else {
                    let bid_y = ly - ask_rows_h - spread_h;
                    let bid_i = (bid_y / row_h).floor() as usize;
                    if bid_i < state.bids.levels.len() {
                        Some(ask_count + 1 + bid_i)
                    } else {
                        None
                    }
                };

                // Helper: extract level info by row index (copies values to avoid borrow conflict)
                let level_info = row_idx.and_then(|idx| {
                    if idx < ask_count {
                        let ask_idx = ask_count - 1 - idx;
                        let l = &state.asks.levels[ask_idx];
                        Some((l.price, BookSideKind::Ask, l.quantity, l.classification))
                    } else if idx > ask_count {
                        let bid_idx = idx - ask_count - 1;
                        state
                            .bids
                            .levels
                            .get(bid_idx)
                            .map(|l| (l.price, BookSideKind::Bid, l.quantity, l.classification))
                    } else {
                        None
                    }
                });

                match pe.kind {
                    PointerEventKind::Move => {
                        let old = state.hovered_row;
                        state.hovered_row = row_idx;
                        if old != state.hovered_row {
                            if let Some((price, side, qty, _)) = level_info {
                                state.push_event(OrderBookEvent::LevelHoverEnter {
                                    price,
                                    side,
                                    quantity: qty,
                                });
                            } else if old.is_some() {
                                state.push_event(OrderBookEvent::LevelHoverLeave);
                            }
                        }
                        old != state.hovered_row
                    }
                    PointerEventKind::Down => {
                        if let Some((price, side, qty, classification)) = level_info {
                            state.push_event(OrderBookEvent::LevelClicked {
                                price,
                                side,
                                quantity: qty,
                                classification,
                            });
                        }
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn debug_type_name(&self) -> &'static str {
        "OrderBookBody"
    }
}

impl WidgetDowncast for OrderBookBody {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::app::RetainedSceneTestHarness;
    use crate::core::icon::IconRegistry;
    use crate::core::layout::Constraints;
    use crate::core::scene::{RectNode, SceneNode, TextNode};
    use crate::core::theme::Theme;
    use crate::core::widget_tree::WidgetTree;
    use crate::model::order_book::{BookLevel, BookSide, LevelClassification};
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use crate::widgets::order_book::OrderBook;

    fn side(price: f64) -> BookSide {
        BookSide {
            levels: vec![BookLevel {
                price,
                quantity: 2.0,
                cumulative: 2.0,
                order_count: None,
                classification: LevelClassification::Normal,
                metadata: Vec::new(),
            }],
        }
    }

    fn collect<'a>(
        nodes: &'a [SceneNode],
        texts: &mut Vec<&'a TextNode>,
        rects: &mut Vec<&'a RectNode>,
    ) {
        for node in nodes {
            match node {
                SceneNode::Text(text) => texts.push(text),
                SceneNode::Rect(rect) => rects.push(rect),
                SceneNode::RetainedFragment(fragment) => collect(&fragment.children, texts, rects),
                SceneNode::Clip(clip) => collect(&clip.children, texts, rects),
                SceneNode::Layer(layer) => collect(&layer.children, texts, rects),
                SceneNode::Scroll(scroll) => collect(&scroll.children, texts, rects),
                _ => {}
            }
        }
    }

    #[test]
    fn explicit_text_z_order_book_row_text_is_above_every_fill() {
        let book = OrderBook::new()
            .with_asks(side(101.0))
            .with_bids(side(99.0));
        let body = book.create_body();
        {
            let mut state = body.state.borrow_mut();
            state.row_h = state.config.row_height;
            state.spread_h = state.config.row_height;
            state.hovered_row = Some(0);
        }

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(body), None, 0);
        let mut harness = RetainedSceneTestHarness::new();
        let mut text_engine = FontManagerTextEngine::new();
        let frame = harness.frame(
            &mut tree,
            root,
            Constraints::tight(Size::new(360.0, 57.0)),
            &Theme::default(),
            &mut text_engine,
            &IconRegistry::new(&[]),
        );

        let mut texts = Vec::new();
        let mut rects = Vec::new();
        collect(&frame.scene_root, &mut texts, &mut rects);
        assert!(!texts.is_empty());
        assert!(
            texts
                .iter()
                .all(|text| text.z_index == ORDER_BOOK_TEXT_LAYER)
        );
        let highest_fill = rects.iter().map(|rect| rect.z_index).max().unwrap();
        assert!(highest_fill < ORDER_BOOK_TEXT_LAYER);
    }
}
