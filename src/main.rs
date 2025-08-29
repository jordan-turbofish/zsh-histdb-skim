extern crate skim;
mod environment;
mod history;
mod location;
mod query;
mod title;

use crate::environment::*;
use crate::history::History;
use crate::location::Location;
use crate::query::build_query_string;
use crate::title::generate_title;

use rusqlite::{Connection, OpenFlags, Result};
use skim::prelude::*;
use std::collections::HashSet;
use std::env;
use std::sync::Mutex;
use std::thread;

struct HistoryCollection {
    collection: Vec<History>,
    filled: bool,
}

impl HistoryCollection {
    fn new() -> Self {
        HistoryCollection {
            collection: Vec::new(),
            filled: false,
        }
    }
}

fn read_entries(history_collection: Arc<Mutex<HistoryCollection>>) {
    let conn_res =
        Connection::open_with_flags(get_histdb_database(), OpenFlags::SQLITE_OPEN_READ_ONLY);
    if conn_res.is_err() {
        return;
    }
    let conn = conn_res.unwrap();

    let s = build_query_string();

    let stmt_result = conn.prepare(&s);
    if stmt_result.is_err() {
        return;
    }
    let mut stmt = stmt_result.unwrap();

    let history_entries = stmt
        .query_map([], |row| {
            let cmd: String = row.get("cmd")?;
            let commandend = cmd.len();
            Ok(History {
                id: row.get("id")?,
                cmd,
                start: row.get("start")?,
                exit_status: row.get("exit_status")?,
                duration: row.get("duration")?,
                count: row.get("count")?,
                session: row.get("session")?,
                host: row.get("host")?,
                dir: row.get("dir")?,
                searchrange: [(
                    History::COMMAND_START,
                    commandend + (History::COMMAND_START),
                )],
            })
        })
        .unwrap();

    let mut filtered_history_entries = history_entries.filter_map(|x| x.ok()).peekable();

    'outer: while filtered_history_entries.peek().is_some() {
        let mut c = history_collection.lock().unwrap();
        for _ in 0..100 {
            if let Some(history_entry) = filtered_history_entries.next() {
                c.collection.push(history_entry);
            } else {
                break 'outer;
            }
        }
    }

    let mut c = history_collection.lock().unwrap();
    c.filled = true;
}

fn filter_entry(location: &Location, app_state: &AppState, entry: &History) -> bool {
    match location {
        Location::Session => entry.session == app_state.session && entry.host == app_state.machine,
        Location::Directory => entry.dir == app_state.dir && entry.host == app_state.machine,
        Location::Machine => entry.host == app_state.machine,
        Location::Everywhere => true,
    }
}

struct AppState {
    session: i64,
    dir: String,
    machine: String,
}

fn filter_entries(
    history_collection: Arc<Mutex<HistoryCollection>>,
    location: &Location,
    tx_item: SkimItemSender,
    end_early: Arc<Mutex<bool>>,
    grouped: bool,
) {
    let app_state = AppState {
        session: get_current_session_id().parse::<i64>().unwrap(),
        dir: get_current_dir(),
        machine: get_current_host(),
    };

    // TODO: Do we care about grouping across hosts?
    let mut seen_commands = HashSet::new();
    let mut filled = false;
    let mut len = 0;
    let mut next_idx = 0;
    let block_size = 100;

    while !filled || next_idx < len {
        {
            let end_early = end_early.lock().unwrap();
            if *end_early {
                break;
            }
        }
        // try to grab up to block_size items
        let mut entries_block: Vec<Arc<dyn SkimItem>> = Vec::with_capacity(block_size);
        let c = history_collection.lock().unwrap();
        filled = c.filled;
        len = c.collection.len();

        for i in next_idx..len {
            if (!grouped || !seen_commands.contains(&c.collection[i].cmd))
                && filter_entry(location, &app_state, &c.collection[i])
            {
                entries_block.push(Arc::new(c.collection[i].clone()));
                seen_commands.insert(c.collection[i].cmd.clone());
            }
            next_idx = i+1;
            if entries_block.len() == block_size {
                break;
            }
        }

        if entries_block.len() > 0 {
            let _ = tx_item.send(entries_block);
        }
    }
}

