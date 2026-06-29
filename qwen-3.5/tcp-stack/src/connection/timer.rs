//! TCP timers

use std::time::{Duration, Instant};

/// TCP Timer
pub struct Timer {
  deadline: Option<Instant>,
  duration: Duration,
}

impl Timer {
  pub fn new() -> Self {
    Self {
      deadline: None,
      duration: Duration::ZERO,
    }
  }

  pub fn start(&mut self, duration: Duration) {
    self.duration = duration;
    self.deadline = Some(Instant::now() + duration);
  }

  pub fn cancel(&mut self) {
    self.deadline = None;
  }

  pub fn is_expired(&self) -> bool {
    self.deadline.is_some_and(|dl| Instant::now() >= dl)
  }

  pub fn time_until_expiry(&self) -> Option<Duration> {
    self.deadline.map(|dl| {
      let now = Instant::now();
      if dl > now { dl - now } else { Duration::ZERO }
    })
  }

  pub fn reset(&mut self) {
    if self.deadline.is_some() {
      self.deadline = Some(Instant::now() + self.duration);
    }
  }
}

impl Default for Timer {
  fn default() -> Self {
    Self::new()
  }
}
