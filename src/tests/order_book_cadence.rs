use crate::core::widget::{UpdateContext, Widget};
use crate::model::order_book::{BookLevel, BookSide, LevelClassification};
use crate::plot::MarketDetailLink;
use crate::widgets::aligned_order_book::AlignedOrderBook;
use crate::widgets::order_book::{OrderBook, OrderBookUpdate, PriceDirection};

fn side(price: f64, quantity: f64) -> BookSide {
    BookSide {
        levels: vec![BookLevel {
            price,
            quantity,
            cumulative: quantity,
            order_count: None,
            classification: LevelClassification::Normal,
            metadata: Vec::new(),
        }],
    }
}

fn side_with_quantities(base_price: f64, quantities: &[f64]) -> BookSide {
    BookSide {
        levels: quantities
            .iter()
            .enumerate()
            .map(|(index, quantity)| BookLevel {
                price: base_price + index as f64,
                quantity: *quantity,
                cumulative: quantities[..=index].iter().sum(),
                order_count: None,
                classification: LevelClassification::Normal,
                metadata: Vec::new(),
            })
            .collect(),
    }
}

#[test]
fn aligned_order_book_idle_receiver_is_update_work_not_animation() {
    let (mut book, _tx) =
        AlignedOrderBook::new(MarketDetailLink::new(90.0, 110.0)).with_update_channel();
    let mut update = UpdateContext::new(None);

    assert!(book.needs_update());
    assert!(!book.needs_animation());

    update.begin_widget_update();
    book.update(1.0 / 120.0, &mut update);
    assert_eq!(update.take_widget_invalidations(), (false, false, false));
}

#[test]
fn aligned_order_book_coalesces_a_burst_into_one_visible_paint_change() {
    let (mut book, tx) =
        AlignedOrderBook::new(MarketDetailLink::new(90.0, 110.0)).with_update_channel();
    let mut update = UpdateContext::new(None);
    book.project_viewport_for_test(300.0, 200.0);

    for quantity in [1.0, 2.0, 3.0, 4.0] {
        tx.send(OrderBookUpdate::Snapshot {
            bids: side(99.0, quantity),
            asks: side(101.0, quantity),
        })
        .unwrap();
    }

    update.begin_widget_update();
    book.update(1.0 / 120.0, &mut update);
    assert_eq!(
        update.take_widget_invalidations(),
        (true, false, false),
        "the final visible snapshot is projected once and invalidates only paint"
    );

    update.begin_widget_update();
    book.update(0.0, &mut update);
    assert_eq!(
        update.take_widget_invalidations(),
        (false, false, false),
        "an idle channel does not regenerate paint"
    );

    tx.send(OrderBookUpdate::Snapshot {
        bids: side(99.0, 4.0),
        asks: side(101.0, 4.0),
    })
    .unwrap();
    update.begin_widget_update();
    book.update(0.0, &mut update);
    assert_eq!(
        update.take_widget_invalidations(),
        (false, false, false),
        "an identical final snapshot is not a visible change"
    );
}

#[test]
fn order_book_channel_invalidates_descendant_paint_without_parent_animation() {
    let (mut book, tx) = OrderBook::new()
        .with_bids(side_with_quantities(96.0, &[1.0, 1.0, 1.0, 1.0]))
        .with_asks(side(101.0, 1.0))
        .with_update_channel();
    let body = book.create_body();
    let mut update = UpdateContext::new(None);

    assert!(book.needs_update());
    assert!(!book.needs_animation());

    update.begin_widget_update();
    book.update(1.0 / 120.0, &mut update);
    assert_eq!(update.take_widget_invalidations(), (false, false, false));

    for price in [100.0, 100.1, 100.2, 100.3] {
        tx.send(OrderBookUpdate::LastPrice {
            price,
            direction: PriceDirection::Up,
        })
        .unwrap();
    }
    update.begin_widget_update();
    book.update(1.0 / 120.0, &mut update);
    assert_eq!(
        update.take_widget_invalidations(),
        (true, true, false),
        "shared row presentation changes regenerate the retained child subtree"
    );
    assert!(!book.needs_animation());
    assert!(!body.needs_animation());

    tx.send(OrderBookUpdate::Bids(side_with_quantities(
        96.0,
        &[1.0, 1.0, 2.0, 1.0],
    )))
    .unwrap();
    update.begin_widget_update();
    book.update(1.0 / 120.0, &mut update);
    assert_eq!(update.take_widget_invalidations(), (true, true, false));
    assert!(
        body.needs_animation(),
        "the row body that paints flashes owns their animation cadence"
    );
}

#[test]
fn order_book_row_count_change_requests_layout_but_identical_data_is_idle() {
    let (mut book, tx) = OrderBook::new().with_update_channel();
    let mut update = UpdateContext::new(None);
    let bids = side(99.0, 1.0);

    tx.send(OrderBookUpdate::Bids(bids.clone())).unwrap();
    update.begin_widget_update();
    book.update(1.0 / 120.0, &mut update);
    assert_eq!(
        update.take_widget_invalidations(),
        (true, false, true),
        "row topology changes the scrollable body extent"
    );

    tx.send(OrderBookUpdate::Bids(bids)).unwrap();
    update.begin_widget_update();
    book.update(0.0, &mut update);
    assert_eq!(
        update.take_widget_invalidations(),
        (false, false, false),
        "an identical authoritative snapshot is not a visible change"
    );
}
