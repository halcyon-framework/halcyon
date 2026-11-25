// Showcase — Console tab content.
//
// Ports examples/console.rs (the authoritative Console demo, SPEC-HAL-305) into
// the showcase, substituting `parent` (the showcase's content_node) for the
// standalone's root. The Console streams live log output: build_console seeds a
// backlog and wires the tree; console_tick (driven from ShowcaseScreen::update
// while the Console tab is active) generates new lines over time.
//
//   content_node (passed in by ShowcaseScreen::build)
//     +- Console (flex 1.0)
//          +- ScrollArea (vertical scrollbar; horizontal disabled)
//               +- ConsoleBody
//
// Multi-span colored lines, ANSI parsing, ring-buffer capacity, and auto-scroll
// are exercised exactly as in the standalone demo.

use super::ShowcaseScreen;
use halcyon::core::app::ScreenFrameContext;
use halcyon::core::layout::FlexParams;
use halcyon::core::widget_tree::NodeId;
use halcyon::prelude::*;
use halcyon::widgets::console::{Console, ConsoleLevel, ConsoleLine, ConsoleSpan, ConsoleStyle};
use halcyon::widgets::scroll_area::ScrollArea;
use web_time::{Duration, Instant};

impl ShowcaseScreen {
    /// SplitMix-style LCG for deterministic demo data (mirrors the standalone
    /// demo's `next_rand`, operating on the showcase's `console_seed`).
    fn console_next_rand(&mut self) -> u64 {
        self.console_seed = self
            .console_seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.console_seed
    }

    /// Build the Console tab content. Mirrors examples/console.rs:58-156 with
    /// `parent` substituted for the standalone root and `console_id` retained.
    pub(super) fn build_console(&mut self, cx: &mut ScreenFrameContext<'_>, parent: NodeId) {
        let config = halcyon::widgets::console::ConsoleConfig {
            policy: halcyon::widgets::console::ConsolePolicy::Selectable,
            overflow: halcyon::widgets::console::ConsoleOverflowMode::Wrap,
            ..Default::default()
        };
        let console = Console::with_config(config).with_capacity(1000);
        let body = console.create_body();

        // Seed an initial backlog (verbatim from examples/console.rs:68-127).
        for i in 0..200 {
            let r = self.console_next_rand();
            let level = match r % 20 {
                0..=11 => ConsoleLevel::Info,
                12..=14 => ConsoleLevel::Debug,
                15..=17 => ConsoleLevel::Warn,
                18 => ConsoleLevel::Error,
                _ => ConsoleLevel::Trace,
            };
            let msgs = [
                "System initialized",
                "Loading configuration",
                "Database connection pool created",
                "Worker thread spawned",
                "Cache warmed up",
                "Health check endpoint registered",
                "TLS certificates loaded",
                "Metrics exporter started",
                "Service discovery registered",
                "Ready to accept connections",
            ];
            let msg = msgs[(r >> 16) as usize % msgs.len()];
            console.push_line(ConsoleLine {
                spans: vec![
                    ConsoleSpan {
                        text: format!("[boot] "),
                        style: ConsoleStyle {
                            fg: Some(halcyon::core::paint::Color::from_hex(0x888888)),
                            ..Default::default()
                        },
                    },
                    ConsoleSpan {
                        text: format!("{:5} ", level.tag()),
                        style: ConsoleStyle {
                            fg: match level {
                                ConsoleLevel::Warn => {
                                    Some(halcyon::core::paint::Color::from_hex(0xF39C12))
                                }
                                ConsoleLevel::Error => {
                                    Some(halcyon::core::paint::Color::from_hex(0xE74C3C))
                                }
                                ConsoleLevel::Debug => {
                                    Some(halcyon::core::paint::Color::from_hex(0x3498DB))
                                }
                                ConsoleLevel::Trace => {
                                    Some(halcyon::core::paint::Color::from_hex(0x7F8C8D))
                                }
                                _ => None,
                            },
                            ..Default::default()
                        },
                    },
                    ConsoleSpan {
                        text: format!("{} (step {})", msg, i + 1),
                        style: ConsoleStyle::default(),
                    },
                ],
                level: Some(level),
            });
        }

        // Console fills the tab; ScrollArea (vertical only) holds the body.
        let console_node = cx.tree.add_child(parent, Box::new(console), None, 0);
        cx.tree.set_flex(console_node, FlexParams::flexible(1.0));
        self.console_id = Some(console_node);

        let scroll_area = ScrollArea::new_unwired()
            .with_scrollbars(true)
            .enable_horizontal_scrollbar(false);
        let sa_id = cx
            .tree
            .add_child(console_node, Box::new(scroll_area), None, 0);

        let body_id = cx.tree.add_child(sa_id, Box::new(body), None, 0);

        if let Some(node) = cx.tree.node_mut(sa_id) {
            if let Some(sa) = node.widget_mut().as_any_mut().downcast_mut::<ScrollArea>() {
                sa.wire_content(body_id);
            }
        }

        if let Some(node) = cx.tree.node_mut(console_node) {
            if let Some(c) = node.widget_mut().as_any_mut().downcast_mut::<Console>() {
                c.set_scroll_area(sa_id);
            }
        }

        // Restart the line generator's timing for this mount.
        self.console_next_gen = Instant::now();
        self.console_burst_remaining = 0;
    }

