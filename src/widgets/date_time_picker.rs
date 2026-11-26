//! Universal civil date/time picker. All panels are regions of this one Widget.
//! Legacy DatePickerWidget, DatePickerField and TimePicker keep their contracts.

use std::cell::Cell;

use crate::core::event::{
    Event, EventPhase, KeyCode, KeyboardAuthorityMode, PointerButton, PointerEventKind,
};
use crate::core::icon::IconId;
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size, snap_to_physical};
use crate::core::paint::{Color, LAYER_CONTENT, LAYER_OVERLAY, RoundedCorners};
use crate::core::popup::compute_popup_rect;
use crate::core::text_engine::compute_control_text_baseline_y;
use crate::core::theme::{ControlChromeCategory, ControlChromeState};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::{NodeId, OverlayHitRegion, OwnedWidgetPresentationGeometry};
use crate::model::date_time::{CalendarView, CivilDateBounds, DateTimeSelectionState};
use crate::model::date_value::DateValue;
use crate::model::time_value::{TimeDisplayMode, TimeValue};
use crate::renderer::path::StrokeParams;
use crate::utils::id::{TreeWidgetId, WidgetInternalId};
use crate::widgets::combo_box::PopupGeometry;
use crate::widgets::scrollbar::ScrollbarV;

pub use crate::model::date_time::{
    DateTimeEndpoint, DateTimeMode, DateTimeSelection, DateTimeSelectionError, DateTimeValue,
};

/// Presentation of known-unavailable time components. Selection validity is unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UnavailableTimeSlotVisibility {
    #[default]
    Show,
    /// Hide unavailable alternatives, retaining a selected unavailable value truthfully.
    /// H12 Hour/AM/PM components remain editable draft choices even when their
    /// intermediate complete time is unavailable; Apply still requires validity.
    Hide,
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAYS: [&str; 7] = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Column {
    Year,
    Month,
    Calendar,
    Hour,
    Minute,
    Period,
}

