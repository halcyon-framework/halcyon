// SPEC-HAL-334 §3.2: TimeValue — timezone-neutral civil time of day.
// Relocated verbatim from widgets/time_picker.rs. Semantics per SPEC-HAL-273.

/// Construction-time presentation; stored time is always 24-hour civil time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeDisplayMode {
    #[default]
    H24,
    H12,
}

/// Time of day value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeValue {
    pub hour: u8,           // 0..24
    pub minute: u8,         // 0..60
    pub second: Option<u8>, // 0..60, if present
}

impl TimeValue {
    pub fn new(hour: u8, minute: u8, second: Option<u8>) -> Self {
        debug_assert!(hour < 24, "hour must be < 24, got {}", hour);
        debug_assert!(minute < 60, "minute must be < 60, got {}", minute);
        if let Some(s) = second {
            debug_assert!(s < 60, "second must be < 60, got {}", s);
        }
        Self {
            hour: hour.min(23),
            minute: minute.min(59),
            second: second.map(|s| s.min(59)),
        }
    }

    pub fn format_24h(&self) -> String {
        match self.second {
            Some(s) => format!("{:02}:{:02}:{:02}", self.hour, self.minute, s),
            None => format!("{:02}:{:02}", self.hour, self.minute),
        }
    }

    pub fn format_12h(&self) -> String {
        let (h, ampm) = if self.hour == 0 {
            (12, "AM")
        } else if self.hour < 12 {
            (self.hour, "AM")
        } else if self.hour == 12 {
            (12, "PM")
        } else {
            (self.hour - 12, "PM")
        };
        match self.second {
            Some(s) => format!("{}:{:02}:{:02} {}", h, self.minute, s, ampm),
            None => format!("{}:{:02} {}", h, self.minute, ampm),
        }
    }

    pub fn is_am(&self) -> bool {
        self.hour < 12
    }
}