enum SelectionResult {
    Command(String),
    NullCommand,
    Continue,
    Abort,
}

fn get_starting_location() -> Location {
    let mut location = Location::Session;
    if get_current_session_id().is_empty() {
        location = Location::Directory;
    }
    location
}

fn show_history(thequery: String) -> Result<String, String> {
    let mut location = get_starting_location();
    let mut grouped = true;
    let mut query = thequery;
    let history_collection = Arc::new(Mutex::new(HistoryCollection::new()));

    let _handle = {
        let history_collection = history_collection.clone();
        thread::spawn(move || {
            read_entries(history_collection);
        })
    };

    loop {
        let title = generate_title(&location);

        let options = SkimOptionsBuilder::default()
            .height(Some("100%"))
            .multi(false)
            .reverse(true)
            .prompt(Some(">"))
            .query(Some(&query))
            .bind(vec![
                "f1:abort",
                "f2:abort",
                "f3:abort",
                "f4:abort",
                "f5:abort",
                "ctrl-r:abort",
                "ctrl-u:half-page-up",
                "ctrl-d:half-page-down",
            ])
            .header(Some(&title))
            .preview(Some("")) // preview should be specified to enable preview window
            .nosort(get_nosort_option())
            .build()
            .unwrap();

        let (tx_item, rx_item): (SkimItemSender, SkimItemReceiver) = unbounded();
        let end_early = Arc::new(Mutex::new(false));

        let handle = {
            let history_collection = history_collection.clone();
            let end_early = end_early.clone();
            thread::spawn(move || {
                filter_entries(history_collection, &location, tx_item, end_early, grouped);
            })
        };

        let selected_items = Skim::run_with(&options, Some(rx_item));

        if let Some(output) = &selected_items {
            if output.is_abort {
                let mut end_early = end_early.lock().unwrap();
                *end_early = true;
            }
        }
        handle.join().unwrap();

        let selection_result = process_result(&selected_items, &mut location, &mut grouped);

        match selection_result {
            SelectionResult::Abort => return Err("Aborted".to_string()),
            SelectionResult::Continue => query = selected_items.unwrap().query,
            SelectionResult::Command(command) => return Ok(command),
            SelectionResult::NullCommand => return Ok(selected_items.unwrap().query),
        };
    }
}

fn process_result(
    selected_items: &Option<SkimOutput>,
    loc: &mut Location,
    grouped: &mut bool,
) -> SelectionResult {
    if selected_items.is_some() {
        let sel = selected_items.as_ref().unwrap();
        match sel.final_key {
            Key::ESC | Key::Ctrl('c') | Key::Ctrl('d') | Key::Ctrl('z') => {
                return SelectionResult::Abort;
            }
            Key::Enter => {
                if sel.selected_items.is_empty() {
                    return SelectionResult::NullCommand;
                } else {
                    return SelectionResult::Command(sel.selected_items[0].output().to_string());
                }
            }
            Key::F(1) => {
                *loc = Location::Session;
            }
            Key::F(2) => {
                *loc = Location::Directory;
            }
            Key::F(3) => {
                *loc = Location::Machine;
            }
            Key::F(4) => {
                *loc = Location::Everywhere;
            }
            Key::F(5) => {
                *grouped = !*grouped;
            }
            Key::Ctrl('r') => {
                *loc = match *loc {
                    Location::Session => Location::Directory,
                    Location::Directory => Location::Machine,
                    Location::Machine => Location::Everywhere,
                    Location::Everywhere => Location::Session,
                };
            }
            _ => (),
        };
        SelectionResult::Continue
    } else {
        SelectionResult::Continue
    }
}

fn main() -> Result<()> {
    let _conn =
        Connection::open_with_flags(get_histdb_database(), OpenFlags::SQLITE_OPEN_READ_ONLY);

    let args: Vec<String> = env::args().collect();
    let query = if args.len() > 1 {
        args[1].to_string()
    } else {
        "".to_string()
    };

    if query == "--version" {
        println!("v0.8.20");
        std::process::exit(1);
    }

    let result = show_history(query);
    if result.is_ok() {
        println!("{}", result.ok().unwrap());
    } else {
        eprintln!("{}", result.err().unwrap());
        std::process::exit(1);
    }

    Ok(())
}
