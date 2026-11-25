// =============================================================================
// EventLog Demo (SPEC-HAL-312 Phase 1)
// =============================================================================
//
// Demonstrates the EventLog widget composing ListView with:
// - 4 columns: Severity (pinned), Timestamp, Source, Message
// - SeverityPainter with colored indicators per level
// - Row selection, keyboard navigation, Ctrl+C structured copy
// - 200 entries spanning all 6 severity levels
//
// =============================================================================

use std::cell::Cell;
use std::rc::Rc;

use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::FlexParams;
use halcyon::core::scroll_container::HorizontalScrollLink;
use halcyon::core::widget::Widget;
use halcyon::model::tabular::row_expansion::ExpansionPolicy;
use halcyon::prelude::*;
use halcyon::widgets::event_log::{EventLog, EventLogConfig, EventLogEntry, EventLogLevel};
use halcyon::widgets::flex::Flex;
use halcyon::widgets::label::Label;
use halcyon::widgets::scroll_area::ScrollArea;

// =============================================================================
// Configuration
// =============================================================================

const WINDOW_WIDTH: f32 = 900.0;
const WINDOW_HEIGHT: f32 = 700.0;
const FOOTER_HEIGHT: f32 = 24.0;

// =============================================================================
// Demo Data Generation
// =============================================================================

fn generate_entries(count: usize) -> Vec<EventLogEntry> {
    let sources = ["engine", "broker", "ui", "net"];
    let levels = [
        EventLogLevel::Trace,
        EventLogLevel::Debug,
        EventLogLevel::Info,
        EventLogLevel::Warning,
        EventLogLevel::Error,
        EventLogLevel::Critical,
    ];

    let messages: &[&str] = &[
        "Connection established to primary endpoint",
        "Processing batch of 1024 records",
        "Cache miss for key 'session_token_42'",
        "Heartbeat timeout — retrying in 5s",
        "Failed to parse response: unexpected EOF at byte 1337",
        "Fatal: consensus quorum lost, entering degraded mode",
        "Loaded configuration from /etc/halcyon/config.toml",
        "Websocket handshake completed in 12ms",
        "Memory usage at 78% — approaching threshold",
        "Disk write latency spike: 450ms (avg: 2ms)",
        "Authentication succeeded for user admin@example.com",
        "Rate limiter engaged: 1000 req/s exceeded",
        "Snapshot checkpoint written to /data/snapshots/2026-04-16",
        "TLS certificate expires in 14 days",
        "Order matched: BUY 100 @ 42850.00",
        "Market data feed reconnected after 3.2s gap",
        "Schema migration v47 applied successfully",
        "Deadlock detected in pool thread #7 — recovering",
        "Gradient descent converged after 847 iterations",
        "GPU memory allocation failed: 0 bytes free of 8GB",
    ];

    let mut entries = Vec::with_capacity(count);
    // Base timestamp: 2026-04-16 09:30:00.000 UTC in epoch-ms.
    let base_ts: i64 = 1_776_508_200_000;

    for i in 0..count {
        let level = levels[i % levels.len()];
        let source = sources[i % sources.len()];
        let message = messages[i % messages.len()];
        let ts = base_ts + (i as i64 * 150); // 150ms apart

        let detail = format!(
            "Event #{i} — {source}/{level:?}\n\
             Timestamp: {ts} ms (UTC)\n\
             Correlation ID: evt-{i:06}\n\
             Subsystem: {source}\n\
             \n\
             {message}\n\
             \n\
             Upstream status: nominal — no anomalies detected in \
             preceding pipeline stages. All health checks passed \
             within expected thresholds. The monitoring subsystem \
             confirmed zero packet loss across all three redundant \
             network paths. Latency remained below the 99th percentile \
             target of 2.5ms throughout the observation window.\n\
             \n\
             Downstream impact assessment: No cascading effects \
             observed. All dependent services acknowledged the event \
             within their configured timeout windows. The circuit \
             breaker remained in closed state. Message queue depth \
             stayed within normal operating range (12/10000 capacity). \
             Retry budget: 847 of 1000 remaining.\n\
             \n\
             Diagnostic payload: request_id=req-{i:08x} trace_id=\
             trc-{i:012x} span_id=spn-{i:06x} parent_span=none \
             sampling_rate=0.01 flags=0x0042 baggage=region:eu-west-1,\
             dc:fra-2,rack:r14,slot:s07 user_agent=halcyon-engine/\
             0.42.0 protocol=grpc content_length=4096 \
             compression=zstd"
        );
        let entry = EventLogEntry::new(level, ts, source.to_string(), message.to_string())
            .with_detail(detail);
        entries.push(entry);
    }
    entries
}

