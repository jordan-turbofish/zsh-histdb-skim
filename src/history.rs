extern crate skim;
use crate::environment::*;
use chrono::Timelike;
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use humantime::format_duration;
use skim::prelude::*;
use std::time::Duration;
use textwrap::fill;

#[derive(Clone, Debug)]
pub struct History {
    pub id: i64,
    pub cmd: String,
    pub start: u64,
    pub exit_status: Option<i64>,
    pub duration: Option<i64>,
    pub count: i64,
    pub session: i64,
    pub host: String,
    pub dir: String,
    pub searchrange: [(usize, usize); 1],
}

impl History {
    pub const FORMAT_DATE_LENGTH: usize = 10;
    pub const COMMAND_START: usize = (History::FORMAT_DATE_LENGTH + 1);

    pub fn command(&self) -> &String {
        return &self.cmd;
    }
}

impl History {
    fn format_date(&self, full: bool) -> String {
        let naive = NaiveDateTime::from_timestamp_opt(self.start as i64, 0).unwrap_or_default();
        let start_time: DateTime<Local> = Local.from_utc_datetime(&naive);
        let current_time: DateTime<Local> = Local::now();
        let seconds_since_midnight = (current_time.hour() * 3600
            + current_time.minute() * 60
            + current_time.second()) as i64;
        let day_beginning = current_time.timestamp() - seconds_since_midnight;
        if full {
            let mut dateinfo = String::from("");
            dateinfo.push_str(&get_date_format());
            dateinfo.push_str(" %H:%M");
            return format!("{}", start_time.format(&dateinfo));
        } else if start_time.timestamp() > day_beginning {
            return format!("{}", start_time.format("%H:%M"));
        } else {
            return format!("{}", start_time.format(&get_date_format()));
        }
    }

    fn format_or_none(x: Option<i64>) -> String {
        if x.is_some() {
            format!("{}", x.unwrap())
        } else {
            "\x1b[37;1m<NONE>\x1b[0m".to_string()
        }
    }

    fn format_duration(&self) -> String {
        if self.duration.is_some() {
            let duration = Duration::from_secs(self.duration.unwrap() as u64);
            format_duration(duration).to_string()
        } else {
            History::format_or_none(self.duration)
        }
    }
}

impl SkimItem for History {
    fn text(&self) -> Cow<str> {
        let information = format!("{:10} {}", self.format_date(false), self.cmd);
        Cow::Owned(information)
    }

    fn preview(&self, _context: PreviewContext) -> ItemPreview {
        let mut information = String::from(format!("\x1b[1mDetails for {}\x1b[0m\n\n", self.id));

        let mut tformat = |name: &str, value: &str| {
            information.push_str(&format!("\x1b[1m{:20}\x1b[0m{}\n", name, value));
        };

        tformat("Runtime", &self.format_duration());
        tformat("Host", &self.host);
        tformat("Executed", &self.count.to_string());
        tformat("Directory", &self.dir);
        tformat("Exit Status", &History::format_or_none(self.exit_status));
        tformat("Session", &self.session.to_string());
        tformat("Start Time", &self.format_date(false));
        information.push_str(&format!(
            "\x1b[1mCommand\x1b[0m\n\n{}\n",
            &fill(&self.cmd, _context.width)
        ));
        ItemPreview::AnsiText(information)
    }

    fn get_matching_ranges(&self) -> Option<&[(usize, usize)]> {
        Some(&self.searchrange)
    }
}