impl Column {
    fn scroll_index(self) -> Option<usize> {
        match self {
            Self::Year => Some(0),
            Self::Month => Some(1),
            Self::Hour => Some(2),
            Self::Minute => Some(3),
            Self::Period => Some(4),
            Self::Calendar => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Year(i32),
    Month(u8),
    Day(DateValue),
    Hour(u8),
    Minute(u8),
    Period(bool),
    Endpoint(DateTimeEndpoint),
    Apply,
    Cancel,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct MinuteProjectionBasis {
    date: Option<DateValue>,
    hour: u8,
    endpoint: DateTimeEndpoint,
}

struct MinuteProjection {
    basis: MinuteProjectionBasis,
    targets: Vec<Target>,
}

struct ListRow {
    rect: RectF,
    label: String,
    target: Target,
}

struct ColumnGeometry {
    kind: Column,
    header: RectF,
    body: RectF,
    rows: Vec<ListRow>,
    count: usize,
    scroll: f32,
}

struct PickerGeometry {
    size: Size,
    row_height: f32,
    columns: Vec<ColumnGeometry>,
    cells: Vec<(DateValue, RectF)>,
    start: RectF,
    end: RectF,
    cancel: RectF,
    apply: RectF,
    status: RectF,
    footer_top: f32,
}

impl Default for PickerGeometry {
    fn default() -> Self {
        Self {
            size: Size::zero(),
            row_height: 0.0,
            columns: Vec::new(),
            cells: Vec::new(),
            start: RectF::zero(),
            end: RectF::zero(),
            cancel: RectF::zero(),
            apply: RectF::zero(),
            status: RectF::zero(),
            footer_top: 0.0,
        }
    }
}

/// One popup placement at the existing traversal boundary. Scene primitives are
/// authored before the ancestor ScrollNode; event/hit sidecars use its resolved
/// absolute logical projection. Neither path independently chooses a position.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct PopupPlacement {
    paint: RectF,
    window: RectF,
    anchor_window: RectF,
    visible_window: RectF,
    presentation: Option<OwnedWidgetPresentationGeometry>,
}

impl PopupPlacement {
    fn new(anchor: RectF, scroll: (f32, f32), bounds: RectF, size: Size) -> Self {
        let anchor_window = RectF::new(
            anchor.x - scroll.0,
            anchor.y - scroll.1,
            anchor.width,
            anchor.height,
        );
        let (local, _) = compute_popup_rect(
            anchor_window.translate(-bounds.x, -bounds.y),
            size.width,
            size.height,
            bounds.width,
            bounds.height,
        );
        let window = local.translate(bounds.x, bounds.y);
        let paint = RectF::new(
            window.x + scroll.0,
            window.y + scroll.1,
            window.width,
            window.height,
        );
        Self {
            paint,
            window,
            anchor_window,
            visible_window: window.intersection(&bounds).unwrap_or(RectF::zero()),
            presentation: None,
        }
    }

    fn chrome_geometry(&self, anchor: RectF, scale: f32) -> PopupGeometry {
        let mut geometry = PopupGeometry::from_rects(anchor, self.paint).snapped(scale);
        // Both clip scopes share the same physical attachment edge, including
        // above-placement subtraction at fractional device scales.
        if self.paint.y < anchor.y {
            geometry.popup.height = geometry.control.y - geometry.popup.y;
        } else {
            let bottom = geometry.popup.bottom();
            geometry.popup.y = geometry.control.bottom();
            geometry.popup.height = bottom - geometry.popup.y;
        }
        geometry
    }
}

struct PanelScroll {
    value: Option<f32>,
    bar: ScrollbarV,
}

impl Default for PanelScroll {
    fn default() -> Self {
        Self {
            value: None,
            bar: ScrollbarV::new(),
        }
    }
}

/// Availability is supplied by the application, never inferred from rendering.
#[derive(Default)]
struct Availability {
    bounds: Option<CivilDateBounds>,
    date: Option<Box<dyn Fn(DateValue) -> bool + Send + Sync>>,
    point: Option<Box<dyn Fn(DateTimeValue) -> bool + Send + Sync>>,
    range: Option<Box<dyn Fn(DateTimeSelection) -> bool + Send + Sync>>,
    reference_date: Option<DateValue>,
}

impl Availability {
    fn point(
        &self,
        value: DateTimeSelection,
        endpoint: DateTimeEndpoint,
    ) -> Result<(), DateTimeSelectionError> {
        let date = value.date(endpoint).or(self.reference_date);
        if self
            .bounds
            .is_some_and(|bounds| date.is_some_and(|date| !bounds.contains(date)))
        {
            return Err(DateTimeSelectionError::OutsideDateBounds);
        }
        if let Some(policy) = &self.date {
            let date = date.ok_or(DateTimeSelectionError::MissingDateContext)?;
            if !policy(date) {
                return Err(DateTimeSelectionError::Unavailable);
            }
        }
        if let Some(policy) = &self.point {
            let date = date.ok_or(DateTimeSelectionError::MissingDateContext)?;
            let time = value
                .time(endpoint)
                .ok_or(DateTimeSelectionError::ModeMismatch)?;
            if !policy(DateTimeValue { date, time }) {
                return Err(DateTimeSelectionError::Unavailable);
            }
        }
        Ok(())
    }

    fn selection(&self, value: DateTimeSelection) -> Result<(), DateTimeSelectionError> {
        value.validate()?;
        if let Some(bounds) = self.bounds {
            for endpoint in [DateTimeEndpoint::Start, DateTimeEndpoint::End] {
                if value
                    .date(endpoint)
                    .is_some_and(|date| !bounds.contains(date))
                {
                    return Err(DateTimeSelectionError::OutsideDateBounds);
                }
            }
        }
        if value.mode().is_range() {
            // The application owns both boundaries and the entire interval.
            // An excluded end may itself be an unavailable individual point.
            if let Some(policy) = &self.range {
                return if policy(value) {
                    Ok(())
                } else {
                    Err(DateTimeSelectionError::Unavailable)
                };
            }
            if (self.date.is_some() || self.point.is_some()) && self.range.is_none() {
                return Err(DateTimeSelectionError::MissingRangePolicy);
            }
            return Ok(());
        }
        self.point(value, DateTimeEndpoint::Start)
    }
}

/// A compact anchored picker with Date, Time, DateTime and two range modes.
///
/// Construction requires an explicit complete initial value. The value is never
/// rounded to a menu increment. Capture exit retains the working value; Cancel
/// discards it, and Apply/Enter commits a complete currently available selection.
pub struct DateTimePicker {
    id: WidgetInternalId,
    state: DateTimeSelectionState,
    display_mode: TimeDisplayMode,
    hour_step: u8,
    minute_step: u8,
    unavailable_time_slots: UnavailableTimeSlotVisibility,
    minute_projection: Option<MinuteProjection>,
    endpoint: DateTimeEndpoint,
    calendar: Option<CalendarView>,
    cursor: Option<DateValue>,
    column: Column,
    popup_open: bool,
    hovered: Option<Target>,
    armed: Option<Target>,
    dragging_column: Option<Column>,
    scroll: [PanelScroll; 5],
    geometry: PickerGeometry,
    popup_projection: Cell<PopupPlacement>,
    availability: Availability,
    on_commit: Option<Box<dyn FnMut(DateTimeSelection)>>,
    width: Option<f32>,
}

impl DateTimePicker {
    pub fn new(value: DateTimeSelection) -> Result<Self, DateTimeSelectionError> {
        let state = DateTimeSelectionState::new(value)?;
        let date = value.date(DateTimeEndpoint::Start);
        Ok(Self {
            id: WidgetInternalId::new(),
            state,
            display_mode: TimeDisplayMode::H24,
            hour_step: 1,
            minute_step: 1,
            unavailable_time_slots: UnavailableTimeSlotVisibility::Show,
            minute_projection: None,
            endpoint: DateTimeEndpoint::Start,
            calendar: date.map(|date| CalendarView::new(date, 0)),
            cursor: date,
            column: if date.is_some() {
                Column::Calendar
            } else {
                Column::Hour
            },
            popup_open: false,
            hovered: None,
            armed: None,
            dragging_column: None,
            scroll: std::array::from_fn(|_| PanelScroll::default()),
            geometry: PickerGeometry::default(),
            popup_projection: Cell::new(PopupPlacement::default()),
            availability: Availability::default(),
            on_commit: None,
            width: None,
        })
    }

    pub fn with_display_mode(mut self, display_mode: TimeDisplayMode) -> Self {
        self.display_mode = display_mode;
        self
    }

    /// Configure time-column presentation before mounting. The default shows disabled
    /// slots. Hide keeps the current value visible, even if unavailable or off step;
    /// it never changes selection, point/range policy, or Apply validity.
    pub fn with_unavailable_time_slots(
        mut self,
        visibility: UnavailableTimeSlotVisibility,
    ) -> Self {
        self.unavailable_time_slots = visibility;
        self
    }

    /// Increments define offered options only. Existing off-step components stay exact.
    pub fn with_steps(mut self, hours: u8, minutes: u8) -> Result<Self, DateTimeSelectionError> {
        if !(1..=24).contains(&hours) || !(1..=60).contains(&minutes) {
            return Err(DateTimeSelectionError::InvalidStep);
        }
        self.hour_step = hours;
        self.minute_step = minutes;
        Ok(self)
    }

    pub fn with_width(mut self, width: f32) -> Self {
        self.width = width.is_finite().then_some(width.max(0.0));
        self
    }

    pub fn with_first_day_of_week(mut self, monday_first: bool) -> Self {
        if let Some(calendar) = &mut self.calendar {
            calendar.first_dow = u8::from(monday_first);
        }
        self
    }

    /// Inclusive civil endpoint bounds for this picker. Existing values remain
    /// unchanged and are reported unavailable if the bounds exclude them.
    pub fn with_date_bounds(
        mut self,
        min: DateValue,
        max: DateValue,
    ) -> Result<Self, DateTimeSelectionError> {
        self.set_date_bounds(min, max)?;
        Ok(self)
    }

    pub fn with_year_bounds(
        self,
        min_year: i32,
        max_year: i32,
    ) -> Result<Self, DateTimeSelectionError> {
        self.with_date_bounds(
            DateValue::new(min_year, 1, 1).ok_or(DateTimeSelectionError::InvalidDate)?,
            DateValue::new(max_year, 12, 31).ok_or(DateTimeSelectionError::InvalidDate)?,
        )
    }

    /// Update bounds through ordinary scoped widget access, then invalidate Layout.
    /// Only the browsing position is constrained; neither draft nor commit is rewritten.
    pub fn set_date_bounds(
        &mut self,
        min: DateValue,
        max: DateValue,
    ) -> Result<(), DateTimeSelectionError> {
        if !self.mode().has_date() {
            return Err(DateTimeSelectionError::ModeMismatch);
        }
        self.availability.bounds = Some(CivilDateBounds::new(min, max)?);
        self.select_endpoint(self.endpoint);
        Ok(())
    }

    pub fn date_bounds(&self) -> Option<(DateValue, DateValue)> {
        self.availability
            .bounds
            .map(|bounds| (bounds.min, bounds.max))
    }

    /// Remove the optional bounds, then invalidate Layout through the caller's owner.
    pub fn clear_date_bounds(&mut self) {
        self.availability.bounds = None;
        for scroll in &mut self.scroll {
            scroll.value = None;
        }
    }

    pub fn with_reference_date(mut self, date: DateValue) -> Result<Self, DateTimeSelectionError> {
        DateTimeSelection::Date(date).validate()?;
        self.availability.reference_date = Some(date);
        Ok(self)
    }

    /// Pure date policy. Time-only mode also needs `with_reference_date`.
    pub fn with_date_availability(
        mut self,
        policy: impl Fn(DateValue) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.availability.date = Some(Box::new(policy));
        self
    }

    /// Pure policy over the complete civil date and exact time, including seconds.
    /// Date-only modes do not admit a time policy. Time-only mode needs an explicit date.
    pub fn with_date_time_availability(
        mut self,
        policy: impl Fn(DateTimeValue) -> bool + Send + Sync + 'static,
    ) -> Result<Self, DateTimeSelectionError> {
        if !self.mode().has_time() {
            return Err(DateTimeSelectionError::ModeMismatch);
        }
        if self.mode() == DateTimeMode::Time && self.availability.reference_date.is_none() {
            return Err(DateTimeSelectionError::MissingDateContext);
        }
        self.availability.point = Some(Box::new(policy));
        Ok(self)
    }

    /// The application qualifies the entire inclusive date or half-open date/time range.
    /// It must account for occupied or nonworking interior points; the picker never samples them.
    pub fn with_range_availability(
        mut self,
        policy: impl Fn(DateTimeSelection) -> bool + Send + Sync + 'static,
    ) -> Result<Self, DateTimeSelectionError> {
        if !self.mode().is_range() {
            return Err(DateTimeSelectionError::ModeMismatch);
        }
        self.availability.range = Some(Box::new(policy));
        Ok(self)
    }

    pub fn on_commit(mut self, callback: impl FnMut(DateTimeSelection) + 'static) -> Self {
        self.on_commit = Some(Box::new(callback));
        self
    }

    pub fn mode(&self) -> DateTimeMode {
        self.state.committed().mode()
    }
    pub fn value(&self) -> DateTimeSelection {
        self.state.committed()
    }
    pub fn working(&self) -> DateTimeSelection {
        self.state.working()
    }
    pub fn working_availability(&self) -> Result<(), DateTimeSelectionError> {
        self.availability.selection(self.working())
    }

    /// External values remain truthful even if availability has since changed.
    pub fn set_value(&mut self, value: DateTimeSelection) -> Result<(), DateTimeSelectionError> {
        self.state.set_value(value)?;
        self.minute_projection = None;
        self.select_endpoint(DateTimeEndpoint::Start);
        Ok(())
    }

    fn format_time(&self, time: TimeValue) -> String {
        match self.display_mode {
            TimeDisplayMode::H24 => time.format_24h(),
            TimeDisplayMode::H12 => {
                let hour = if time.hour % 12 == 0 {
                    12
                } else {
                    time.hour % 12
                };
                let period = if time.hour < 12 { "AM" } else { "PM" };
                match time.second {
                    Some(second) => format!("{hour:02}:{:02}:{second:02} {period}", time.minute),
                    None => format!("{hour:02}:{:02} {period}", time.minute),
                }
            }
        }
    }

    fn format_endpoint(&self, value: DateTimeSelection, endpoint: DateTimeEndpoint) -> String {
        match (value.date(endpoint), value.time(endpoint)) {
            (Some(date), Some(time)) => format!("{} {}", date.to_iso8601(), self.format_time(time)),
            (Some(date), None) => date.to_iso8601(),
            (None, Some(time)) => self.format_time(time),
            _ => String::new(),
        }
    }

    fn format_selection(&self, value: DateTimeSelection) -> String {
        let start = self.format_endpoint(value, DateTimeEndpoint::Start);
        if value.mode().is_range() {
            format!(
                "{start} – {}",
                self.format_endpoint(value, DateTimeEndpoint::End)
            )
        } else {
            start
        }
    }

    fn trigger_label(&self, unavailable: bool) -> String {
        let mut label = self.format_selection(self.value());
        if unavailable {
            label.push_str(" · unavailable");
        }
        if self.working() != self.value() {
            label.push_str(" · draft");
        }
        label
    }

    fn select_endpoint(&mut self, endpoint: DateTimeEndpoint) {
        if endpoint == DateTimeEndpoint::End && !self.mode().is_range() {
            return;
        }
        self.endpoint = endpoint;
        if let Some(date) = self.working().date(endpoint) {
            let date = self
                .availability
                .bounds
                .map_or(date, |bounds| bounds.view_date(date));
            let first_dow = self.calendar.map_or(0, |calendar| calendar.first_dow);
            self.calendar = Some(CalendarView::new(date, first_dow));
            self.cursor = Some(date);
        }
        for scroll in &mut self.scroll {
            scroll.value = None;
        }
        self.refresh_minute_projection();
    }

    fn constrain_calendar_view(&mut self) {
        if let Some(calendar) = self.calendar {
            if let Some(date) = DateValue::new(
                calendar.year,
                calendar.month,
                self.cursor.map_or(1, |date| date.day).min(
                    crate::model::date_value::days_in_month(calendar.year, calendar.month),
                ),
            ) {
                let date = self
                    .availability
                    .bounds
                    .map_or(date, |bounds| bounds.view_date(date));
                self.calendar = Some(CalendarView::new(date, calendar.first_dow));
                self.cursor = Some(date);
            }
        }
    }

    fn time(&self) -> Option<TimeValue> {
        self.working().time(self.endpoint)
    }

    fn hours(&self) -> Vec<u8> {
        let current = self.time().map_or(0, |time| time.hour);
        let mut values = match self.display_mode {
            TimeDisplayMode::H24 => (0..24)
                .filter(|hour| hour % self.hour_step == 0)
                .collect::<Vec<_>>(),
            TimeDisplayMode::H12 => {
                let period = if current >= 12 { 12 } else { 0 };
                (1..=12)
                    .map(|hour| hour % 12 + period)
                    .filter(|hour| hour % self.hour_step == 0)
                    .collect()
            }
        };
        if !values.contains(&current) {
            values.push(current);
        }
        match self.display_mode {
            TimeDisplayMode::H24 => values.sort_unstable(),
            TimeDisplayMode::H12 => values.sort_unstable_by_key(|hour| (hour % 12 + 11) % 12),
        }
        values
    }

    fn minutes(&self) -> Vec<u8> {
        let current = self.time().map_or(0, |time| time.minute);
        let mut values = (0..60)
            .filter(|minute| minute % self.minute_step == 0)
            .collect::<Vec<_>>();
        if !values.contains(&current) {
            values.push(current);
            values.sort_unstable();
        }
        values
    }

    fn time_targets(&self, kind: Column) -> Vec<Target> {
        if kind == Column::Minute {
            if let Some(projection) = &self.minute_projection {
                if self.minute_projection_basis() == Some(projection.basis) {
                    return projection.targets.clone();
                }
            }
        }
        self.current_time_targets(kind)
    }

    fn minute_projection_basis(&self) -> Option<MinuteProjectionBasis> {
        Some(MinuteProjectionBasis {
            date: self
                .working()
                .date(self.endpoint)
                .or(self.availability.reference_date),
            hour: self.time()?.hour,
            endpoint: self.endpoint,
        })
    }

    fn refresh_minute_projection(&mut self) {
        if !self.popup_open {
            return;
        }
        let Some(basis) = self.minute_projection_basis() else {
            self.minute_projection = None;
            return;
        };
        if self
            .minute_projection
            .as_ref()
            .is_some_and(|projection| projection.basis == basis)
        {
            return;
        }
        // Freeze membership, not selection or availability authority. Minute-only
        // edits and Capture dismissal keep it; a new date/hour/endpoint requalifies it.
        self.minute_projection = Some(MinuteProjection {
            basis,
            targets: self.current_time_targets(Column::Minute),
        });
        self.scroll[Column::Minute.scroll_index().unwrap()].value = None;
        if matches!(self.hovered, Some(Target::Minute(_))) {
            self.hovered = None;
        }
    }

    fn current_time_targets(&self, kind: Column) -> Vec<Target> {
        let mut targets = match kind {
            Column::Hour => self
                .hours()
                .into_iter()
                .map(Target::Hour)
                .collect::<Vec<_>>(),
            Column::Minute => self.minutes().into_iter().map(Target::Minute).collect(),
            Column::Period => vec![Target::Period(false), Target::Period(true)],
            _ => return Vec::new(),
        };
        if self.unavailable_time_slots == UnavailableTimeSlotVisibility::Hide {
            targets.retain(|target| self.selected(*target) || self.target_editable(*target));
        }
        targets
    }

    fn time_label(&self, target: Target) -> String {
        match target {
            Target::Hour(hour) => {
                let hour = match self.display_mode {
                    TimeDisplayMode::H24 => hour,
                    TimeDisplayMode::H12 => {
                        if hour % 12 == 0 {
                            12
                        } else {
                            hour % 12
                        }
                    }
                };
                format!("{hour:02}")
            }
            Target::Minute(minute) => format!("{minute:02}"),
            Target::Period(pm) => if pm { "PM" } else { "AM" }.into(),
            _ => unreachable!("only time targets have time-column labels"),
        }
    }

    fn year_bounds(&self) -> (i32, i32) {
        if let Some(bounds) = self.availability.bounds {
            return (bounds.min.year, bounds.max.year);
        }
        // The normal civil picker span is 0001..9999. Preserve an explicit
        // existing proleptic year rather than replacing or rounding its value.
        let year = self.calendar.map_or(1, |calendar| calendar.year);
        (1.min(year), 9999.max(year))
    }

    fn row_count(&self, kind: Column) -> usize {
        match kind {
            Column::Year => {
                let (min, max) = self.year_bounds();
                (max as i64 - min as i64 + 1) as usize
            }
            Column::Month => 12,
            Column::Hour | Column::Minute | Column::Period => self.time_targets(kind).len(),
            Column::Calendar => 0,
        }
    }

    fn row(&self, kind: Column, index: usize) -> (String, Target) {
        match kind {
            Column::Year => {
                let year = (self.year_bounds().0 as i64 + index as i64) as i32;
                (format!("{year:04}"), Target::Year(year))
            }
            Column::Month => (MONTHS[index].into(), Target::Month(index as u8 + 1)),
            Column::Hour | Column::Minute | Column::Period => {
                let target = self.time_targets(kind)[index];
                (self.time_label(target), target)
            }
            Column::Calendar => unreachable!("Calendar has fixed cells, not list rows"),
        }
    }

    fn selected_index(&self, kind: Column) -> usize {
        match kind {
            Column::Year => self.calendar.map_or(0, |calendar| {
                (calendar.year as i64 - self.year_bounds().0 as i64) as usize
            }),
            Column::Month => self
                .calendar
                .map_or(0, |calendar| calendar.month as usize - 1),
            Column::Hour | Column::Minute | Column::Period => self
                .time_targets(kind)
                .iter()
                .position(|target| self.selected(*target))
                .unwrap_or(0),
            Column::Calendar => 0,
        }
    }

    fn candidate(&self, target: Target) -> Result<DateTimeSelection, DateTimeSelectionError> {
        match target {
            Target::Day(date) => self.working().with_date(self.endpoint, date),
            Target::Hour(hour) => {
                let time = self.time().ok_or(DateTimeSelectionError::ModeMismatch)?;
                self.working()
                    .with_time(self.endpoint, TimeValue { hour, ..time })
            }
            Target::Minute(minute) => {
                let time = self.time().ok_or(DateTimeSelectionError::ModeMismatch)?;
                self.working()
                    .with_time(self.endpoint, TimeValue { minute, ..time })
            }
            Target::Period(pm) => {
                let time = self.time().ok_or(DateTimeSelectionError::ModeMismatch)?;
                self.working().with_time(
                    self.endpoint,
                    TimeValue {
                        hour: time.hour % 12 + if pm { 12 } else { 0 },
                        ..time
                    },
                )
            }
            _ => Ok(self.working()),
        }
    }

    fn target_available(&self, target: Target) -> bool {
        match target {
            Target::Apply => self.working_availability().is_ok(),
            Target::Year(year) => {
                let (min, max) = self.year_bounds();
                min <= year && year <= max
            }
            Target::Month(month) => self.calendar.is_some_and(|calendar| {
                self.availability
                    .bounds
                    .is_none_or(|bounds| bounds.month_allowed(calendar.year, month))
            }),
            Target::Day(_) | Target::Hour(_) | Target::Minute(_) | Target::Period(_) => {
                self.candidate(target).is_ok_and(|value| {
                    if self.availability.bounds.is_some_and(|bounds| {
                        value
                            .date(self.endpoint)
                            .is_some_and(|date| !bounds.contains(date))
                    }) {
                        return false;
                    }
                    // Independent boundary edits may leave an invalid draft.
                    // Only Apply admits the complete interval.
                    self.mode().is_range() || self.availability.point(value, self.endpoint).is_ok()
                })
            }
            _ => true,
        }
    }

    fn target_editable(&self, target: Target) -> bool {
        if self.target_available(target) {
            return true;
        }
        if self.display_mode != TimeDisplayMode::H12
            || self.mode().is_range()
            || !matches!(target, Target::Hour(_) | Target::Period(_))
        {
            return false;
        }
        // Hour and AM/PM are independent draft components, not complete-point
        // commits. Requiring point availability here can make the other period
        // unreachable. Keep civil bounds and final Apply admission unchanged.
        self.candidate(target).is_ok_and(|value| {
            self.availability.bounds.is_none_or(|bounds| {
                value
                    .date(self.endpoint)
                    .is_none_or(|date| bounds.contains(date))
            })
        })
    }

    fn selected(&self, target: Target) -> bool {
        match target {
            Target::Year(year) => self.calendar.is_some_and(|calendar| calendar.year == year),
            Target::Month(month) => self
                .calendar
                .is_some_and(|calendar| calendar.month == month),
            Target::Day(date) => self.working().date(self.endpoint) == Some(date),
            Target::Hour(hour) => self.time().is_some_and(|time| time.hour == hour),
            Target::Minute(minute) => self.time().is_some_and(|time| time.minute == minute),
            Target::Period(pm) => self.time().is_some_and(|time| (time.hour >= 12) == pm),
            Target::Endpoint(endpoint) => self.endpoint == endpoint,
            _ => false,
        }
    }

    fn day_in_range(&self, date: DateValue) -> bool {
        match self.working() {
            DateTimeSelection::DateRange { start, end } => start <= date && date <= end,
            DateTimeSelection::DateTimeRange { start, end } => {
                start.date <= date
                    && (date < end.date
                        || (date == end.date
                            && (end.time.hour > 0
                                || end.time.minute > 0
                                || end.time.second.unwrap_or(0) > 0)))
            }
            _ => false,
        }
    }

    fn hit(&self, x: f32, y: f32) -> Option<Target> {
        let geometry = &self.geometry;
        if geometry.apply.contains_point(x, y) {
            return Some(Target::Apply);
        }
        if geometry.cancel.contains_point(x, y) {
            return Some(Target::Cancel);
        }
        if self.mode().is_range() {
            if geometry.start.contains_point(x, y) {
                return Some(Target::Endpoint(DateTimeEndpoint::Start));
            }
            if geometry.end.contains_point(x, y) {
                return Some(Target::Endpoint(DateTimeEndpoint::End));
            }
        }
        for column in &geometry.columns {
            if column.body.contains_point(x, y) {
                for row in &column.rows {
                    if row.rect.contains_point(x, y) {
                        return Some(row.target);
                    }
                }
            }
        }
        geometry
            .cells
            .iter()
            .find(|(_, rect)| rect.contains_point(x, y))
            .map(|(date, _)| Target::Day(*date))
    }

    fn choose(&mut self, target: Target, cx: &mut EventContext) {
        if !self.target_editable(target) {
            return;
        }
        if matches!(target, Target::Minute(_)) {
            if let Some(column) = self
                .geometry
                .columns
                .iter()
                .find(|column| column.kind == Column::Minute)
            {
                // A click changes the selected value, not the list's scroll position.
                self.scroll[Column::Minute.scroll_index().unwrap()].value = Some(column.scroll);
            }
        }
        match target {
            Target::Year(year) => {
                if let Some(calendar) = &mut self.calendar {
                    calendar.year = year;
                    self.cursor = DateValue::new(
                        year,
                        calendar.month,
                        self.cursor.map_or(1, |date| date.day).min(
                            crate::model::date_value::days_in_month(year, calendar.month),
                        ),
                    );
                }
                self.constrain_calendar_view();
                self.column = Column::Year;
            }
            Target::Month(month) => {
                if let Some(calendar) = &mut self.calendar {
                    calendar.month = month;
                    self.cursor = DateValue::new(
                        calendar.year,
                        month,
                        self.cursor.map_or(1, |date| date.day).min(
                            crate::model::date_value::days_in_month(calendar.year, month),
                        ),
                    );
                }
                self.constrain_calendar_view();
                self.column = Column::Month;
            }
            Target::Endpoint(endpoint) => self.select_endpoint(endpoint),
            Target::Apply => {
                let availability = &self.availability;
                if let Ok(changed) = self
                    .state
                    .commit(|value| availability.selection(value).is_ok())
                {
                    self.minute_projection = None;
                    if let (Some(value), Some(callback)) = (changed, &mut self.on_commit) {
                        callback(value);
                    }
                    self.popup_open = false;
                    cx.release_keyboard_capture();
                }
            }
            Target::Cancel => {
                self.state.cancel();
                self.minute_projection = None;
                self.popup_open = false;
                self.select_endpoint(self.endpoint);
                cx.release_keyboard_capture();
            }
            _ => {
                if let Ok(candidate) = self.candidate(target) {
                    if self.state.set_working(candidate).is_ok() {
                        self.column = match target {
                            Target::Day(_) => Column::Calendar,
                            Target::Hour(_) => Column::Hour,
                            Target::Minute(_) => Column::Minute,
                            _ => Column::Period,
                        };
                        if let Target::Day(date) = target {
                            self.cursor = Some(date);
                            if let Some(calendar) = &mut self.calendar {
                                calendar.year = date.year;
                                calendar.month = date.month;
                            }
                        }
                    }
                }
            }
        }
        self.refresh_minute_projection();
        cx.invalidate_layout();
    }

    fn scroll_column(&mut self, kind: Column, delta: f32, cx: &mut EventContext) {
        let Some(index) = kind.scroll_index() else {
            return;
        };
        let Some(column) = self
            .geometry
            .columns
            .iter()
            .find(|column| column.kind == kind)
        else {
            return;
        };
        let row = self.geometry.row_height;
        if !delta.is_finite() || delta == 0.0 || !row.is_finite() || row <= 0.0 {
            return;
        }
        let max = (column.count as f32 * row - column.body.height).max(0.0);
        // Routed deltas already contain global pixel/DPI normalization and may
        // coalesce several native inputs. Cap each intent at one item while
        // preserving sub-row pixel precision; do not infer native notch counts.
        let current = self.scroll[index].value.unwrap_or(column.scroll);
        let next = (current + delta.clamp(-row, row)).clamp(0.0, max);
        if next != current {
            self.scroll[index].value = Some(next);
            cx.invalidate_layout();
        }
    }

    fn keyboard_step(&mut self, delta: i32, cx: &mut EventContext) {
        if self.column == Column::Calendar {
            if let Some(mut date) = self.cursor.or(self.working().date(self.endpoint)) {
                for _ in 0..delta.unsigned_abs() {
                    let next = if delta < 0 { date.pred() } else { date.succ() };
                    let Some(next) = next else {
                        return;
                    };
                    date = next;
                }
                self.choose(Target::Day(date), cx);
            }
            return;
        }
        let targets = self.time_targets(self.column);
        let time_column = matches!(self.column, Column::Hour | Column::Minute | Column::Period);
        let count = if time_column {
            targets.len()
        } else {
            self.row_count(self.column)
        };
        let start = if time_column {
            targets
                .iter()
                .position(|target| self.selected(*target))
                .unwrap_or(0)
        } else {
            self.selected_index(self.column)
        } as i64;
        for offset in 1..=count {
            let index = start + offset as i64 * delta.signum() as i64;
            if index < 0 || index >= count as i64 {
                break;
            }
            let target = if time_column {
                targets[index as usize]
            } else {
                self.row(self.column, index as usize).1
            };
            if self.target_editable(target) {
                self.choose(target, cx);
                if let Some(scroll) = self.column.scroll_index() {
                    self.scroll[scroll].value = None;
                }
                break;
            }
        }
    }

    fn paint_label(
        cx: &mut PaintContext,
        label: &str,
        rect: RectF,
        color: Color,
        centered: bool,
        z: u32,
    ) {
        let run = cx.text_engine.shape(label, cx.theme.typography.font_size);
        let y = rect.y
            + compute_control_text_baseline_y(
                rect.height,
                run.ascent,
                run.descent,
                cx.theme.typography.baseline_offset,
            )
            - run.ascent;
        let x = if centered {
            rect.x + (rect.width - run.width) * 0.5
        } else {
            rect.x
        };
        cx.scene
            .push_text_with_backgrounds(&run, x, y, color, vec![], z, None);
    }

    fn popup_placement(&self, cx: &PaintContext, rect: RectF) -> PopupPlacement {
        let presentation = cx.own_presentation_geometry();
        let (scroll, bounds) = if let Some(presentation) = presentation {
            (presentation.cumulative_scroll, presentation.clip)
        } else {
            let (width, height) = cx.viewport_size();
            (cx.cumulative_scroll_offset_for_paint(), RectF::new(0.0, 0.0, width, height))
        };
        let mut placement = PopupPlacement::new(rect, scroll, bounds, self.geometry.size);
        placement.presentation = presentation;
        if let Some(presentation) = presentation {
            let painted_popup = placement.chrome_geometry(rect, cx.scale_factor).popup
                .translate(-presentation.cumulative_scroll.0, -presentation.cumulative_scroll.1);
            placement.visible_window = placement.visible_window.intersection(&painted_popup)
                .unwrap_or(RectF::zero());
        } else {
            // Rounded, interpolated and unbound contexts retain the existing
            // root-only behavior; they do not qualify rectangular containment.
            placement.visible_window = placement.window;
        }
        placement
    }

    pub(crate) fn popup_projection_matches(
        &self,
        presentation: Option<OwnedWidgetPresentationGeometry>,
    ) -> bool {
        !self.popup_open || self.popup_projection.get().presentation == presentation
    }

    fn translated(rect: RectF, origin: RectF) -> RectF {
        RectF::new(
            rect.x + origin.x,
            rect.y + origin.y,
            rect.width,
            rect.height,
        )
    }
}

impl WidgetDowncast for DateTimePicker {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for DateTimePicker {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let theme = cx.theme;
        let padding = theme.geometry.time_picker.popup_padding;
        let row_height = theme
            .list_geom
            .row_height_min
            .max(theme.typography.line_height);
        let control_height = theme.geometry.text_field.height;
        let caption_height = theme.typography.line_height;
        // All value rows share a baseline with the six calendar day rows.
        let weekday_height = if self.mode().has_date() {
            caption_height
        } else {
            0.0
        };
        let body_height = weekday_height + row_height * 6.0;
        let range_height = if self.mode().is_range() {
            control_height + padding
        } else {
            0.0
        };
        let body_y = padding + range_height + caption_height;
        let scrollbar_width = self.scroll[0].bar.config.thickness;
        let number_width = cx.text_engine.measure("0000", theme.typography.font_size).0
            + padding * 2.0
            + scrollbar_width;
        let month_width = cx
            .text_engine
            .measure("September", theme.typography.font_size)
            .0
            + padding * 2.0
            + scrollbar_width;
        let time_width = cx
            .text_engine
            .measure("Hour", theme.typography.font_size)
            .0
            .max(cx.text_engine.measure("00", theme.typography.font_size).0)
            + padding * 2.0
            + scrollbar_width;
        let period_width = cx
            .text_engine
            .measure("AM/PM", theme.typography.font_size)
            .0
            + padding * 2.0;
        let calendar_width = control_height * 7.0;
        let mut geometry = PickerGeometry {
            row_height,
            ..PickerGeometry::default()
        };
        let mut x = padding;
        for (kind, width) in [
            (Column::Year, number_width),
            (Column::Month, month_width),
            (Column::Calendar, calendar_width),
            (Column::Hour, time_width),
            (Column::Minute, time_width),
            (Column::Period, period_width),
        ] {
            let included = match kind {
                Column::Year | Column::Month | Column::Calendar => self.mode().has_date(),
                Column::Hour | Column::Minute => self.mode().has_time(),
                Column::Period => {
                    self.mode().has_time() && self.display_mode == TimeDisplayMode::H12
                }
            };
            if !included {
                continue;
            }
            let header = RectF::new(x, body_y - caption_height, width, caption_height);
            let list_inset = if kind == Column::Calendar {
                0.0
            } else {
                weekday_height
            };
            let body = RectF::new(x, body_y + list_inset, width, body_height - list_inset);
            let targets = self.time_targets(kind);
            let time_column = matches!(kind, Column::Hour | Column::Minute | Column::Period);
            let count = if time_column {
                targets.len()
            } else {
                self.row_count(kind)
            };
            let mut rows = Vec::new();
            let mut scroll = 0.0;
            if let Some(scroll_index) = kind.scroll_index() {
                let max = (count as f32 * row_height - body.height).max(0.0);
                let selected = if time_column {
                    targets
                        .iter()
                        .position(|target| self.selected(*target))
                        .unwrap_or(0)
                } else {
                    self.selected_index(kind)
                };
                let centered = selected.saturating_sub(3) as f32 * row_height;
                scroll = self.scroll[scroll_index]
                    .value
                    .unwrap_or(centered)
                    .clamp(0.0, max);
                let first = (scroll / row_height).floor() as usize;
                let last = (first + (body.height / row_height).ceil() as usize + 1).min(count);
                let gutter = if count as f32 * row_height > body.height {
                    scrollbar_width
                } else {
                    0.0
                };
                for index in first..last {
                    let (label, target) = if time_column {
                        let target = targets[index];
                        (self.time_label(target), target)
                    } else {
                        self.row(kind, index)
                    };
                    rows.push(ListRow {
                        rect: RectF::new(
                            x + padding * 0.5,
                            body.y + index as f32 * row_height - scroll,
                            (width - padding - gutter).max(0.0),
                            row_height,
                        ),
                        label,
                        target,
                    });
                }
            } else if let Some(calendar) = self.calendar {
                let cell_width = width / 7.0;
                for (index, date) in calendar.cells().into_iter().enumerate() {
                    if let Some(date) = date {
                        geometry.cells.push((
                            date,
                            RectF::new(
                                x + (index % 7) as f32 * cell_width,
                                body_y + caption_height + (index / 7) as f32 * row_height,
                                cell_width,
                                row_height,
                            ),
                        ));
                    }
                }
            }
            geometry.columns.push(ColumnGeometry {
                kind,
                header,
                body,
                rows,
                count,
                scroll,
            });
            x += width + padding;
        }
        let button_width = cx
            .text_engine
            .measure("Cancel", theme.typography.font_size)
            .0
            .max(
                cx.text_engine
                    .measure("Apply", theme.typography.font_size)
                    .0,
            )
            + padding * 2.0;
        let status_width = cx
            .text_engine
            .measure("Selection is unavailable", theme.typography.font_size)
            .0
            + padding * 2.0;
        let width = x.max(button_width * 2.0 + padding * 3.0).max(status_width);
        let shift = (width - x) * 0.5;
        for column in &mut geometry.columns {
            column.header.x += shift;
            column.body.x += shift;
            for row in &mut column.rows {
                row.rect.x += shift;
            }
        }
        for (_, cell) in &mut geometry.cells {
            cell.x += shift;
        }
        let footer_y = body_y + body_height + padding;
        geometry.footer_top = footer_y;
        let inline_status_width = width - button_width * 2.0 - padding * 4.0;
        let inline_status = inline_status_width >= status_width;
        geometry.status = RectF::new(
            padding,
            footer_y + padding,
            if inline_status {
                inline_status_width
            } else {
                width - padding * 2.0
            },
            if inline_status {
                control_height
            } else {
                caption_height
            },
        );
        let button_y = footer_y + padding + if inline_status { 0.0 } else { caption_height };
        geometry.apply = RectF::new(
            width - padding - button_width,
            button_y,
            button_width,
            control_height,
        );
        geometry.cancel = RectF::new(
            geometry.apply.x - padding - button_width,
            button_y,
            button_width,
            control_height,
        );
        if self.mode().is_range() {
            let endpoint_width = (width - padding * 3.0) * 0.5;
            geometry.start = RectF::new(padding, padding, endpoint_width, control_height);
            geometry.end = RectF::new(
                padding * 2.0 + endpoint_width,
                padding,
                endpoint_width,
                control_height,
            );
        }
        geometry.size = Size::new(width, button_y + control_height + padding);
        self.geometry = geometry;
        let summary = self.trigger_label(self.availability.selection(self.value()).is_err());
        let measured = cx
            .text_engine
            .measure(&summary, theme.typography.input_font_size)
            .0
            + theme.geometry.text_field.padding_x * 2.0
            + theme.geometry.combo_box.affordance_width;
        let desired = self.width.unwrap_or(measured.max(
            theme.geometry.time_picker.column_width * 2.0
                + theme.geometry.combo_box.affordance_width,
        ));
        LayoutResult::fit(Size::new(
            desired
                .max(constraints.min_width)
                .min(constraints.max_width),
            control_height
                .max(constraints.min_height)
                .min(constraints.max_height),
        ))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let theme = cx.theme;
        let state = if cx.has_capture() {
            ControlChromeState::Captured
        } else if cx.has_focus_cue() {
            ControlChromeState::Focused
        } else {
            ControlChromeState::Idle
        };
        let style = theme.input_chrome(state);
        let radius = theme.chrome_radius(
            ControlChromeCategory::RecessedEditable,
            theme.geometry.text_field.radius,
            false,
        );
        if self.popup_open {
            let placement = self.popup_placement(cx, rect);
            let geometry = placement.chrome_geometry(rect, cx.scale_factor);
            let edge = cx.one_px();
            let border = theme.input_chrome(ControlChromeState::Idle).chrome.border_color;
            cx.scene.begin_clip(geometry.control, None);
            cx.scene.push_path(
                geometry.fill_outline(radius, edge),
                style.chrome.fill.unwrap_or(theme.colors.surfaces.content),
                LAYER_CONTENT,
                cx.scene_key,
            );
            cx.scene.push_stroked_path(
                geometry.outline(radius, edge),
                StrokeParams {
                    width: edge.min(geometry.popup.height),
                    ..Default::default()
                },
                border,
                LAYER_CONTENT + 1,
                None,
            );
            if let Some(divider) = geometry.input_divider(edge, cx.one_px()) {
                cx.scene.push_rect(
                    divider,
                    theme.input_chrome(ControlChromeState::Idle).chrome.border_color,
                    LAYER_CONTENT + 2,
                    None,
                );
            }
            cx.scene.end_clip();
            if matches!(state, ControlChromeState::Focused | ControlChromeState::Captured) {
                let width = if cx.has_capture() {
                    theme.focus_ring.captured_width
                } else {
                    theme.focus_ring.focused_width
                };
                let scale = cx.scale_factor;
                let height = snap_to_physical(width, scale).max(edge).min(rect.height - edge);
                let left = snap_to_physical(rect.x + theme.geometry.text_field.padding_x, scale);
                let right = snap_to_physical(rect.right() - theme.geometry.text_field.padding_x, scale);
                let bottom = snap_to_physical(rect.bottom() - edge, scale);
                let underline = RectF::new(left, bottom - height, right - left, height);
                if underline.width > 0.0
                    && underline.height > 0.0
                    && [
                        underline.x,
                        underline.y,
                        underline.width,
                        underline.height,
                        underline.right(),
                        underline.bottom(),
                    ]
                    .into_iter()
                    .all(f32::is_finite)
                {
                    cx.scene.push_rect(
                        underline,
                        theme.colors.accents.primary,
                        LAYER_CONTENT + 3,
                        None,
                    );
                }
            }
        } else {
            cx.scene.push_rounded_rect(
                rect,
                style.chrome.fill.unwrap_or(theme.colors.surfaces.content),
                RoundedCorners::uniform(radius),
                LAYER_CONTENT,
                cx.scene_key,
            );
            cx.scene.push_border_rounded_rect(
                rect,
                style.chrome.border_color,
                style.chrome.border_width,
                RoundedCorners::uniform(radius),
                LAYER_CONTENT + 1,
                None,
            );
        }
        let affordance_width = theme.geometry.combo_box.affordance_width;
        let padding = theme.geometry.text_field.padding_x;
        let text_rect = RectF::new(
            rect.x + padding,
            rect.y,
            (rect.width - padding * 2.0 - affordance_width).max(0.0),
            rect.height,
        );
        let unavailable = self.availability.selection(self.value()).is_err();
        let label = self.trigger_label(unavailable);
        cx.scene.begin_clip(text_rect, None);
        Self::paint_label(
            cx,
            &label,
            text_rect,
            if unavailable {
                theme.colors.typography.muted
            } else {
                theme.colors.typography.primary
            },
            false,
            LAYER_CONTENT + 2,
        );
        cx.scene.end_clip();
        if let Some(icon) = cx.icon_registry.get(if self.mode().has_date() {
            IconId::CALENDAR
        } else {
            IconId::CHEVRON_DOWN
        }) {
            let size = theme
                .geometry
                .text_field
                .icon_size
                .min(rect.height - padding);
            cx.scene.push_icon(
                icon,
                rect.right() - affordance_width + (affordance_width - size) * 0.5,
                rect.y + (rect.height - size) * 0.5,
                size,
                theme.colors.typography.secondary,
                LAYER_CONTENT + 2,
            );
        }
    }

    fn paint_overlay(&self, cx: &mut PaintContext, rect: RectF, owner: TreeWidgetId) {
        if !self.popup_open {
            return;
        }
        let (viewport_width, viewport_height) = cx.viewport_size();
        let placement = self.popup_placement(cx, rect);
        let popup = placement.paint;
        self.popup_projection.set(placement);
        cx.register_overlay_hit_region(OverlayHitRegion {
            abs_rect: RectF::new(0.0, 0.0, viewport_width, viewport_height),
            owner_abs_rect: placement.anchor_window,
            widget_id: owner,
        });
        if placement.visible_window.width > 0.0 && placement.visible_window.height > 0.0 {
            cx.register_overlay_hit_region(OverlayHitRegion {
                abs_rect: placement.visible_window,
                owner_abs_rect: placement.anchor_window,
                widget_id: owner,
            });
        }
        let theme = cx.theme;
        let style = theme.input_chrome(if cx.has_capture() {
            ControlChromeState::Captured
        } else {
            ControlChromeState::Idle
        });
        let radius = theme.chrome_radius(
            ControlChromeCategory::RecessedEditable,
            theme.geometry.combo_box.radius,
            false,
        );
        let chrome_style = theme.input_chrome(if cx.has_capture() {
            ControlChromeState::Captured
        } else if cx.has_focus_cue() {
            ControlChromeState::Focused
        } else {
            ControlChromeState::Idle
        });
        let chrome_radius = theme.chrome_radius(
            ControlChromeCategory::RecessedEditable,
            theme.geometry.text_field.radius,
            false,
        );
        let edge = cx.one_px();
        let geometry = placement.chrome_geometry(rect, cx.scale_factor);
        let corners = geometry.corners(geometry.popup, chrome_radius, edge);
        // A separate popup shadow would cross the joined attachment. The
        // existing overlay scope carries only this part of the shared shell.
        cx.scene.begin_clip(geometry.popup, Some(corners));
        cx.scene.push_path(
            geometry.fill_outline(chrome_radius, edge),
            chrome_style.chrome.fill.unwrap_or(theme.colors.surfaces.content),
            LAYER_OVERLAY + 1,
            None,
        );
        let fill_z = LAYER_OVERLAY + 2;
        let text_z = LAYER_OVERLAY + 4;
        let padding = theme.geometry.time_picker.popup_padding;
        for pair in self.geometry.columns.windows(2) {
            let x = (pair[0].body.right() + pair[1].body.x) * 0.5;
            cx.scene.push_rect(
                RectF::new(
                    popup.x + x,
                    popup.y + pair[0].header.y,
                    cx.one_px(),
                    self.geometry.footer_top - pair[0].header.y - padding,
                ),
                theme.colors.borders.subtle,
                fill_z + 1,
                None,
            );
        }
        cx.scene.push_rect(
            RectF::new(
                popup.x + padding,
                popup.y + self.geometry.footer_top,
                (popup.width - padding * 2.0).max(0.0),
                cx.one_px(),
            ),
            theme.colors.borders.subtle,
            fill_z + 1,
            None,
        );
        if self.mode().is_range() {
            for (endpoint, local) in [
                (DateTimeEndpoint::Start, self.geometry.start),
                (DateTimeEndpoint::End, self.geometry.end),
            ] {
                let target = Target::Endpoint(endpoint);
                let cell = Self::translated(local, popup);
                let fill = if self.selected(target) {
                    style.affordance
                } else {
                    style.browsing
                };
                cx.scene.push_rounded_rect(
                    cell,
                    fill,
                    RoundedCorners::uniform(radius),
                    fill_z,
                    None,
                );
                if self.selected(target) {
                    cx.scene.push_border_rounded_rect(
                        cell,
                        style.chrome.border_color,
                        cx.one_px(),
                        RoundedCorners::uniform(radius),
                        fill_z + 1,
                        None,
                    );
                }
                let prefix = if endpoint == DateTimeEndpoint::Start {
                    "Start"
                } else {
                    "End"
                };
                let label = format!(
                    "{prefix}  {}",
                    self.format_endpoint(self.working(), endpoint)
                );
                cx.scene.begin_clip(cell, None);
                Self::paint_label(
                    cx,
                    &label,
                    cell,
                    theme.colors.typography.primary,
                    true,
                    text_z,
                );
                cx.scene.end_clip();
            }
        }
        for column in &self.geometry.columns {
            let header = Self::translated(column.header, popup);
            let active = self.column == column.kind
                && cx.appearance_facts().is_some_and(|facts| {
                    facts.focused()
                        && facts.keyboard_captured()
                        && facts.window_active()
                        && facts.interaction_domain_active()
                });
            let title = match column.kind {
                Column::Year => "Year".into(),
                Column::Month => "Month".into(),
                Column::Calendar => self.calendar.map_or(String::new(), |calendar| {
                    format!(
                        "{} {}",
                        MONTHS[(calendar.month - 1) as usize],
                        calendar.year
                    )
                }),
                Column::Hour => "Hour".into(),
                Column::Minute => "Min".into(),
                Column::Period => "AM/PM".into(),
            };
            cx.scene.begin_clip(header, None);
            Self::paint_label(
                cx,
                &title,
                header,
                if active {
                    style.chrome.border_color
                } else {
                    theme.colors.typography.secondary
                },
                true,
                text_z,
            );
            if active {
                cx.scene.push_rect(
                    RectF::new(
                        header.x,
                        header.bottom() - cx.one_px(),
                        header.width,
                        cx.one_px(),
                    ),
                    style.chrome.border_color,
                    fill_z + 1,
                    None,
                );
            }
            cx.scene.end_clip();
            let body = Self::translated(column.body, popup);
            cx.scene.begin_clip(body, None);
            if column.kind == Column::Calendar {
                let first_dow = self.calendar.map_or(0, |calendar| calendar.first_dow);
                for index in 0..7 {
                    let day_rect = RectF::new(
                        body.x + index as f32 * body.width / 7.0,
                        body.y,
                        body.width / 7.0,
                        column.header.height,
                    );
                    Self::paint_label(
                        cx,
                        WEEKDAYS[(index + first_dow as usize) % 7],
                        day_rect,
                        theme.colors.typography.muted,
                        true,
                        text_z,
                    );
                }
                for (date, local) in &self.geometry.cells {
                    let cell = Self::translated(*local, popup);
                    let target = Target::Day(*date);
                    let selected = self.working().date(DateTimeEndpoint::Start) == Some(*date)
                        || (self.mode().is_range()
                            && self.working().date(DateTimeEndpoint::End) == Some(*date));
                    let available = self.target_available(target);
                    let hovered = self.hovered == Some(target) && available;
                    if selected || self.day_in_range(*date) || hovered {
                        let fill = if selected {
                            style.affordance
                        } else if hovered {
                            theme.colors.list.row_hover
                        } else {
                            style.browsing
                        };
                        cx.scene.push_rounded_rect(
                            cell,
                            fill,
                            RoundedCorners::uniform(radius),
                            fill_z,
                            None,
                        );
                    }
                    if cx.has_capture()
                        && self.column == Column::Calendar
                        && self.cursor == Some(*date)
                    {
                        cx.scene.push_border_rounded_rect(
                            cell,
                            theme.colors.borders.strong,
                            cx.one_px(),
                            RoundedCorners::uniform(radius),
                            fill_z + 1,
                            None,
                        );
                    }
                    Self::paint_label(
                        cx,
                        &date.day.to_string(),
                        cell,
                        if available {
                            theme.colors.typography.primary
                        } else {
                            theme.colors.typography.muted
                        },
                        true,
                        text_z,
                    );
                    if !available {
                        cx.scene.push_rect(
                            RectF::new(
                                cell.x + cell.width * 0.25,
                                cell.y + cell.height * 0.5,
                                cell.width * 0.5,
                                cx.one_px(),
                            ),
                            theme.colors.typography.muted,
                            text_z + 1,
                            None,
                        );
                    }
                }
            } else {
                let scroll_index = column
                    .kind
                    .scroll_index()
                    .expect("list column owns a scroll region");
                let bar = &self.scroll[scroll_index].bar;
                let scrollable = bar.is_visible(
                    column.count as f32 * self.geometry.row_height,
                    column.body.height,
                );
                let content = RectF::new(
                    body.x,
                    body.y,
                    (body.width
                        - if scrollable {
                            bar.config.thickness
                        } else {
                            0.0
                        })
                    .max(0.0),
                    body.height,
                );
                cx.scene.begin_clip(content, None);
                for row in &column.rows {
                    let cell = Self::translated(row.rect, popup);
                    let available = self.target_available(row.target);
                    if self.selected(row.target)
                        || (self.hovered == Some(row.target) && self.target_editable(row.target))
                    {
                        cx.scene.push_rounded_rect(
                            cell,
                            if self.selected(row.target) {
                                style.affordance
                            } else {
                                theme.colors.list.row_hover
                            },
                            RoundedCorners::uniform(radius),
                            fill_z,
                            None,
                        );
                    }
                    let label_rect = if column.kind == Column::Month {
                        RectF::new(
                            cell.x + padding,
                            cell.y,
                            (cell.width - padding * 2.0).max(0.0),
                            cell.height,
                        )
                    } else {
                        cell
                    };
                    Self::paint_label(
                        cx,
                        &row.label,
                        label_rect,
                        if available {
                            theme.colors.typography.primary
                        } else {
                            theme.colors.typography.muted
                        },
                        column.kind != Column::Month,
                        text_z,
                    );
                }
                cx.scene.end_clip();
                if scrollable {
                    let layout = bar.layout(
                        RectF::new(0.0, 0.0, body.width, body.height),
                        body.height,
                        column.count as f32 * self.geometry.row_height,
                        body.height,
                        column.scroll,
                    );
                    cx.scene.push_rounded_rect(
                        Self::translated(layout.thumb, body),
                        bar.thumb_color(&theme.colors),
                        RoundedCorners::uniform(bar.config.thickness * 0.5),
                        fill_z + 1,
                        None,
                    );
                }
            }
            cx.scene.end_clip();
        }
        let status = match self.working_availability() {
            Ok(()) => {
                if self.working() != self.value() {
                    "Unapplied changes".into()
                } else if self.mode() == DateTimeMode::DateTimeRange {
                    "End boundary is excluded".into()
                } else if self.mode() == DateTimeMode::DateRange {
                    "Both dates are included".into()
                } else {
                    String::new()
                }
            }
            Err(error) => error.to_string(),
        };
        let status_rect = Self::translated(self.geometry.status, popup);
        cx.scene.begin_clip(status_rect, None);
        Self::paint_label(
            cx,
            &status,
            status_rect,
            theme.colors.typography.secondary,
            false,
            text_z,
        );
        cx.scene.end_clip();
        for (label, target, local) in [
            ("Cancel", Target::Cancel, self.geometry.cancel),
            ("Apply", Target::Apply, self.geometry.apply),
        ] {
            let cell = Self::translated(local, popup);
            let available = self.target_available(target);
            let fill = if self.armed == Some(target) {
                theme.colors.controls.background_pressed
            } else if self.hovered == Some(target) && available {
                theme.colors.controls.background_hover
            } else {
                style.affordance
            };
            cx.scene
                .push_rounded_rect(cell, fill, RoundedCorners::uniform(radius), fill_z, None);
            cx.scene.push_border_rounded_rect(
                cell,
                theme.colors.borders.subtle,
                cx.one_px(),
                RoundedCorners::uniform(radius),
                fill_z + 1,
                None,
            );
            Self::paint_label(
                cx,
                label,
                cell,
                if available {
                    theme.colors.typography.primary
                } else {
                    theme.colors.typography.muted
                },
                true,
                text_z,
            );
        }
        cx.scene.end_clip();
        cx.scene.begin_clip(geometry.popup, None);
        cx.scene.push_stroked_path(
            geometry.outline(chrome_radius, edge),
            StrokeParams {
                width: edge.min(geometry.popup.height),
                ..Default::default()
            },
            theme.input_chrome(ControlChromeState::Idle).chrome.border_color,
            LAYER_OVERLAY + 3,
            None,
        );
        cx.scene.end_clip();
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }
        match event {
            Event::Pointer(pointer) => {
                if let Some(kind) = self.dragging_column {
                    match pointer.kind {
                        PointerEventKind::Scroll { .. } => return true,
                        PointerEventKind::Move => {
                            if let Some(index) = kind.scroll_index() {
                                self.scroll[index].value =
                                    Some(self.scroll[index].bar.update_drag(pointer.global_pos.1));
                            }
                            cx.invalidate_layout();
                            return true;
                        }
                        PointerEventKind::Up => {
                            if let Some(index) = kind.scroll_index() {
                                self.scroll[index].bar.end_drag();
                            }
                            self.dragging_column = None;
                            cx.release_pointer();
                            return true;
                        }
                        _ => {}
                    }
                }
                if !self.popup_open {
                    if pointer.kind == PointerEventKind::Down
                        && pointer.button == Some(PointerButton::Primary)
                    {
                        cx.request_keyboard_capture();
                        return true;
                    }
                    return false;
                }
                let projection = self.popup_projection.get();
                let popup = projection.window;
                let x = pointer.global_pos.0 - popup.x;
                let y = pointer.global_pos.1 - popup.y;
                let in_popup = projection.visible_window.contains_point(pointer.global_pos.0, pointer.global_pos.1);
                match pointer.kind {
                    PointerEventKind::Move => {
                        let hovered = in_popup.then(|| self.hit(x, y)).flatten();
                        let changed = self.hovered != hovered;
                        self.hovered = hovered;
                        changed
                    }
                    PointerEventKind::Down if pointer.button == Some(PointerButton::Primary) => {
                        if !in_popup {
                            self.popup_open = false;
                            cx.release_keyboard_capture();
                            return true;
                        }
                        for column in &self.geometry.columns {
                            let Some(index) = column.kind.scroll_index() else {
                                continue;
                            };
                            let body = column.body;
                            if !body.contains_point(x, y) {
                                continue;
                            }
                            let content = column.count as f32 * self.geometry.row_height;
                            if content <= body.height {
                                continue;
                            }
                            let layout = self.scroll[index].bar.layout(
                                RectF::new(0.0, 0.0, body.width, body.height),
                                body.height,
                                content,
                                body.height,
                                column.scroll,
                            );
                            if layout.hit_rect.contains_point(x - body.x, y - body.y) {
                                let mut scroll = column.scroll;
                                if !layout.thumb.contains_point(x - body.x, y - body.y) {
                                    scroll = (scroll
                                        + if y - body.y < layout.thumb.y {
                                            -body.height
                                        } else {
                                            body.height
                                        })
                                    .clamp(0.0, content - body.height);
                                }
                                self.scroll[index].value = Some(scroll);
                                self.scroll[index].bar.begin_drag(
                                    pointer.global_pos.1,
                                    scroll,
                                    popup.y + body.y,
                                    popup.y + body.bottom(),
                                    layout.thumb.height,
                                    content - body.height,
                                );
                                self.dragging_column = Some(column.kind);
                                cx.capture_pointer(PointerButton::Primary);
                                cx.invalidate_layout();
                                return true;
                            }
                        }
                        if let Some(target) = self.hit(x, y) {
                            if matches!(target, Target::Apply | Target::Cancel) {
                                if self.target_available(target) {
                                    self.armed = Some(target);
                                    cx.capture_pointer(PointerButton::Primary);
                                }
                            } else {
                                self.choose(target, cx);
                            }
                        }
                        true
                    }
                    PointerEventKind::Up if pointer.button == Some(PointerButton::Primary) => {
                        if let Some(armed) = self.armed.take() {
                            cx.release_pointer();
                            if in_popup && self.hit(x, y) == Some(armed) {
                                self.choose(armed, cx);
                            }
                            return true;
                        }
                        in_popup
                    }
                    PointerEventKind::Scroll { delta_y, .. } if in_popup => {
                        if let Some(column) = self
                            .geometry
                            .columns
                            .iter()
                            .find(|column| column.body.contains_point(x, y))
                        {
                            let kind = column.kind;
                            if kind == Column::Calendar
                                || column.count as f32 * self.geometry.row_height
                                    <= column.body.height
                                || delta_y == 0.0
                            {
                                return false;
                            }
                            self.scroll_column(kind, -delta_y, cx);
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            }
            Event::Key(key)
                if key.is_down
                    && self.popup_open
                    && cx.keyboard_mode == KeyboardAuthorityMode::Capture =>
            {
                match key.key_code {
                    KeyCode::Enter => {
                        self.choose(Target::Apply, cx);
                        true
                    }
                    KeyCode::Tab => {
                        let Some(index) = self
                            .geometry
                            .columns
                            .iter()
                            .position(|column| column.kind == self.column)
                        else {
                            return false;
                        };
                        let next = if key.modifiers.shift {
                            index.checked_sub(1)
                        } else {
                            (index + 1 < self.geometry.columns.len()).then_some(index + 1)
                        };
                        if let Some(next) = next {
                            self.column = self.geometry.columns[next].kind;
                            true
                        } else {
                            false
                        }
                    }
                    KeyCode::Left if self.column == Column::Calendar => {
                        self.keyboard_step(-1, cx);
                        true
                    }
                    KeyCode::Right if self.column == Column::Calendar => {
                        self.keyboard_step(1, cx);
                        true
                    }
                    KeyCode::Up => {
                        self.keyboard_step(
                            if self.column == Column::Calendar {
                                -7
                            } else {
                                -1
                            },
                            cx,
                        );
                        true
                    }
                    KeyCode::Down => {
                        self.keyboard_step(
                            if self.column == Column::Calendar {
                                7
                            } else {
                                1
                            },
                            cx,
                        );
                        true
                    }
                    KeyCode::PageUp | KeyCode::PageDown if self.column == Column::Calendar => {
                        if let Some(mut calendar) = self.calendar {
                            calendar.move_month(if key.key_code == KeyCode::PageUp {
                                -1
                            } else {
                                1
                            });
                            if self.availability.bounds.is_none_or(|bounds| {
                                bounds.month_allowed(calendar.year, calendar.month)
                            }) {
                                self.calendar = Some(calendar);
                                self.constrain_calendar_view();
                            }
                        }
                        cx.invalidate_layout();
                        true
                    }
                    KeyCode::Home | KeyCode::End if self.column != Column::Calendar => {
                        let count = self.row_count(self.column);
                        if count > 0 {
                            let index = if key.key_code == KeyCode::Home {
                                0
                            } else {
                                count - 1
                            };
                            let (_, target) = self.row(self.column, index);
                            self.choose(target, cx);
                            if let Some(index) = self.column.scroll_index() {
                                self.scroll[index].value = None;
                            }
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
    fn focusable(&self) -> bool {
        true
    }
    fn capture_eligible(&self) -> bool {
        true
    }
    fn wants_scroll(&self) -> bool {
        self.popup_open
    }
    fn on_capture_enter(&mut self, cx: &mut EventContext) {
        self.popup_open = true;
        self.refresh_minute_projection();
        cx.invalidate_layout();
    }
    fn on_capture_leave(&mut self, cx: &mut EventContext) {
        if self.armed.is_some() || self.dragging_column.is_some() {
            cx.release_pointer();
        }
        self.popup_open = false;
        self.hovered = None;
        self.armed = None;
        self.dragging_column = None;
        for scroll in &mut self.scroll {
            scroll.bar.end_drag();
        }
        // HAL-328: keyboard release is not Cancel; keep the exact working value.
        cx.invalidate_layout();
    }
    fn on_blur(&mut self, cx: &mut EventContext) {
        self.on_capture_leave(cx);
    }
    fn on_pointer_capture_cancel(
        &mut self,
        cx: &mut EventContext,
        _pointer: crate::core::event::PointerId,
        _reason: crate::core::event::CancellationReason,
    ) {
        self.armed = None;
        self.dragging_column = None;
        for scroll in &mut self.scroll {
            scroll.bar.end_drag();
        }
        cx.invalidate_paint();
    }
    fn paints_own_focus_indicator(&self) -> bool {
        true
    }
    fn debug_type_name(&self) -> &'static str {
        "DateTimePicker"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn date(day: u8) -> DateValue {
        DateValue::new(2026, 11, day).unwrap()
    }

    #[test]
    fn date_time_picker_clip_local_placement_preserves_natural_size_and_visible_input() {
        let clip = RectF::new(4.0, 99.0, 1092.0, 466.2705);
        let scroll = (0.0, 356.2129);
        let size = Size::new(508.0, 280.0);
        let anchor = RectF::new(20.0, 869.4834, 519.0, 28.0);
        let above = PopupPlacement::new(anchor, scroll, clip, size);
        assert!((above.window.bottom() - above.anchor_window.y).abs() < 0.0001);
        assert_eq!(above.visible_window, above.window);
        assert_eq!((above.window.width, above.window.height), (size.width, size.height));
        assert_eq!(above.paint, above.window.translate(scroll.0, scroll.1));

        let anchor = RectF::new(20.0, 506.2129, 519.0, 28.0);
        let below = PopupPlacement::new(anchor, scroll, clip, size);
        assert_eq!(below.window.y, below.anchor_window.bottom());
        assert_eq!(below.visible_window, below.window);

        let no_fit = RectF::new(40.0, 80.0, 360.0, 120.0);
        let placement = PopupPlacement::new(RectF::new(80.0, 130.0, 160.0, 28.0), (0.0, 0.0), no_fit, size);
        assert_eq!((placement.window.width, placement.window.height), (size.width, size.height));
        assert_eq!(placement.visible_window, placement.window.intersection(&no_fit).unwrap());
        assert_ne!(placement.visible_window, placement.window);
        assert!(placement.window.contains_point(placement.window.right() - 1.0, placement.window.bottom() - 1.0));
        assert!(!placement.visible_window.contains_point(placement.window.right() - 1.0, placement.window.bottom() - 1.0));
    }

    #[test]
    fn date_time_picker_unqualified_context_preserves_root_only_placement() {
        let theme = crate::core::theme::Theme::light();
        let timeline = crate::core::animation::Timeline::new();
        let mut picker = DateTimePicker::new(DateTimeSelection::Date(date(2))).unwrap();
        picker.geometry.size = Size::new(400.0, 244.0);
        let anchor = RectF::new(35.0, 80.0, 200.0, 28.0);
        crate::widgets::radio_button::tests::radio_paint(&timeline, &theme, |cx| {
            assert!(cx.own_presentation_geometry().is_none());
            let (width, height) = cx.viewport_size();
            let expected = compute_popup_rect(anchor, 400.0, 244.0, width, height).0;
            let projection = picker.popup_placement(cx, anchor);
            assert_eq!(projection.window, expected);
            assert_eq!(projection.visible_window, expected);
            assert!(projection.presentation.is_none());
        });
    }

    #[test]
    fn date_time_picker_retained_clip_projection_refreshes_scroll_and_resize() {
        use crate::core::app::RetainedSceneTestHarness;
        use crate::core::event::{FusedInputEvent, InteractionDomains, PointerDevice, WindowId};
        use crate::core::event_router::EventRouter;
        use crate::core::layout::FlexParams;
        use crate::core::math::Point;
        use crate::core::theme::Theme;
        use crate::core::widget_tree::WidgetTree;
        use crate::renderer::wgpu_renderer::WgpuRenderer;
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::{flex::Flex, scroll_area::ScrollArea};
        use std::sync::Arc;
        use std::time::Instant;

        fn control(tree: &WidgetTree, node: NodeId) -> &DateTimePicker {
            tree.node(node).unwrap().widget().as_any().downcast_ref().unwrap()
        }
        fn assert_fitting_projection(tree: &WidgetTree, node: NodeId, harness: &RetainedSceneTestHarness) {
            let picker = control(tree, node);
            let projection = picker.popup_projection.get();
            let admitted = tree.own_presentation_geometry(node, 1.0).unwrap();
            assert_eq!(projection.window.intersection(&admitted.clip), Some(projection.window));
            let overlay = harness.overlay_fragment(tree.node(node).unwrap().id).unwrap();
            let crate::core::scene::SceneNode::Clip(clip) = &overlay[0] else { panic!("actual popup clip") };
            let painted = clip.rect.translate(-admitted.cumulative_scroll.0, -admitted.cumulative_scroll.1);
            assert_eq!(projection.visible_window.intersection(&painted), Some(projection.visible_window));
            let apply = picker.geometry.apply;
            assert!(projection.visible_window.contains_point(
                projection.window.x + apply.x + apply.width * 0.5,
                projection.window.y + apply.y + apply.height * 0.5,
            ));
        }
        let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).expect("native text owner required");
        let theme = Theme::light();
        let icons = crate::core::icons::builtin_registry();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column().with_padding(
            crate::core::layout::EdgeInsets { left: 4.0, top: 99.0, right: 4.0, bottom: 0.0 },
        )), None, 0);
        let scroll = tree.add_child(root, Box::new(ScrollArea::new_unwired()), None, 0);
        tree.set_flex(scroll, FlexParams::fixed(466.2705));
        let content = tree.add_child(scroll, Box::new(Flex::column().with_padding(
            crate::core::layout::EdgeInsets { left: 16.0, top: 770.4834, right: 0.0, bottom: 600.0 },
        )), None, 0);
        tree.node_mut(scroll).unwrap().widget.as_any_mut().downcast_mut::<ScrollArea>().unwrap().wire_content(content);
        let value = DateTimeSelection::DateTimeRange {
            start: DateTimeValue::new(date(2), TimeValue::new(9, 7, None)).unwrap(),
            end: DateTimeValue::new(date(2), TimeValue::new(11, 30, None)).unwrap(),
        };
        let node = tree.add_child(content, Box::new(DateTimePicker::new(value).unwrap().with_width(519.0)), None, 0);
        let owner = tree.node(node).unwrap().id;
        let mut harness = RetainedSceneTestHarness::new();
        let mut domains = InteractionDomains::new();
        let mut frame = |tree: &mut WidgetTree, harness: &mut RetainedSceneTestHarness, domains: &InteractionDomains| {
            harness.frame_with_appearance(
                tree, root, Constraints::tight(Size::new(1100.0, 900.0)),
                &theme, renderer.text_engine_mut(), &icons, domains, WindowId::PRIMARY,
            )
        };
        frame(&mut tree, &mut harness, &domains);
        tree.node_mut(scroll).unwrap().widget.as_any_mut().downcast_mut::<ScrollArea>().unwrap().jump_to_y(356.2129);
        tree.mark_paint_dirty(scroll);
        frame(&mut tree, &mut harness, &domains);
        let anchor = tree.get_absolute_paint_rect(node).unwrap();
        let offset = tree.cumulative_scroll_offset(node);
        let mut router = EventRouter::new();
        router.route_input_event(
            &mut tree,
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY, time: Instant::now(),
                pos: Point::new(anchor.x - offset.0 + 10.0, anchor.y - offset.1 + 10.0),
                device: PointerDevice::Mouse, button: PointerButton::Primary, click_count: 1,
            },
            Some(root), renderer.text_engine_mut(), None, domains.primary_state_mut(), &mut BindingRegistry::new(),
        );
        let mut frame = |tree: &mut WidgetTree, harness: &mut RetainedSceneTestHarness| {
            harness.frame_with_appearance(
                tree, root, Constraints::tight(Size::new(1100.0, 900.0)),
                &theme, renderer.text_engine_mut(), &icons, &domains, WindowId::PRIMARY,
            )
        };
        frame(&mut tree, &mut harness);
        let anchor = tree.get_absolute_paint_rect(node).unwrap();
        let picker = control(&tree, node);
        assert!(picker.popup_open);
        let initial = picker.popup_projection.get();
        let size = picker.geometry.size;
        assert_eq!(size.height, picker.geometry.apply.bottom() + theme.geometry.time_picker.popup_padding);
        assert!(initial.window.y < initial.anchor_window.y);
        assert_fitting_projection(&tree, node, &harness);
        println!("DateTimeRange complete natural popup {size:?}, clip {:?}, projection {initial:?}", tree.own_presentation_geometry(node, 1.0).unwrap().clip);
        let warm = harness.overlay_fragment(owner).unwrap();
        frame(&mut tree, &mut harness);
        assert!(Arc::ptr_eq(&warm, &harness.overlay_fragment(owner).unwrap()));

        tree.node_mut(scroll).unwrap().widget.as_any_mut().downcast_mut::<ScrollArea>().unwrap().jump_to_y(380.2129);
        tree.mark_paint_dirty(scroll);
        frame(&mut tree, &mut harness);
        let offset = tree.cumulative_scroll_offset(node);
        let scrolled = control(&tree, node).popup_projection.get();
        assert_eq!(scrolled.anchor_window, anchor.translate(-offset.0, -offset.1), "open popup must refresh its window-space hit projection after ancestor scroll");
        assert_ne!(scrolled.anchor_window, initial.anchor_window);
        assert_fitting_projection(&tree, node, &harness);

        tree.set_flex(scroll, FlexParams::fixed(780.0));
        tree.mark_layout_dirty(root);
        frame(&mut tree, &mut harness);
        let resized = control(&tree, node).popup_projection.get();
        assert_eq!(resized.window.y, resized.anchor_window.bottom(), "newly available space below must be used after ancestor resize");
        assert_fitting_projection(&tree, node, &harness);
        assert_eq!(control(&tree, node).geometry.size, size);

        tree.set_flex(scroll, FlexParams::fixed(160.0));
        tree.mark_layout_dirty(root);
        frame(&mut tree, &mut harness);
        let picker = control(&tree, node);
        let no_fit = picker.popup_projection.get();
        let apply = picker.geometry.apply;
        let hidden_apply = Point::new(
            no_fit.window.x + apply.x + apply.width * 0.5,
            no_fit.window.y + apply.y + apply.height * 0.5,
        );
        assert_ne!(no_fit.visible_window, no_fit.window);
        assert!(no_fit.window.contains(hidden_apply));
        assert!(!no_fit.visible_window.contains(hidden_apply));
        router.route_input_event(
            &mut tree,
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY, time: Instant::now(), pos: hidden_apply,
                device: PointerDevice::Mouse, button: PointerButton::Primary, click_count: 1,
            },
            Some(root), renderer.text_engine_mut(), None, domains.primary_state_mut(), &mut BindingRegistry::new(),
        );
        assert!(!control(&tree, node).popup_open, "the invisible footer is backdrop dismissal, not an Apply hit");
        assert_eq!(control(&tree, node).value(), value);
    }

    // Local owner semantics only; this is not native/routed Capture evidence.
    fn context() -> EventContext<'static> {
        EventContext::new(
            TreeWidgetId(1),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            None,
            None,
            false,
            false,
            KeyboardAuthorityMode::Capture,
        )
    }

    #[test]
    fn date_time_picker_local_capture_exit_preserves_draft_and_only_cancel_reverts() {
        let commits = Rc::new(RefCell::new(Vec::new()));
        let received = Rc::clone(&commits);
        let initial = DateTimeSelection::Date(date(2));
        let mut picker = DateTimePicker::new(initial)
            .unwrap()
            .on_commit(move |value| received.borrow_mut().push(value));
        let mut cx = context();
        picker.on_capture_enter(&mut cx);
        picker.choose(Target::Day(date(3)), &mut cx);
        assert_eq!(picker.value(), initial);
        picker.on_capture_leave(&mut cx);
        assert!(!picker.popup_open);
        assert_eq!(picker.working(), DateTimeSelection::Date(date(3)));
        picker.on_capture_enter(&mut cx);
        assert_eq!(picker.working(), DateTimeSelection::Date(date(3)));
        picker.choose(Target::Cancel, &mut cx);
        assert_eq!(picker.working(), initial);
        assert!(commits.borrow().is_empty());
        picker.on_capture_enter(&mut cx);
        picker.choose(Target::Day(date(4)), &mut cx);
        picker.choose(Target::Apply, &mut cx);
        assert_eq!(picker.value(), DateTimeSelection::Date(date(4)));
        assert_eq!(*commits.borrow(), vec![DateTimeSelection::Date(date(4))]);
        picker.choose(Target::Apply, &mut cx);
        assert_eq!(commits.borrow().len(), 1);
    }

    #[test]
    fn date_time_picker_local_steps_preserve_off_grid_time_and_display_never_changes_value() {
        let initial = DateTimeSelection::Time(TimeValue::new(23, 7, Some(17)));
        let picker = DateTimePicker::new(initial)
            .unwrap()
            .with_steps(1, 15)
            .unwrap();
        assert_eq!(picker.value(), initial);
        assert_eq!(picker.minutes(), vec![0, 7, 15, 30, 45]);
        assert_eq!(picker.hours(), (0..24).collect::<Vec<_>>());
        assert_eq!(picker.format_selection(initial), "23:07:17");
        let picker = picker.with_display_mode(TimeDisplayMode::H12);
        assert_eq!(picker.value(), initial);
        assert_eq!(picker.format_selection(initial), "11:07:17 PM");
        assert_eq!(picker.row(Column::Hour, 0).0, "01");
        assert_eq!(picker.row(Column::Hour, 11).0, "12");
    }

    #[test]
    fn date_time_picker_local_page_navigation_keeps_cursor_in_visible_month_without_commit() {
        let initial = DateTimeSelection::Date(DateValue::new(2026, 1, 31).unwrap());
        let mut picker = DateTimePicker::new(initial).unwrap();
        let mut cx = context();
        picker.on_capture_enter(&mut cx);
        let key = |key_code| {
            Event::Key(crate::core::event::KeyEvent {
                key_code,
                physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                is_down: true,
                is_repeat: false,
                modifiers: crate::core::event::Modifiers::default(),
                text: None,
            })
        };
        assert!(picker.handle_event(&mut cx, &key(KeyCode::PageDown), EventPhase::Target));
        assert_eq!(picker.cursor, DateValue::new(2026, 2, 28));
        assert_eq!(picker.working(), initial);
        assert!(picker.handle_event(&mut cx, &key(KeyCode::Right), EventPhase::Target));
        assert_eq!(
            picker.working(),
            DateTimeSelection::Date(DateValue::new(2026, 3, 1).unwrap())
        );
        assert_eq!(picker.value(), initial);
    }

    #[test]
    fn date_time_picker_civil_bounds_limit_navigation_without_replacing_value() {
        let initial = DateTimeSelection::Date(date(2));
        let min = DateValue::new(2026, 5, 15).unwrap();
        let max = DateValue::new(2027, 2, 10).unwrap();
        let mut picker = DateTimePicker::new(initial)
            .unwrap()
            .with_date_bounds(min, max)
            .unwrap();
        let mut cx = context();
        assert_eq!(picker.year_bounds(), (2026, 2027));
        assert_eq!(picker.row_count(Column::Year), 2);
        picker.choose(Target::Year(2025), &mut cx);
        assert_eq!(picker.calendar.unwrap().year, 2026);
        picker.choose(Target::Year(2027), &mut cx);
        assert_eq!(picker.calendar.unwrap().month, 2);
        assert!(!picker.target_available(Target::Month(3)));
        assert!(!picker.target_available(Target::Day(DateValue::new(2027, 2, 11).unwrap())));
        picker.choose(Target::Day(max), &mut cx);
        picker.choose(Target::Apply, &mut cx);
        assert_eq!(picker.value(), DateTimeSelection::Date(max));
        picker.keyboard_step(1, &mut cx);
        assert_eq!(picker.working(), DateTimeSelection::Date(max));
        let june_min = DateValue::new(2026, 6, 1).unwrap();
        let june_max = DateValue::new(2026, 6, 30).unwrap();
        picker.set_date_bounds(june_min, june_max).unwrap();
        assert_eq!(picker.value(), DateTimeSelection::Date(max));
        assert_eq!(picker.working(), DateTimeSelection::Date(max));
        assert_eq!(
            picker.working_availability(),
            Err(DateTimeSelectionError::OutsideDateBounds)
        );
        picker.choose(Target::Apply, &mut cx);
        assert_eq!(picker.value(), DateTimeSelection::Date(max));
        assert_eq!(
            picker.set_date_bounds(max, min),
            Err(DateTimeSelectionError::InvalidDateBounds)
        );
        assert_eq!(picker.date_bounds(), Some((june_min, june_max)));
        assert!(matches!(
            DateTimePicker::new(DateTimeSelection::Time(TimeValue::new(9, 0, None)))
                .unwrap()
                .with_year_bounds(2025, 2030),
            Err(DateTimeSelectionError::ModeMismatch)
        ));
    }

    #[test]
    fn date_time_picker_disabled_time_visibility_preserves_policy_and_exact_value() {
        fn picker(
            value: DateTimeSelection,
            visibility: UnavailableTimeSlotVisibility,
        ) -> DateTimePicker {
            DateTimePicker::new(value)
                .unwrap()
                .with_steps(1, 15)
                .unwrap()
                .with_date_time_availability(|value| {
                    value.date == date(2)
                        && (9..18).contains(&value.time.hour)
                        && value.time.hour != 12
                })
                .unwrap()
                .with_unavailable_time_slots(visibility)
        }
        let value = DateTimeSelection::DateTime(
            DateTimeValue::new(date(2), TimeValue::new(9, 7, Some(17))).unwrap(),
        );
        let show = picker(value, UnavailableTimeSlotVisibility::Show);
        assert_eq!(
            DateTimePicker::new(value).unwrap().unavailable_time_slots,
            UnavailableTimeSlotVisibility::Show
        );
        assert_eq!(show.row_count(Column::Hour), 24);
        assert!(show.time_targets(Column::Hour).contains(&Target::Hour(12)));
        assert!(!show.target_available(Target::Hour(12)));

        let mut hide = picker(value, UnavailableTimeSlotVisibility::Hide);
        assert_eq!(
            hide.time_targets(Column::Hour),
            vec![
                Target::Hour(9),
                Target::Hour(10),
                Target::Hour(11),
                Target::Hour(13),
                Target::Hour(14),
                Target::Hour(15),
                Target::Hour(16),
                Target::Hour(17)
            ]
        );
        assert!(
            hide.time_targets(Column::Minute)
                .contains(&Target::Minute(7))
        );
        assert_eq!(hide.value(), value);
        assert_eq!(hide.working(), value);
        let mut cx = context();
        hide.choose(Target::Hour(12), &mut cx);
        assert_eq!(
            hide.working(),
            value,
            "hidden unavailable alternatives remain unselectable"
        );
        hide.choose(Target::Hour(13), &mut cx);
        assert_eq!(hide.time(), Some(TimeValue::new(13, 7, Some(17))));
        assert_eq!(
            hide.value(),
            value,
            "presentation never commits or rounds a value"
        );

        let occupied = DateTimeSelection::DateTime(
            DateTimeValue::new(date(2), TimeValue::new(12, 7, Some(17))).unwrap(),
        );
        let mut occupied = picker(occupied, UnavailableTimeSlotVisibility::Hide);
        assert!(
            occupied
                .time_targets(Column::Hour)
                .contains(&Target::Hour(12))
        );
        assert!(occupied.selected(Target::Hour(12)));
        assert!(!occupied.target_available(Target::Hour(12)));
        assert_eq!(
            occupied.time_targets(Column::Minute),
            vec![Target::Minute(7)]
        );
        assert_eq!(
            occupied.working_availability(),
            Err(DateTimeSelectionError::Unavailable)
        );
        occupied.choose(Target::Apply, &mut cx);
        assert_eq!(occupied.value(), occupied.working());

        let range = DateTimeSelection::DateTimeRange {
            start: DateTimeValue::new(date(2), TimeValue::new(9, 0, None)).unwrap(),
            end: DateTimeValue::new(date(2), TimeValue::new(18, 0, None)).unwrap(),
        };
        let mut range_picker = picker(range, UnavailableTimeSlotVisibility::Hide)
            .with_range_availability(move |candidate| candidate == range)
            .unwrap();
        range_picker.select_endpoint(DateTimeEndpoint::End);
        assert!(
            range_picker
                .time_targets(Column::Hour)
                .contains(&Target::Hour(18)),
            "excluded end is qualified by complete range policy, not point availability"
        );
        assert_eq!(range_picker.working_availability(), Ok(()));
        range_picker.choose(Target::Hour(17), &mut cx);
        assert_eq!(
            range_picker
                .working()
                .time(DateTimeEndpoint::End)
                .unwrap()
                .hour,
            17
        );
        assert_eq!(
            range_picker.working_availability(),
            Err(DateTimeSelectionError::Unavailable)
        );
        range_picker.choose(Target::Apply, &mut cx);
        assert_eq!(
            range_picker.value(),
            range,
            "independent draft edits cannot bypass final range admission"
        );
    }

    #[test]
    fn date_time_picker_h12_independent_draft_components_preserve_final_point_admission() {
        let key = |key_code| {
            Event::Key(crate::core::event::KeyEvent {
                key_code,
                physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                is_down: true,
                is_repeat: false,
                modifiers: crate::core::event::Modifiers::default(),
                text: None,
            })
        };
        for visibility in [
            UnavailableTimeSlotVisibility::Show,
            UnavailableTimeSlotVisibility::Hide,
        ] {
            for date_time in [false, true] {
                for period_first in [false, true] {
                    let initial_time = TimeValue::new(9, 7, Some(17));
                    let initial = if date_time {
                        DateTimeSelection::DateTime(
                            DateTimeValue::new(date(2), initial_time).unwrap(),
                        )
                    } else {
                        DateTimeSelection::Time(initial_time)
                    };
                    let commits = Rc::new(RefCell::new(Vec::new()));
                    let received = Rc::clone(&commits);
                    let mut picker = DateTimePicker::new(initial)
                        .unwrap()
                        .with_reference_date(date(2))
                        .unwrap()
                        .with_display_mode(TimeDisplayMode::H12)
                        .with_steps(1, 15)
                        .unwrap()
                        .with_date_time_availability(|value| {
                            value.date == date(2)
                                && (9..18).contains(&value.time.hour)
                                && value.time.hour != 12
                        })
                        .unwrap()
                        .with_unavailable_time_slots(visibility)
                        .on_commit(move |value| received.borrow_mut().push(value));
                    let mut cx = context();
                    picker.on_capture_enter(&mut cx);
                    assert!(picker.time_targets(Column::Hour).contains(&Target::Hour(1)));
                    assert!(
                        picker
                            .time_targets(Column::Period)
                            .contains(&Target::Period(true))
                    );
                    assert!(!picker.target_available(Target::Hour(1)));
                    assert!(!picker.target_available(Target::Period(true)));

                    // Either component can be edited first without admitting
                    // the unavailable intermediate point as a committed value.
                    picker.column = if period_first {
                        Column::Period
                    } else {
                        Column::Hour
                    };
                    assert!(picker.handle_event(
                        &mut cx,
                        &key(if period_first {
                            KeyCode::Down
                        } else {
                            KeyCode::Home
                        }),
                        EventPhase::Target,
                    ));
                    assert_eq!(
                        picker.time(),
                        Some(TimeValue::new(
                            if period_first { 21 } else { 1 },
                            7,
                            Some(17)
                        ))
                    );
                    assert_eq!(
                        picker.working_availability(),
                        Err(DateTimeSelectionError::Unavailable)
                    );
                    assert!(!picker.target_available(Target::Apply));
                    picker.choose(Target::Apply, &mut cx);
                    assert!(picker.handle_event(&mut cx, &key(KeyCode::Enter), EventPhase::Target));
                    assert_eq!(picker.value(), initial);
                    assert!(picker.popup_open);
                    assert!(commits.borrow().is_empty());

                    picker.column = if period_first {
                        Column::Hour
                    } else {
                        Column::Period
                    };
                    assert!(picker.handle_event(
                        &mut cx,
                        &key(if period_first {
                            KeyCode::Home
                        } else {
                            KeyCode::Down
                        }),
                        EventPhase::Target,
                    ));
                    assert_eq!(picker.time(), Some(TimeValue::new(13, 7, Some(17))));
                    assert_eq!(picker.working_availability(), Ok(()));
                    assert_eq!(picker.value(), initial);
                    assert!(commits.borrow().is_empty());
                    assert!(picker.handle_event(&mut cx, &key(KeyCode::Enter), EventPhase::Target));
                    assert_eq!(picker.value(), picker.working());
                    assert_eq!(*commits.borrow(), vec![picker.working()]);
                }
            }
        }
    }

    #[test]
    fn date_time_picker_routed_minute_rows_survive_edits_and_requalify_on_basis_change() {
        use crate::core::app::RetainedSceneTestHarness;
        use crate::core::event::{FusedInputEvent, InteractionDomains, PointerDevice, WindowId};
        use crate::core::event_router::EventRouter;
        use crate::core::icons::builtin_registry;
        use crate::core::math::Point;
        use crate::core::theme::Theme;
        use crate::core::widget_tree::WidgetTree;
        use crate::platform::font_manager_text_engine::FontManagerTextEngine;
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::flex::Flex;
        use std::time::Instant;

        fn control(tree: &WidgetTree, node: NodeId) -> &DateTimePicker {
            tree.node(node).unwrap().widget().as_any().downcast_ref().unwrap()
        }
        fn rows(picker: &DateTimePicker) -> Vec<(Target, RectF)> {
            picker.geometry.columns.iter()
                .find(|column| column.kind == Column::Minute).unwrap()
                .rows.iter().map(|row| (row.target, row.rect)).collect()
        }
        fn point(picker: &DateTimePicker, target: Target) -> Point {
            let rect = match target {
                Target::Apply => picker.geometry.apply,
                Target::Cancel => picker.geometry.cancel,
                _ => picker.geometry.columns.iter().flat_map(|column| &column.rows)
                    .find(|row| row.target == target).unwrap().rect,
            };
            let popup = picker.popup_projection.get().window;
            Point::new(popup.x + rect.x + rect.width * 0.5, popup.y + rect.y + rect.height * 0.5)
        }

        for visibility in [UnavailableTimeSlotVisibility::Show, UnavailableTimeSlotVisibility::Hide] {
            let initial = DateTimeSelection::DateTime(
                DateTimeValue::new(date(2), TimeValue::new(9, 7, Some(17))).unwrap(),
            );
            let mut tree = WidgetTree::new();
            let root = tree.create_root(Box::new(Flex::column()), None, 0);
            let node = tree.add_child(root, Box::new(DateTimePicker::new(initial).unwrap()
                .with_steps(1, 15).unwrap()
                .with_date_time_availability(|value| (9..11).contains(&value.time.hour)).unwrap()
                .with_unavailable_time_slots(visibility)), None, 0);
            let (mut router, mut domains, mut bindings) = (
                EventRouter::new(), InteractionDomains::new(), BindingRegistry::new(),
            );
            let (mut text, mut harness) = (
                FontManagerTextEngine::new(), RetainedSceneTestHarness::new(),
            );
            let (theme, icons) = (Theme::default(), builtin_registry());
            let mut frame = |tree: &mut WidgetTree, domains: &InteractionDomains,
                             text: &mut FontManagerTextEngine| {
                harness.frame_with_appearance(tree, root,
                    Constraints::tight(Size::new(900.0, 600.0)), &theme, text,
                    &icons, domains, WindowId::PRIMARY);
            };
            let mut click = |tree: &mut WidgetTree, domains: &mut InteractionDomains,
                             text: &mut FontManagerTextEngine, pos: Point| {
                for event in [
                    FusedInputEvent::PointerDown {
                        window_id: WindowId::PRIMARY, time: Instant::now(), pos,
                        device: PointerDevice::Mouse, button: PointerButton::Primary, click_count: 1,
                    },
                    FusedInputEvent::PointerUp {
                        window_id: WindowId::PRIMARY, time: Instant::now(), pos,
                        device: PointerDevice::Mouse, button: PointerButton::Primary,
                    },
                ] {
                    router.route_input_event(tree, event, Some(root), text, None,
                        domains.primary_state_mut(), &mut bindings);
                }
            };
            frame(&mut tree, &domains, &mut text);
            let anchor = tree.get_absolute_paint_rect(node).unwrap();
            let trigger = Point::new(anchor.x + 10.0, anchor.y + 10.0);
            click(&mut tree, &mut domains, &mut text, trigger);
            frame(&mut tree, &domains, &mut text);
            let before = rows(control(&tree, node));
            let expected = vec![0, 7, 15, 30, 45].into_iter().map(Target::Minute).collect::<Vec<_>>();
            assert_eq!(before.iter().map(|row| row.0).collect::<Vec<_>>(), expected);
            let minute = point(control(&tree, node), Target::Minute(15));
            click(&mut tree, &mut domains, &mut text, minute);
            frame(&mut tree, &domains, &mut text);
            let picker = control(&tree, node);
            assert_eq!(picker.value(), initial);
            assert_eq!(picker.time(), Some(TimeValue::new(9, 15, Some(17))));
            assert!(!picker.selected(Target::Minute(7)));
            assert!(picker.selected(Target::Minute(15)));
            assert_eq!(rows(picker), before, "minute selection must not remove or move rows");
            let popup = picker.popup_projection.get().window;
            assert_eq!(picker.hit(minute.x - popup.x, minute.y - popup.y), Some(Target::Minute(15)));

            // Capture dismissal retains the draft and this exact minute projection.
            click(&mut tree, &mut domains, &mut text, Point::new(899.0, 599.0));
            frame(&mut tree, &domains, &mut text);
            assert!(!control(&tree, node).popup_open);
            click(&mut tree, &mut domains, &mut text, trigger);
            frame(&mut tree, &domains, &mut text);
            assert_eq!(rows(control(&tree, node)), before);

            // Apply reconciles the session; the next opening starts from minute15.
            let apply = point(control(&tree, node), Target::Apply);
            click(&mut tree, &mut domains, &mut text, apply);
            frame(&mut tree, &domains, &mut text);
            assert!(control(&tree, node).minute_projection.is_none());
            click(&mut tree, &mut domains, &mut text, trigger);
            frame(&mut tree, &domains, &mut text);
            assert!(!control(&tree, node).time_targets(Column::Minute).contains(&Target::Minute(7)));

            // External reinitialization may be unavailable; it is never rounded.
            let occupied = DateTimeSelection::DateTime(
                DateTimeValue::new(date(2), TimeValue::new(12, 7, Some(17))).unwrap(),
            );
            tree.node_mut(node).unwrap().widget_mut().as_any_mut()
                .downcast_mut::<DateTimePicker>().unwrap().set_value(occupied).unwrap();
            tree.mark_layout_dirty(node);
            frame(&mut tree, &domains, &mut text);
            assert_eq!(control(&tree, node).working_availability(), Err(DateTimeSelectionError::Unavailable));
            if visibility == UnavailableTimeSlotVisibility::Hide {
                assert_eq!(control(&tree, node).time_targets(Column::Minute), vec![Target::Minute(7)]);
            }
            // A real hour edit requalifies Hide instead of stranding it at [07].
            let hour = point(control(&tree, node), Target::Hour(9));
            click(&mut tree, &mut domains, &mut text, hour);
            frame(&mut tree, &domains, &mut text);
            assert_eq!(control(&tree, node).time_targets(Column::Minute), expected);
            assert_eq!(control(&tree, node).time(), Some(TimeValue::new(9, 7, Some(17))));
            let cancel = point(control(&tree, node), Target::Cancel);
            click(&mut tree, &mut domains, &mut text, cancel);
            frame(&mut tree, &domains, &mut text);
            assert_eq!(control(&tree, node).working(), occupied);
            assert!(control(&tree, node).minute_projection.is_none());
        }
    }

    #[test]
    fn date_time_picker_routed_wheel_is_item_bounded_and_preserves_thumb_drag() {
        use crate::core::app::RetainedSceneTestHarness;
        use crate::core::event::{
            FusedInputEvent, InteractionDomains, PointerButtonsState, PointerDevice, RawInputEvent,
            WheelDelta, WindowId,
        };
        use crate::core::event_router::EventRouter;
        use crate::core::icons::builtin_registry;
        use crate::core::input_fusion::InputFusion;
        use crate::core::math::Point;
        use crate::core::theme::Theme;
        use crate::core::widget_tree::WidgetTree;
        use crate::platform::font_manager_text_engine::FontManagerTextEngine;
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::flex::Flex;
        use std::time::Instant;

        fn control(tree: &WidgetTree, node: NodeId) -> &DateTimePicker {
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref()
                .unwrap()
        }
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        let value = DateTimeSelection::Date(date(2));
        let node = tree.add_child(
            root,
            Box::new(
                DateTimePicker::new(value)
                    .unwrap()
                    .with_year_bounds(2020, 2035)
                    .unwrap(),
            ),
            None,
            0,
        );
        let (mut router, mut domains, mut bindings) = (
            EventRouter::new(),
            InteractionDomains::new(),
            BindingRegistry::new(),
        );
        let (mut text, mut harness) = (
            FontManagerTextEngine::new(),
            RetainedSceneTestHarness::new(),
        );
        let (theme, icons) = (Theme::default(), builtin_registry());
        let mut frame = |tree: &mut WidgetTree,
                         domains: &InteractionDomains,
                         text: &mut FontManagerTextEngine| {
            harness.frame_with_appearance(
                tree,
                root,
                Constraints::tight(Size::new(900.0, 600.0)),
                &theme,
                text,
                &icons,
                domains,
                WindowId::PRIMARY,
            );
        };
        let mut route = |tree: &mut WidgetTree,
                         domains: &mut InteractionDomains,
                         text: &mut FontManagerTextEngine,
                         event| {
            router.route_input_event(
                tree,
                event,
                Some(root),
                text,
                None,
                domains.primary_state_mut(),
                &mut bindings,
            );
        };
        frame(&mut tree, &domains, &mut text);
        let anchor = tree.get_absolute_paint_rect(node).unwrap();
        route(
            &mut tree,
            &mut domains,
            &mut text,
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: Point::new(anchor.x + 10.0, anchor.y + 10.0),
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            },
        );
        frame(&mut tree, &domains, &mut text);
        assert!(control(&tree, node).popup_open);
        let mut fusion = InputFusion::new();
        fusion.set_scale_factor(2.0);
        for kind in [Column::Year, Column::Month] {
            let picker = control(&tree, node);
            let column = picker
                .geometry
                .columns
                .iter()
                .find(|column| column.kind == kind)
                .unwrap();
            let before = column.scroll;
            let row = picker.geometry.row_height;
            assert!(before >= row * 2.0);
            let popup = picker.popup_projection.get().window;
            let position = Point::new(
                popup.x + column.body.x + column.body.width * 0.5,
                popup.y + column.body.y + column.body.height * 0.5,
            );
            for intent in 1..=2 {
                let mut events = Vec::new();
                fusion.process_raw(
                    RawInputEvent::PointerWheel {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        device: PointerDevice::Mouse,
                        delta: WheelDelta::Lines(0.0, 1.0),
                        position,
                    },
                    &mut events,
                );
                fusion.flush(&mut events);
                assert_eq!(events.len(), 1);
                assert!(
                    matches!(events[0], FusedInputEvent::Scroll { delta, .. } if delta.1 > row)
                );
                for event in events {
                    route(&mut tree, &mut domains, &mut text, event);
                }
                assert_eq!(
                    control(&tree, node).scroll[kind.scroll_index().unwrap()].value,
                    Some(before - row * intent as f32),
                    "each intent accumulates before the next Layout"
                );
            }
            frame(&mut tree, &domains, &mut text);
            let picker = control(&tree, node);
            let after = picker
                .geometry
                .columns
                .iter()
                .find(|column| column.kind == kind)
                .unwrap()
                .scroll;
            assert_eq!(
                after,
                before - row * 2.0,
                "two normalized wheel intents move two items"
            );
            assert_eq!(picker.working(), value);
            assert_eq!(picker.value(), value);

            let mut events = Vec::new();
            fusion.process_raw(
                RawInputEvent::PointerWheel {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    device: PointerDevice::Touchpad,
                    delta: WheelDelta::Pixels(0.0, 0.25),
                    position,
                },
                &mut events,
            );
            fusion.flush(&mut events);
            for event in events {
                route(&mut tree, &mut domains, &mut text, event);
            }
            frame(&mut tree, &domains, &mut text);
            let precise = control(&tree, node)
                .geometry
                .columns
                .iter()
                .find(|column| column.kind == kind)
                .unwrap()
                .scroll;
            assert_eq!(
                precise,
                after - 0.25,
                "sub-row pixel input keeps its precision"
            );
        }

        let picker = control(&tree, node);
        let column = picker
            .geometry
            .columns
            .iter()
            .find(|column| column.kind == Column::Year)
            .unwrap();
        let body = column.body;
        let popup = picker.popup_projection.get().window;
        let content = column.count as f32 * picker.geometry.row_height;
        let layout = picker.scroll[0].bar.layout(
            RectF::new(0.0, 0.0, body.width, body.height),
            body.height,
            content,
            body.height,
            column.scroll,
        );
        let down = Point::new(
            popup.x + body.x + layout.thumb.x + layout.thumb.width * 0.5,
            popup.y + body.y + layout.thumb.y + layout.thumb.height * 0.5,
        );
        route(
            &mut tree,
            &mut domains,
            &mut text,
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: down,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            },
        );
        assert!(control(&tree, node).scroll[0].bar.dragging);
        let moved = Point::new(down.x, down.y + 4.25);
        let expected = control(&tree, node).scroll[0].bar.update_drag(moved.y);
        let mut buttons = PointerButtonsState::new();
        buttons.set(PointerButton::Primary, true);
        route(
            &mut tree,
            &mut domains,
            &mut text,
            FusedInputEvent::PointerMove {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: moved,
                delta: (0.0, 4.25),
                device: PointerDevice::Mouse,
                buttons,
            },
        );
        assert_eq!(control(&tree, node).scroll[0].value, Some(expected));
        let mut events = Vec::new();
        fusion.process_raw(
            RawInputEvent::PointerWheel {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                device: PointerDevice::Mouse,
                delta: WheelDelta::Lines(0.0, -8.0),
                position: moved,
            },
            &mut events,
        );
        fusion.flush(&mut events);
        for event in events {
            route(&mut tree, &mut domains, &mut text, event);
        }
        assert_eq!(
            control(&tree, node).scroll[0].value,
            Some(expected),
            "wheel cannot preempt thumb drag"
        );
        route(
            &mut tree,
            &mut domains,
            &mut text,
            FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: moved,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
            },
        );
        assert!(!control(&tree, node).scroll[0].bar.dragging);
        assert_eq!(control(&tree, node).working(), value);
    }

    #[test]
    fn date_time_picker_scrolled_overlay_pointer_matches_painted_day() {
        use crate::core::app::RetainedSceneTestHarness;
        use crate::core::event::{
            FusedInputEvent, InteractionDomains, PointerButtonsState, PointerDevice, WindowId,
        };
        use crate::core::event_router::EventRouter;
        use crate::core::layout::{EdgeInsets, FlexParams};
        use crate::core::math::Point;
        use crate::core::scene::SceneNode;
        use crate::core::theme::Theme;
        use crate::core::widget_tree::WidgetTree;
        use crate::renderer::wgpu_renderer::WgpuRenderer;
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::{flex::Flex, scroll_area::ScrollArea};
        use std::time::Instant;

        // Observe actual scene positions through the same scroll scopes as the
        // renderer, rather than reconstructing a day rect from widget inputs.
        fn day_point(nodes: &[SceneNode], scroll: (f32, f32)) -> Option<Point> {
            for node in nodes {
                let found = match node {
                    SceneNode::Text(text)
                        if text.text == "3" && text.z_index == LAYER_OVERLAY + 4 =>
                    {
                        Some(Point::new(
                            text.position.x - scroll.0 + text.font_size * 0.25,
                            text.position.y - scroll.1 + text.font_size * 0.5,
                        ))
                    }
                    SceneNode::Scroll(n) => {
                        day_point(&n.children, (scroll.0 + n.offset.0, scroll.1 + n.offset.1))
                    }
                    SceneNode::RetainedFragment(n) => day_point(&n.children, scroll),
                    SceneNode::Clip(n) => day_point(&n.children, scroll),
                    SceneNode::Layer(n) => day_point(&n.children, scroll),
                    _ => None,
                };
                if found.is_some() {
                    return found;
                }
            }
            None
        }

        let mut renderer =
            pollster::block_on(WgpuRenderer::new_headless()).expect("real text owner required");
        let theme = Theme::light();
        let icons = crate::core::icons::builtin_registry();
        let mut opened_above_and_below = [false; 2];
        for (viewport_height, scroll_y) in [(600.0, 0.0), (600.0, 116.0), (700.0, 116.0)] {
            let mut tree = WidgetTree::new();
            let root = tree.create_root(
                Box::new(Flex::column().with_padding(EdgeInsets {
                    left: 37.0,
                    right: 17.0,
                    top: 23.0,
                    bottom: 13.0,
                })),
                None,
                0,
            );
            let scroll = tree.add_child(root, Box::new(ScrollArea::new_unwired()), None, 0);
            tree.set_flex(scroll, FlexParams::flexible(1.0));
            let content = tree.add_child(
                scroll,
                Box::new(Flex::column().with_padding(EdgeInsets {
                    left: 11.0,
                    right: 7.0,
                    top: 360.0,
                    bottom: 600.0,
                })),
                None,
                0,
            );
            tree.node_mut(scroll)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<ScrollArea>()
                .unwrap()
                .wire_content(content);
            let picker = tree.add_child(
                content,
                Box::new(DateTimePicker::new(DateTimeSelection::Date(date(2))).unwrap()),
                None,
                0,
            );
            let mut harness = RetainedSceneTestHarness::new();
            let mut domains = InteractionDomains::new();
            let mut router = EventRouter::new();
            let mut bindings = BindingRegistry::new();
            let mut frame = |tree: &mut WidgetTree,
                             renderer: &mut WgpuRenderer,
                             domains: &InteractionDomains| {
                harness.frame_with_appearance(
                    tree,
                    root,
                    Constraints::tight(Size::new(900.0, viewport_height)),
                    &theme,
                    renderer.text_engine_mut(),
                    &icons,
                    domains,
                    WindowId::PRIMARY,
                )
            };
            frame(&mut tree, &mut renderer, &domains);
            tree.node_mut(scroll)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<ScrollArea>()
                .unwrap()
                .jump_to_y(scroll_y);
            tree.mark_subtree_paint_dirty(scroll);
            frame(&mut tree, &mut renderer, &domains);
            let anchor = tree.get_absolute_paint_rect(picker).unwrap();
            let offset = tree.cumulative_scroll_offset(picker);
            let mut route = |tree: &mut WidgetTree,
                             renderer: &mut WgpuRenderer,
                             domains: &mut InteractionDomains,
                             event| {
                router.route_input_event(
                    tree,
                    event,
                    Some(root),
                    renderer.text_engine_mut(),
                    None,
                    domains.primary_state_mut(),
                    &mut bindings,
                );
            };
            let down = |pos| FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            };
            route(
                &mut tree,
                &mut renderer,
                &mut domains,
                down(Point::new(
                    anchor.x - offset.0 + anchor.width * 0.5,
                    anchor.y - offset.1 + anchor.height * 0.5,
                )),
            );
            let opened = frame(&mut tree, &mut renderer, &domains);
            let point = day_point(&opened.scene_root, (0.0, 0.0)).expect("painted day 3");
            let popup = tree
                .node(picker)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<DateTimePicker>()
                .unwrap()
                .popup_projection
                .get().window;
            assert!(popup.contains_point(point.x, point.y));
            assert!(popup.x >= 0.0 && popup.right() <= 900.0);
            assert!(popup.y >= 0.0 && popup.bottom() <= viewport_height);
            let anchor_top = anchor.y - offset.1;
            if popup.y < anchor_top {
                assert_eq!(popup.bottom(), anchor_top);
                opened_above_and_below[0] = true;
            } else {
                assert_eq!(popup.y, anchor_top + anchor.height);
                opened_above_and_below[1] = true;
            }
            route(
                &mut tree,
                &mut renderer,
                &mut domains,
                FusedInputEvent::PointerMove {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    pos: point,
                    delta: (0.0, 0.0),
                    device: PointerDevice::Mouse,
                    buttons: PointerButtonsState::new(),
                },
            );
            let control = tree
                .node(picker)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<DateTimePicker>()
                .unwrap();
            assert!(
                control.hovered == Some(Target::Day(date(3))),
                "painted day point={point:?}, cached popup={:?}, ancestor scroll={offset:?}",
                control.popup_projection.get().window
            );
            let hovered = frame(&mut tree, &mut renderer, &domains);
            assert_eq!(day_point(&hovered.scene_root, (0.0, 0.0)), Some(point));
            route(&mut tree, &mut renderer, &mut domains, down(point));
            let control = tree
                .node(picker)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<DateTimePicker>()
                .unwrap();
            assert_eq!(control.working(), DateTimeSelection::Date(date(3)));
            assert_eq!(control.value(), DateTimeSelection::Date(date(2)));
        }
        assert_eq!(opened_above_and_below, [true, true]);
    }