// =============================================================================
// Demo Screen
// =============================================================================

struct EventLogDemoScreen {
    placeholder: Label,
    built: bool,
}

impl EventLogDemoScreen {
    fn new() -> Self {
        Self {
            placeholder: Label::new(""),
            built: false,
        }
    }
}

impl Screen for EventLogDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        if self.built {
            return &mut self.placeholder;
        }
        self.built = true;

        // Demo uses MultiExpand (default is SingleExpand).
        let mut config = EventLogConfig::default();
        config.expansion_policy = ExpansionPolicy::MultiExpand;
        let mut event_log = EventLog::new(config);

        // Populate with demo entries.
        for entry in generate_entries(200) {
            event_log.push(entry);
        }

        // Set footer height.
        event_log.list_view_mut().set_footer_height(FOOTER_HEIGHT);

        // Wire horizontal scroll link.
        let hscroll_link = HorizontalScrollLink::new();
        let hscroll_freeze = Rc::new(Cell::new(false));
        event_log
            .list_view_mut()
            .set_horizontal_scroll_link(hscroll_link.clone(), hscroll_freeze.clone());

        // Create body and footer widgets.
        let body_widget = event_log.create_body_widget();
        let footer_widget = event_log.create_footer_widget();

        // Build widget tree:
        // Root (Flex column)
        //   └─ ListView (from EventLog, flexible)
        //        ├─ ScrollArea (h-scroll enabled)
        //        │    └─ ListViewBody
        //        └─ ListViewFooter
        let tree_root = cx.tree.root().expect("Tree must have root");
        let root = cx
            .tree
            .add_child(tree_root, Box::new(Flex::column()), None, 0);

        // Take the ListView out of EventLog for tree insertion.
        let list_view = event_log.take_list_view();
        let list_view_id = cx.tree.add_child(root, Box::new(list_view), None, 0);
        cx.tree.set_flex(list_view_id, FlexParams::flexible(1.0));

        // ScrollArea.
        let scroll_area = ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .with_horizontal_scroll_link(hscroll_link)
            .with_horizontal_freeze(hscroll_freeze);
        let scroll_area_id = cx
            .tree
            .add_child(list_view_id, Box::new(scroll_area), None, 0);

        // Body.
        let body_id = cx
            .tree
            .add_child(scroll_area_id, Box::new(body_widget), None, 0);

        // Wire ScrollArea to body.
        if let Some(node) = cx.tree.node_mut(scroll_area_id) {
            if let Some(sa) = node.widget_mut().as_any_mut().downcast_mut::<ScrollArea>() {
                sa.wire_content(body_id);
            }
        }

        // Footer.
        let _footer_id = cx
            .tree
            .add_child(list_view_id, Box::new(footer_widget), None, 0);

        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {
        println!("=== EventLog Demo (SPEC-HAL-312 Phase 1 + 312A Phase 2) ===");
        println!();
        println!("Configuration:");
        println!("  - 200 entries across 6 severity levels");
        println!("  - 4 columns: Severity (pinned), Time, Source, Message");
        println!("  - Row height: 24px (collapsed), 96px (expanded)");
        println!("  - Expansion policy: SingleExpand (one row at a time)");
        println!();
        println!("Severity Indicators:");
        println!("  TRCE: small gray square");
        println!("  DBUG: small light-gray square");
        println!("  INFO: small blue square");
        println!("  WARN: medium amber square");
        println!("  ERR : large red square");
        println!("  CRIT: large red square");
        println!();
        println!("Controls:");
        println!("  - Click: select row");
        println!("  - Shift+Click: range select");
        println!("  - Ctrl+Click: toggle select");
        println!("  - Arrow Up/Down: navigate rows");
        println!("  - Home/End: jump to first/last row");
        println!("  - Ctrl+A: select all");
        println!("  - Enter: toggle row expansion (opens/closes detail panel)");
        println!();
        println!("Close window to exit.");
        println!("=========================================================");
    }

    fn mark_rebuild(&mut self) {}

    fn take_rebuild_flag(&mut self) -> bool {
        false
    }
}

// =============================================================================
// Main
// =============================================================================

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app = App::new(
        AppConfig::new("EventLog Demo (SPEC-HAL-312)").with_size(WINDOW_WIDTH, WINDOW_HEIGHT),
    );
    app.run(EventLogDemoScreen::new());
}
