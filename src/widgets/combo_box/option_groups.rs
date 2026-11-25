use crate::model::selection_resolver::OptionId;
use std::collections::HashSet;

/// A presentation heading before the real option at `start_index`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptionGroupBoundary<'a> {
    pub start_index: usize,
    pub title: &'a str,
}

/// A grouped ComboBox projection was rejected without replacing the current one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum GroupedProjectionError {
    #[error("a grouped projection needs at least one option")]
    EmptyItems,
    #[error("a grouped projection needs at least one boundary")]
    MissingBoundaries,
    #[error("the first group must start at option index 0")]
    FirstGroupStartsAfterZero,
    #[error("group boundary {0} is outside the option list")]
    BoundaryOutOfRange(usize),
    #[error("group boundary {0} must be greater than its predecessor")]
    BoundariesNotIncreasing(usize),
    #[error("the group starting at option index {0} needs a title")]
    EmptyTitle(usize),
    #[error("duplicate option ID {0:?}")]
    DuplicateOption(OptionId),
}

/// The index of a heading or real item in the same immutable projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChoiceDisplayRow {
    Header(usize),
    Item(usize),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ChoiceGroups {
    boundaries: Vec<(usize, String)>,
    display_rows: Vec<ChoiceDisplayRow>,
    item_rows: Vec<usize>,
}

impl ChoiceGroups {
    pub(super) fn new(
        items: &[(OptionId, &str)],
        boundaries: &[OptionGroupBoundary<'_>],
    ) -> Result<Self, GroupedProjectionError> {
        use GroupedProjectionError::*;
        if items.is_empty() {
            return Err(EmptyItems);
        }
        if boundaries.first().ok_or(MissingBoundaries)?.start_index != 0 {
            return Err(FirstGroupStartsAfterZero);
        }
        let mut previous = None;
        for boundary in boundaries {
            let index = boundary.start_index;
            if index >= items.len() {
                return Err(BoundaryOutOfRange(index));
            }
            if previous.is_some_and(|previous| index <= previous) {
                return Err(BoundariesNotIncreasing(index));
            }
            if boundary.title.is_empty() {
                return Err(EmptyTitle(index));
            }
            previous = Some(index);
        }
        let mut ids = HashSet::with_capacity(items.len());
        for (id, _) in items {
            if !ids.insert(*id) {
                return Err(DuplicateOption(*id));
            }
        }

        let mut display_rows = Vec::with_capacity(items.len() + boundaries.len());
        let mut item_rows = Vec::with_capacity(items.len());
        let mut headings = boundaries.iter().enumerate().peekable();
        for item in 0..items.len() {
            if let Some(&(heading, boundary)) = headings.peek()
                && boundary.start_index == item
            {
                headings.next();
                display_rows.push(ChoiceDisplayRow::Header(heading));
            }
            item_rows.push(display_rows.len());
            display_rows.push(ChoiceDisplayRow::Item(item));
        }
        Ok(Self {
            boundaries: boundaries
                .iter()
                .map(|boundary| (boundary.start_index, boundary.title.to_owned()))
                .collect(),
            display_rows,
            item_rows,
        })
    }

    pub(crate) fn boundaries(&self) -> &[(usize, String)] {
        &self.boundaries
    }

    pub(crate) fn title(&self, heading: usize) -> &str {
        &self.boundaries[heading].1
    }

    /// The group in view at the leading display row and its real item count.
    pub(super) fn context_at(&self, first_row: usize) -> Option<(&str, usize)> {
        let heading = match self.row(first_row)? {
            ChoiceDisplayRow::Header(heading) => heading,
            ChoiceDisplayRow::Item(item) => {
                self.boundaries.partition_point(|(start, _)| *start <= item) - 1
            }
        };
        let (start, title) = &self.boundaries[heading];
        let end = self
            .boundaries
            .get(heading + 1)
            .map_or(self.item_rows.len(), |(start, _)| *start);
        Some((title, end - start))
    }

    pub(crate) fn row_count(&self) -> usize {
        self.display_rows.len()
    }

    pub(crate) fn row(&self, index: usize) -> Option<ChoiceDisplayRow> {
        self.display_rows.get(index).copied()
    }

    pub(crate) fn item_row(&self, index: usize) -> usize {
        self.item_rows[index]
    }
}
