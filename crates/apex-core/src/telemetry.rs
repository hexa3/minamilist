use std::time::Duration;

/// Explicit input-to-frame performance budget used by renderer instrumentation.
#[derive(Clone, Copy, Debug)]
pub struct LatencyBudget {
    pub target: Duration,
    pub warning: Duration,
}

impl Default for LatencyBudget {
    fn default() -> Self {
        Self {
            target: Duration::from_millis(5),
            warning: Duration::from_millis(8),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LatencySample {
    count: u64,
    total: Duration,
    maximum: Duration,
}

impl LatencySample {
    pub fn record(&mut self, duration: Duration) {
        self.count += 1;
        self.total += duration;
        self.maximum = self.maximum.max(duration);
    }

    pub fn count(&self) -> u64 {
        self.count
    }

    pub fn average(&self) -> Duration {
        if self.count == 0 {
            Duration::ZERO
        } else {
            self.total / self.count as u32
        }
    }

    pub fn maximum(&self) -> Duration {
        self.maximum
    }

    pub fn within(&self, budget: LatencyBudget) -> bool {
        self.maximum <= budget.warning
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_average_and_regression_threshold() {
        let mut sample = LatencySample::default();
        sample.record(Duration::from_millis(2));
        sample.record(Duration::from_millis(4));
        assert_eq!(sample.average(), Duration::from_millis(3));
        assert!(sample.within(LatencyBudget::default()));
    }
}