    /// Generate streaming log output. Driven from ShowcaseScreen::update while
    /// the Console tab is active. Body ported from examples/console.rs (see A.1).
    pub(super) fn console_tick(&mut self, tree: &mut halcyon::core::widget_tree::WidgetTree) {
        let now = Instant::now();

        // Burst mode: if we have remaining lines in a burst, emit them
        // with very short delays (5-15ms) for rapid-fire effect.
        if self.console_burst_remaining > 0 {
            if now < self.console_next_gen {
                return;
            }
            self.console_burst_remaining -= 1;
            let r = self.console_next_rand();
            // Short inter-burst delay: 1-3ms
            let delay_ms = 1 + (r % 3);
            self.console_next_gen = now + Duration::from_millis(delay_ms);
        } else {
            // Not in burst: check if it's time for next emission.
            if now < self.console_next_gen {
                return;
            }
            let r = self.console_next_rand();
            // Variable interval: mix of quick singles and bursts.
            // 15% chance: single line, 200-1200ms gap (calm lull)
            // 45% chance: burst of 5-14 lines with short delays
            // 25% chance: single line, 10-40ms gap (rapid singles)
            // 15% chance: burst of 14-28 lines (flood)
            let pattern = r % 100;
            if pattern < 15 {
                // Calm lull — longer max for contrast
                let delay_ms = 200 + (self.console_next_rand() % 1000);
                self.console_next_gen = now + Duration::from_millis(delay_ms);
            } else if pattern < 60 {
                // Small burst
                self.console_burst_remaining = 5 + (self.console_next_rand() % 12) as u32;
                self.console_next_gen = now + Duration::from_millis(1);
            } else if pattern < 85 {
                // Rapid singles
                let delay_ms = 10 + (self.console_next_rand() % 30);
                self.console_next_gen = now + Duration::from_millis(delay_ms);
            } else {
                // Flood burst
                self.console_burst_remaining = 15 + (self.console_next_rand() % 20) as u32;
                self.console_next_gen = now + Duration::from_millis(1);
            }
        }

        // --- Generate one line ---
        let r = self.console_next_rand();
        let level = match r % 10 {
            0..=4 => ConsoleLevel::Info,
            5..=6 => ConsoleLevel::Warn,
            7 => ConsoleLevel::Error,
            8 => ConsoleLevel::Debug,
            _ => ConsoleLevel::Trace,
        };

        // Timestamp span (dimmed gray)
        let elapsed = now.duration_since(self.console_start);
        let secs = elapsed.as_secs();
        let ts = format!(
            "{:02}:{:02}:{:02}",
            secs / 3600,
            (secs % 3600) / 60,
            secs % 60
        );

        // 20% chance: use ANSI-colored message.
        let use_ansi = (r >> 8) % 5 == 0;
        if use_ansi {
            let ansi_msgs = [
                "\x1b[1;31mERROR\x1b[0m Connection refused to \x1b[36m10.0.1.42:5432\x1b[0m",
                "\x1b[33mWARN\x1b[0m  CPU usage at \x1b[1;33m87%\x1b[0m -- throttling enabled",
                "\x1b[32mOK\x1b[0m   Health check \x1b[1;32mpassed\x1b[0m (latency: \x1b[34m2ms\x1b[0m)",
                "\x1b[1;35mDEPLOY\x1b[0m Rolling update v2.4.1 -> v2.5.0",
                "\x1b[2;37mTRACE\x1b[0m  \x1b[2mSpan opened: \x1b[36mdb.query\x1b[0m\x1b[2m duration=\x1b[33m142ms\x1b[0m",
            ];
            let ansi_msg = ansi_msgs[(r >> 12) as usize % ansi_msgs.len()];
            let spans = halcyon::widgets::console::parse_ansi(ansi_msg);

            if let Some(console_id) = self.console_id {
                if let Some(node) = tree.node_mut(console_id) {
                    if let Some(console) = node.widget_mut().as_any_mut().downcast_mut::<Console>()
                    {
                        console.push_line(ConsoleLine {
                            spans,
                            level: Some(level),
                        });
                    }
                }
                tree.mark_layout_dirty(console_id);
            }
            return;
        }

        // Build varied multi-span lines
        let msg_index = (r >> 20) as usize % 15;
        let spans = match msg_index {
            0 => vec![
                ConsoleSpan {
                    text: format!("[{}] ", ts),
                    style: ConsoleStyle {
                        fg: Some(halcyon::core::paint::Color::from_hex(0x888888)),
                        ..Default::default()
                    },
                },
                ConsoleSpan {
                    text: format!("{:5} ", level.tag()),
                    style: ConsoleStyle {
                        fg: match level {
                            ConsoleLevel::Warn => {
                                Some(halcyon::core::paint::Color::from_hex(0xF39C12))
                            }
                            ConsoleLevel::Error => {
                                Some(halcyon::core::paint::Color::from_hex(0xE74C3C))
                            }
                            ConsoleLevel::Debug => {
                                Some(halcyon::core::paint::Color::from_hex(0x3498DB))
                            }
                            ConsoleLevel::Trace => {
                                Some(halcyon::core::paint::Color::from_hex(0x7F8C8D))
                            }
                            _ => None,
                        },
                        ..Default::default()
                    },
                },
                ConsoleSpan {
                    text: "Connection from ".to_string(),
                    style: ConsoleStyle::default(),
                },
                ConsoleSpan {
                    text: format!("10.0.{}.{}", (r >> 8) % 255, (r >> 16) % 255),
                    style: ConsoleStyle {
                        fg: Some(halcyon::core::paint::Color::from_hex(0x2ECC71)),
                        ..Default::default()
                    },
                },
                ConsoleSpan {
                    text: format!(":{}", 1024 + (r % 64000)),
                    style: ConsoleStyle {
                        fg: Some(halcyon::core::paint::Color::from_hex(0x2ECC71)),
                        ..Default::default()
                    },
                },
            ],
            1 => vec![
                ConsoleSpan {
                    text: format!("[{}] ", ts),
                    style: ConsoleStyle {
                        fg: Some(halcyon::core::paint::Color::from_hex(0x888888)),
                        ..Default::default()
                    },
                },
                ConsoleSpan {
                    text: format!("{:5} ", level.tag()),
                    style: ConsoleStyle {
                        fg: match level {
                            ConsoleLevel::Warn => {
                                Some(halcyon::core::paint::Color::from_hex(0xF39C12))
                            }
                            ConsoleLevel::Error => {
                                Some(halcyon::core::paint::Color::from_hex(0xE74C3C))
                            }
                            _ => None,
                        },
                        ..Default::default()
                    },
                },
                ConsoleSpan {
                    text: "Query took ".to_string(),
                    style: ConsoleStyle::default(),
                },
                ConsoleSpan {
                    text: format!("{:.1}ms", (r % 5000) as f32 / 10.0),
                    style: ConsoleStyle {
                        fg: Some(halcyon::core::paint::Color::from_hex(if r % 5000 > 1000 {
                            0xE74C3C
                        } else if r % 5000 > 500 {
                            0xF39C12
                        } else {
                            0x2ECC71
                        })),
                        ..Default::default()
                    },
                },
                ConsoleSpan {
                    text: " on SELECT * FROM orders".to_string(),
                    style: ConsoleStyle {
                        fg: Some(halcyon::core::paint::Color::from_hex(0x9B59B6)),
                        ..Default::default()
                    },
                },
            ],
            2 => vec![
                ConsoleSpan {
                    text: format!("[{}] ", ts),
                    style: ConsoleStyle {
                        fg: Some(halcyon::core::paint::Color::from_hex(0x888888)),
                        ..Default::default()
                    },
                },
                ConsoleSpan {
                    text: format!("{:5} ", level.tag()),
                    style: ConsoleStyle {
                        fg: match level {
                            ConsoleLevel::Warn => {
                                Some(halcyon::core::paint::Color::from_hex(0xF39C12))
                            }
                            ConsoleLevel::Error => {
                                Some(halcyon::core::paint::Color::from_hex(0xE74C3C))
                            }
                            _ => None,
                        },
                        ..Default::default()
                    },
                },
                ConsoleSpan {
                    text: format!("CPU: {}%", 10 + r % 90),
                    style: ConsoleStyle {
                        fg: Some(halcyon::core::paint::Color::from_hex(if r % 90 > 80 {
                            0xE74C3C
                        } else if r % 90 > 60 {
                            0xF39C12
                        } else {
                            0x2ECC71
                        })),
                        ..Default::default()
                    },
                },
                ConsoleSpan {
                    text: format!("  MEM: {}%", 30 + r % 60),
                    style: ConsoleStyle {
                        fg: Some(halcyon::core::paint::Color::from_hex(0x3498DB)),
                        ..Default::default()
                    },
                },
                ConsoleSpan {
                    text: format!("  DISK: {}%", 40 + r % 50),
                    style: ConsoleStyle {
                        fg: Some(halcyon::core::paint::Color::from_hex(0x1ABC9C)),
                        ..Default::default()
                    },
                },
            ],
            13 => vec![
                ConsoleSpan {
                    text: format!("[{}] ", ts),
                    style: ConsoleStyle {
                        fg: Some(halcyon::core::paint::Color::from_hex(0x888888)),
                        ..Default::default()
                    },
                },
                ConsoleSpan {
                    text: format!("{:5} ", level.tag()),
                    style: ConsoleStyle {
                        fg: match level {
                            ConsoleLevel::Warn => {
                                Some(halcyon::core::paint::Color::from_hex(0xF39C12))
                            }
                            ConsoleLevel::Error => {
                                Some(halcyon::core::paint::Color::from_hex(0xE74C3C))
                            }
                            ConsoleLevel::Debug => {
                                Some(halcyon::core::paint::Color::from_hex(0x3498DB))
                            }
                            ConsoleLevel::Trace => {
                                Some(halcyon::core::paint::Color::from_hex(0x7F8C8D))
                            }
                            _ => None,
                        },
                        ..Default::default()
                    },
                },
                ConsoleSpan {
                    text: format!(
                        "Long diagnostic: request_id={} client=10.0.{}.{} method=POST path=/api/v2/instruments/BTCUSDT/orders latency_ms={} status=200 bytes_in=1024 bytes_out={} cache=miss retry=0 trace_id=a1b2c3d4e5f6 span_id=7890abcd",
                        1000 + (r % 9999),
                        (r >> 8) % 255,
                        (r >> 16) % 255,
                        1 + (r % 500),
                        256 + (r % 8192)
                    ),
                    style: ConsoleStyle::default(),
                },
            ],
            14 => vec![
                ConsoleSpan {
                    text: format!("[{}] ", ts),
                    style: ConsoleStyle {
                        fg: Some(halcyon::core::paint::Color::from_hex(0x888888)),
                        ..Default::default()
                    },
                },
                ConsoleSpan {
                    text: "ERROR ".to_string(),
                    style: ConsoleStyle {
                        fg: Some(halcyon::core::paint::Color::from_hex(0xE74C3C)),
                        bold: true,
                        ..Default::default()
                    },
                },
                ConsoleSpan {
                    text: format!(
                        "Stack trace: OrderValidation::validate() -> RiskEngine::check_limits() -> PortfolioManager::available_margin() -> AccountService::get_balance(account_id={}) -> DatabasePool::execute(query=\"SELECT available, reserved FROM balances WHERE account_id = $1\") -> ConnectionError: timeout after 30000ms, retry_count=3, last_error=\"connection reset by peer\"",
                        10000 + (r % 90000)
                    ),
                    style: ConsoleStyle {
                        fg: Some(halcyon::core::paint::Color::from_hex(0xE74C3C)),
                        ..Default::default()
                    },
                },
            ],
            _ => {
                // Simple single-message lines with level coloring
                let simple_msgs = [
                    "Health check passed",
                    "Cache miss on key user:1234",
                    "Worker pool resized to 12",
                    "TLS handshake completed",
                    "Request timeout after 30s",
                    "Disk usage approaching threshold",
                    "Config reload: 3 services updated",
                    "Rate limit hit: client batch-07",
                    "Garbage collection completed in 12ms",
                    "New deployment detected: v2.4.1",
                    "Backup snapshot created successfully",
                    "Certificate expires in 30 days",
                ];
                let msg = simple_msgs[msg_index % simple_msgs.len()];
                vec![
                    ConsoleSpan {
                        text: format!("[{}] ", ts),
                        style: ConsoleStyle {
                            fg: Some(halcyon::core::paint::Color::from_hex(0x888888)),
                            ..Default::default()
                        },
                    },
                    ConsoleSpan {
                        text: format!("{:5} ", level.tag()),
                        style: ConsoleStyle {
                            fg: match level {
                                ConsoleLevel::Warn => {
                                    Some(halcyon::core::paint::Color::from_hex(0xF39C12))
                                }
                                ConsoleLevel::Error => {
                                    Some(halcyon::core::paint::Color::from_hex(0xE74C3C))
                                }
                                ConsoleLevel::Debug => {
                                    Some(halcyon::core::paint::Color::from_hex(0x3498DB))
                                }
                                ConsoleLevel::Trace => {
                                    Some(halcyon::core::paint::Color::from_hex(0x7F8C8D))
                                }
                                _ => None,
                            },
                            ..Default::default()
                        },
                    },
                    ConsoleSpan {
                        text: msg.to_string(),
                        style: ConsoleStyle::default(),
                    },
                ]
            }
        };

        if let Some(console_id) = self.console_id {
            if let Some(node) = tree.node_mut(console_id) {
                if let Some(console) = node.widget_mut().as_any_mut().downcast_mut::<Console>() {
                    console.push_line(ConsoleLine {
                        spans,
                        level: Some(level),
                    });
                }
            }
            tree.mark_layout_dirty(console_id);
        }
    }
}