    #[test]
    fn date_time_picker_local_availability_uses_full_date_time_and_retains_unavailable_value() {
        let initial = DateTimeSelection::Time(TimeValue::new(9, 7, Some(17)));
        assert!(matches!(
            DateTimePicker::new(initial)
                .unwrap()
                .with_date_time_availability(|_| true),
            Err(DateTimeSelectionError::MissingDateContext)
        ));
        let mut picker = DateTimePicker::new(initial)
            .unwrap()
            .with_reference_date(date(2))
            .unwrap()
            .with_date_time_availability(|value| {
                value.date == date(2) && value.time.hour == 10 && value.time.second == Some(17)
            })
            .unwrap();
        assert_eq!(picker.value(), initial);
        assert_eq!(
            picker.working_availability(),
            Err(DateTimeSelectionError::Unavailable)
        );
        let mut cx = context();
        picker.choose(Target::Apply, &mut cx);
        assert_eq!(picker.value(), initial);
        picker.choose(Target::Hour(10), &mut cx);
        assert_eq!(
            picker.working().time(DateTimeEndpoint::Start),
            Some(TimeValue::new(10, 7, Some(17)))
        );
        picker.choose(Target::Apply, &mut cx);
        assert_eq!(picker.value(), picker.working());
    }

    #[test]
    fn date_time_picker_local_complete_range_policy_owns_excluded_end_and_invalid_drafts() {
        let point = |hour| DateTimeValue::new(date(2), TimeValue::new(hour, 0, None)).unwrap();
        let initial = DateTimeSelection::DateTimeRange {
            start: point(11),
            end: point(12),
        };
        let mut picker = DateTimePicker::new(initial)
            .unwrap()
            .with_date_time_availability(|value| value.time.hour < 12)
            .unwrap()
            .with_range_availability(|value| match value {
                DateTimeSelection::DateTimeRange { start, end } => {
                    start.date == end.date && start.time.hour >= 9 && end.time.hour <= 12
                }
                _ => false,
            })
            .unwrap();
        assert_eq!(picker.working_availability(), Ok(()));
        let mut cx = context();
        picker.select_endpoint(DateTimeEndpoint::End);
        picker.choose(Target::Hour(10), &mut cx);
        assert_eq!(
            picker.working().time(DateTimeEndpoint::End),
            Some(TimeValue::new(10, 0, None))
        );
        assert_eq!(
            picker.working_availability(),
            Err(DateTimeSelectionError::InvalidRange)
        );
        picker.choose(Target::Apply, &mut cx);
        assert_eq!(picker.value(), initial);
        picker.choose(Target::Hour(12), &mut cx);
        assert_eq!(picker.working_availability(), Ok(()));
        picker.choose(Target::Hour(13), &mut cx);
        assert_eq!(
            picker.working_availability(),
            Err(DateTimeSelectionError::Unavailable)
        );
        picker.on_capture_leave(&mut cx);
        assert_eq!(
            picker.working().time(DateTimeEndpoint::End),
            Some(TimeValue::new(13, 0, None))
        );
    }
}
