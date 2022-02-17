use env_logger;
use std::io::{ Write, Error, ErrorKind };
use log::Level::Debug;
use log::log_enabled;

const MAX: usize = 200000;
const MIN: usize = 180000;

pub fn capture_compute_units() {
    if log_enabled!(Debug) {
        println!("Debug logging not enabled!");
        return
    }

    let mut builder = env_logger::builder();
    builder.is_test(true);
    builder.format(|buf, record| {
        let msg = format!("{}", record.args());
        let m = msg.split(" ").collect::<Vec<&str>>();
        if let Ok(n) = m[2].parse::<usize>() {
            return writeln!(buf, "{}", n,);
        }

        Err(Error::new(ErrorKind::Other, "oh no!"))
    });
    builder.init();
}

pub fn check_compute_units() {
    if log_enabled!(Debug) {
        println!("Debug logging not enabled!");
        return
    }

    let mut builder = env_logger::builder();
    builder.is_test(true);
    builder.format(|buf, record| {
        let msg = format!("{}", record.args());
        if let Ok(n) = msg.split(" ").last().unwrap().parse::<usize>() {
            let mut overflow = String::new();

            if n > MAX {
                overflow = format!("\t\x1b[31m{}\x1b[0m CUs too much!", n - MAX);
            } else if n < MIN {
                overflow = format!("\t\x1b[34m{}\x1b[0m CUs unused!", MIN - n);
            }

            let color = if n > MAX {
                "31"
            } else if n > 195_000 {
                "33"
            } else if n < MIN {
                "34"
            } else {
                "32"
            };

            return writeln!(buf,
                "Program call: \t CUs: \x1b[{}m{}\x1b[0m {}",
                color,
                n,
                overflow,
            );
        }

        Err(Error::new(ErrorKind::Other, "oh no!"))
    });
    builder.init();
}