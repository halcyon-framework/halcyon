// SPEC-HAL-303: Shared Order Book Types
//
// BookLevel, BookSide, BookSideKind, LevelClassification — shared between
// DepthBarSeries (plot) and OrderBook widget.

/// A single order book price level with identity and metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct BookLevel {
    /// Price (Y coordinate). Unique within a side.
    pub price: f64,
    /// Total quantity at this level.
    pub quantity: f64,
    /// Cumulative quantity from best price to this level.
    pub cumulative: f64,
    /// Number of distinct orders resting at this level.
    pub order_count: Option<u32>,
    /// Consumer-assigned classification.
    pub classification: LevelClassification,
    /// Consumer-defined metadata lines (for tooltip/hover).
    pub metadata: Vec<String>,
}

/// Visual classification of a price level.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum LevelClassification {
    #[default]
    Normal,
    Wall,
    SuspectedSpoof,
    Persistent,
    Iceberg {
        inferred_total: f64,
    },
    OwnOrder,
}

/// One side of the order book.
#[derive(Debug, Clone, PartialEq)]
pub struct BookSide {
    pub levels: Vec<BookLevel>,
}

impl BookSide {
    pub fn new() -> Self {
        Self { levels: Vec::new() }
    }

    pub fn best_price(&self) -> Option<f64> {
        self.levels.first().map(|l| l.price)
    }

    pub fn max_quantity(&self) -> f64 {
        self.levels.iter().map(|l| l.quantity).fold(0.0, f64::max)
    }
}

impl Default for BookSide {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookSideKind {
    Bid,
    Ask,
}
