use crate::LwskError;

// TODO do something similar to enum_dispatch
/// What action to perform at this entry in the [Schedule]
#[derive(Clone, Debug, PartialEq, Eq)]
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

    /// Switch to other schedule
    SwitchSchedule(usize),
}

/// A schedule contains a fixed sequence of actions to perform
pub struct Schedule {
    /// Name of this schedule
    pub name: String,

    /// Sequence of actions
    pub sequence: Vec<ScheduleEntry>,

    /// Index to the event sequence
    pub current_action: usize,
}

impl Schedule {
    /// Initialize a new Schedule
    pub fn new<I: Into<Vec<ScheduleEntry>>>(name: String, entries: I) -> Result<Self, LwskError> {
        let order = entries.into();

        // an empty schedule is wrong, as we guarantee to return an ScheduleEntry in ::next()
        if order.is_empty() {
            return Err(LwskError::EmptySchedule);
        }

        trace!("Current schedule: {:?}", order);

        Ok(Self {
            name,
            sequence: order,
            current_action: 0,
        })
    }

    pub fn next_action(&mut self) -> ScheduleEntry {
        debug_assert!(
            !self.sequence.is_empty(),
            "the schedule must never be empty"
        );

        self.current_action = self.current_action.wrapping_add(1) % self.sequence.len();
        self.sequence[self.current_action].clone()
    }
}

impl Iterator for Schedule {
    type Item = ScheduleEntry;

    fn next(&mut self) -> Option<Self::Item> {
        Some(Self::next_action(self))
    }
}
