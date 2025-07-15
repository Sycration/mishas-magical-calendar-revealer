use std::{
    collections::HashMap,
    env,
    io::{Read, Write},
    sync::{Arc, Mutex},
};

use axum::{
    Form, debug_handler,
    extract::State,
    response::{IntoResponse, Redirect},
};
use axum_template::{RenderHtml, engine};
use chrono::{Datelike, Duration, Local, NaiveDateTime, Timelike, Utc};
use rrule::{RRule, RRuleSet, Tz, Unvalidated, Validated};
use serde::{Deserialize, Serialize};
use serde_json::json;
use ureq::Agent;
use url::Url;

use crate::AppState;

#[derive(Deserialize)]
pub(crate) struct CalendarPage {
    number: u64,
}

#[derive(Debug, Clone)]
struct MyEvent {
    start_time: chrono::DateTime<Utc>,
    end_time: chrono::DateTime<Utc>,
    name: String,
    location: String,
    notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PrettyEvent {
    starts: String,
    ends: String,
    name: String,
    location: String,
    notes: String,
}

impl MyEvent {
    fn pretty(self) -> PrettyEvent {
        PrettyEvent {
            starts: self
                .start_time
                .with_timezone(&Local)
                .format("%A, %B %d, %Y at %l:%M %p")
                .to_string(),
            ends: self
                .end_time
                .with_timezone(&Local)
                .format("%A, %B %d, %Y at %l:%M %p")
                .to_string(),
            name: self.name,
            location: self.location,
            notes: self.notes,
        }
    }
}

fn parse_timestamp(ts: &String) -> chrono::DateTime<Utc> {
    if ts.contains('T') {
        iso8601::datetime(ts)
            .unwrap()
            .into_fixed_offset()
            .unwrap()
            .to_utc()
    } else {
        iso8601::date(ts)
            .unwrap()
            .into_naive()
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
    }
}

pub(crate) async fn calendar(
    State(AppState { engine }): State<AppState>,
    Form(form): Form<CalendarPage>,
) -> impl IntoResponse {
    let mut used_file = std::fs::OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open("./used.txt")
        .unwrap();
    //std::fs::File::create("./used.txt").unwrap();
    let mut contents = String::new();
    used_file.read_to_string(&mut contents).unwrap();
    let used_numbers: Vec<u64> = contents
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect();
    if used_numbers.contains(&form.number) {
        return Redirect::to("/tryagain?reason=already%20used").into_response();
    }
    if !env::var("MAGIC_NUMBERS")
        .unwrap()
        .split(',')
        .filter_map(|s| s.parse::<u64>().ok())
        .fold(false, |acc, x| if !acc { form.number == x } else { true })
    {
        return Redirect::to("/tryagain?reason=invalid").into_response();
    }
    used_file
        .write(format!("{}\n", form.number).as_bytes())
        .unwrap();

    let url = Url::parse(&env::var("URL").unwrap()).unwrap();
    let username = env::var("USERNAME").unwrap();
    let password = env::var("PASSWORD").unwrap();
    let credentials = minicaldav::Credentials::Basic(username.into(), password.into());
    let agent = Agent::new();
    let calendars = minicaldav::get_calendars(agent.clone(), &credentials, &url).unwrap();

    let mut singular_events = vec![];
    let now = Utc::now();
    let two_weeks_ago = now - chrono::Duration::weeks(2);
    let one_week_later = now + chrono::Duration::weeks(1);
    for calendar in calendars {
        let (events, _) = minicaldav::get_events(agent.clone(), &credentials, &calendar).unwrap();

        for event in events {
            let properties = event
                .properties()
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect::<HashMap<String, String>>();
            if let Some(rrule) = properties.get("RRULE") {
                let start_time = parse_timestamp(properties.get("DTSTART").unwrap());
                let end_time = parse_timestamp(properties.get("DTEND").unwrap());

                let name = properties
                    .get("SUMMARY")
                    .unwrap_or(&"Unknown Event".to_string())
                    .to_owned();
                let location = properties
                    .get("LOCATION")
                    .unwrap_or(&"No location".to_string())
                    .to_owned();
                let notes = properties
                    .get("DESCRIPTION")
                    .unwrap_or(&"".to_string())
                    .to_owned();

                let rrule: RRule<Unvalidated> = rrule.parse().unwrap();
                let rrule = rrule
                    .validate(start_time.with_timezone(&rrule::Tz::Tz(chrono_tz::Tz::UTC)))
                    .unwrap();

                let times =
                    RRuleSet::new(start_time.with_timezone(&rrule::Tz::Tz(chrono_tz::Tz::UTC)))
                        .before(one_week_later.with_timezone(&rrule::Tz::Tz(chrono_tz::Tz::UTC)))
                        .after(two_weeks_ago.with_timezone(&rrule::Tz::Tz(chrono_tz::Tz::UTC)))
                        .rrule(rrule)
                        .all(256)
                        .dates;

                for time in times {
                    singular_events.push(MyEvent {
                        start_time: time.to_utc(),
                        end_time: {
                            let delta = end_time - start_time;
                            (time + delta).to_utc()
                        },
                        name: name.clone(),
                        location: location.clone(),
                        notes: notes.clone(),
                    });
                }
            } else {
                let start_time = parse_timestamp(properties.get("DTSTART").unwrap());
                let end_time = parse_timestamp(properties.get("DTEND").unwrap());

                let name = properties
                    .get("SUMMARY")
                    .unwrap_or(&"Unknown Event".to_string())
                    .to_owned();
                let location = properties
                    .get("LOCATION")
                    .unwrap_or(&"No location".to_string())
                    .to_owned();
                let notes = properties
                    .get("DESCRIPTION")
                    .unwrap_or(&"".to_string())
                    .to_owned();

                singular_events.push(MyEvent {
                    start_time,
                    end_time,
                    name,
                    location,
                    notes,
                });
            }
        }
    }
    singular_events.retain(|e| e.start_time > two_weeks_ago && e.start_time < one_week_later);
    singular_events.sort_by(|a, b| a.start_time.cmp(&b.start_time));

    let events = singular_events
        .into_iter()
        .map(MyEvent::pretty)
        .collect::<Vec<_>>();

    let data = json!({
        "events": events
    });
    return RenderHtml("calendar.hbs", engine, data).into_response();
}

