use crate::LwskError;

/// What action to perform at this entry in the [Schedule]
#[derive(Clone, Debug)]
pub enum ScheduleEntry {
    /// Run a function
    FunctionInvocation(usize),

    /// Sample IO driver, writing to a channel
    IoIn {
        from_io_idx: usize,
        to_channel_idx: usize,
    },

    /// Push data from channel out via IO driver
    IoOut {
        from_channel_idx: usize,
        to_io_idx: usize,
    },

    /// Wait for a specified period of time
    Wait(core::time::Duration),
}

/// A schedule contains a fixed sequence of actions to perform
pub struct Schedule {
    /// Sequence of events
    entry_sequence: Vec<ScheduleEntry>,

    /// Index to the event sequence
    pub current_event: usize,
}

impl Schedule {
    /// Initialize a new Schedule
    pub fn new<I: Into<Vec<ScheduleEntry>>>(entries: I) -> Result<Self, LwskError> {
        let order = entries.into();

        // an empty schedule is wrong, as we guarantee to return an ScheduleEntry in ::next()
        if order.is_empty() {
            return Err(LwskError::DriverError(0));
        }

        trace!("Current schedule: {:?}", order);

        Ok(Self {
            entry_sequence: order,
            current_event: 0,
        })
    }

    pub fn next(&mut self) -> ScheduleEntry {
        debug_assert!(
            !self.entry_sequence.is_empty(),
            "the schedule must never be empty"
        );

        self.current_event = self.current_event.wrapping_add(1) % self.entry_sequence.len();
        self.entry_sequence[self.current_event].clone()
    }
}

impl Iterator for Schedule {
    type Item = ScheduleEntry;

    fn next(&mut self) -> Option<Self::Item> {
        Some(Self::next(self))
    }
}
