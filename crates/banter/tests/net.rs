//! A real server on loopback.

mod common;

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    time::Duration,
};

use banter::net::{Event, Server, Silent, Tcp};
use common::counter;

fn serve(limit: usize, events: Option<Arc<Mutex<Vec<String>>>>) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let tcp = Tcp::from(listener).idle(Some(Duration::from_secs(5)));
    let server = Server::new(counter(), tcp).limit(limit);
    std::thread::spawn(move || match events {
        None => server.observer(Silent).run(),
        Some(log) => server
            .observer(move |e: Event<'_>| {
                let name = match e {
                    Event::Listening(_) => "listening",
                    Event::Accepted(_) => "accepted",
                    Event::Busy(_) => "busy",
                    Event::Closed(..) => "closed",
                    _ => "other",
                };
                log.lock().unwrap().push(name.into());
            })
            .run(),
    });
    addr
}

fn until_prompt(reader: &mut impl BufRead) -> String {
    let mut out = Vec::new();
    while !out.ends_with(b"c> ") {
        let mut byte = [0];
        reader.read_exact(&mut byte).unwrap();
        out.push(byte[0]);
    }
    String::from_utf8(out).unwrap()
}

#[test]
fn serves_tcp_sessions() {
    let addr = serve(4, None);
    let stream = TcpStream::connect(addr).unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut writer = stream;
    assert_eq!(until_prompt(&mut reader), "hello\nc> ");
    writer.write_all(b"add 2\n").unwrap();
    assert_eq!(until_prompt(&mut reader), "  n  2\nc> ");
}

#[test]
fn turns_peers_away_past_the_limit_and_reports_events() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let addr = serve(1, Some(Arc::clone(&log)));
    let first = TcpStream::connect(addr).unwrap();
    until_prompt(&mut BufReader::new(first.try_clone().unwrap()));

    let mut second = TcpStream::connect(addr).unwrap();
    let mut reply = String::new();
    second.read_to_string(&mut reply).unwrap();
    assert_eq!(reply, "busy, try again shortly\n");

    drop(first);
    std::thread::sleep(Duration::from_millis(100));
    let log = log.lock().unwrap();
    assert_eq!(log[..3], ["listening", "accepted", "busy"]);
    assert!(log.contains(&"closed".to_string()));
}
