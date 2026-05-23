use std::fmt::Write;

use crate::simulation::SimulationEvent;

pub fn render_text(events: &[SimulationEvent]) -> String {
    let mut output = String::new();
    for event in events {
        let _ = writeln!(output, "Month {}: {}", event.month, event.summary);
    }
    output
}

pub fn render_markdown(events: &[SimulationEvent]) -> String {
    let mut output = String::from("# Mundis Chronicle\n");
    let mut current_year = None;

    for event in events {
        let year = ((event.month - 1) / 12) + 1;
        if current_year != Some(year) {
            current_year = Some(year);
            let _ = writeln!(output, "\n## Year {}", year);
        }
        let _ = writeln!(output, "- Month {}: {}", event.month, event.summary);
    }

    output
}

pub fn render_json(events: &[SimulationEvent]) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(events)
}
