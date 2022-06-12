use std::io::Write;
use log::LevelFilter;
use log4rs::append::file::FileAppender;
use log4rs::encode::pattern::PatternEncoder;
use log4rs::config::{Appender, Config, Root};
use std::fs::File;
use std::io::{self, BufRead};
use regex::Regex;

const LOG_LOCATION: &str = "log/output.log";

pub fn save_debug_log() {
    _ = std::fs::remove_file(LOG_LOCATION);

    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{l} - {m}\n")))
        .build(LOG_LOCATION).unwrap();

    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(Root::builder()
                   .appender("logfile")
                   .build(LevelFilter::Debug)).unwrap();

    log4rs::init_config(config).unwrap();
}

/// When using `solana_program::log::sol_log_compute_units` before and after a command, we are able to compute the required (min, max, avg CUs used)
pub fn get_compute_unit_pairs_from_log() {
    let file = File::open(LOG_LOCATION).unwrap();
    let lines = io::BufReader::new(file).lines();
    let re = Regex::new(r"(?x) (.*) (Program) \s (consumption.) \s (?P<compute_units>\d*) \s (units) \s (remaining) (.*) ").unwrap();

    let mut cus = Vec::new();
    let mut v = 0;
    let mut a = true;

    for line in lines {
        match line {
            Ok(line) => {
                if line.contains("Program consumption") {
                    let caps = re.captures(&line).unwrap();
                    let cu: usize = (&caps["compute_units"]).parse().unwrap();

                    if a {
                        v = cu;
                    } else {
                        cus.push(v - cu);
                        v = 0;
                    }

                    a = !a;
                }
            },
            Err(_) => return,
        }
    }

    let max = cus.iter().fold(0, |a, b| std::cmp::max(a, *b));
    let min = cus.iter().fold(usize::MAX, |a, b| std::cmp::min(a, *b));
    let avg = if !cus.is_empty() { cus.iter().sum::<usize>() / cus.len() } else { 0 };
    let mut out = format!("(CUs: max: {}, min: {}, avg: {})", max, min, avg);

    for c in cus {
        out.push_str(&format!("\n{}", c));
    }

    let mut output = File::create("log/compute_units.log").unwrap();
    write!(output, "{}", out).unwrap();
}