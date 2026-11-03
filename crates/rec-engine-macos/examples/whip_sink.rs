//! A tiny local WHIP endpoint for testing Go live without MediaMTX: accepts
//! one WHIP POST on 127.0.0.1, answers it with str0m, receives the media and
//! writes the video frames (Annex B) to a file for ffprobe. Test use only.
//!
//! cargo run --release -p rec-engine-macos --example whip_sink -- [PORT] [OUT.h264] [SECS]
//! then stream to http://127.0.0.1:PORT/whip

use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::{TcpListener, UdpSocket};
use std::time::{Duration, Instant};

use str0m::change::SdpOffer;
use str0m::format::Codec;
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, IceConnectionState, Input, Output, Rtc};

fn main() {
    let mut args = std::env::args().skip(1);
    let port: u16 = args.next().and_then(|p| p.parse().ok()).unwrap_or(18889);
    let out = args.next().unwrap_or_else(|| "whip-received.h264".into());
    let secs: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(60);
    let http = TcpListener::bind(("127.0.0.1", port)).expect("bind HTTP");
    println!("whip_sink: listening on http://127.0.0.1:{port}/whip");

    // 1. The WHIP POST: SDP offer in, SDP answer + Location out. A client
    // reconnecting first DELETEs its old session (from a previous sink): OK it.
    let (mut conn, offer) = loop {
        let (mut conn, _) = http.accept().expect("accept");
        let (method, auth, body) = read_request(&mut conn);
        println!("whip_sink: {method}, authorization {}", if auth.is_empty() { "none" } else { "present" });
        if method.starts_with("POST") {
            break (conn, body);
        }
        let _ = write!(conn, "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    };
    let socket = UdpSocket::bind("127.0.0.1:0").expect("bind UDP");
    let mut rtc = Rtc::builder()
        .clear_codecs()
        .enable_h264(true)
        .enable_h265(true)
        .enable_opus(true, false)
        .build(Instant::now());
    rtc.add_local_candidate(Candidate::host(socket.local_addr().unwrap(), "udp").unwrap());
    let offer = SdpOffer::from_sdp_string(&offer).expect("SDP offer");
    let answer = rtc.sdp_api().accept_offer(offer).expect("accept offer").to_sdp_string();
    write!(
        conn,
        "HTTP/1.1 201 Created\r\nContent-Type: application/sdp\r\nLocation: /whip/session\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}",
        answer.len()
    )
    .unwrap();
    drop(conn);

    // 2. A DELETE of the session ends the stream (WHIP's stop).
    http.set_nonblocking(true).unwrap();
    let mut file = std::fs::File::create(&out).expect("create output");
    let (mut video, mut keys, mut audio, mut bytes) = (0u64, 0u64, 0u64, 0u64);
    let (mut first, mut last, mut codecs) = (None::<Instant>, Instant::now(), vec![]);
    let end = Instant::now() + Duration::from_secs(secs);
    let mut buf = vec![0u8; 2000];
    let reason = 'run: loop {
        if Instant::now() > end {
            break "time limit";
        }
        if let Ok((mut c, _)) = http.accept() {
            c.set_nonblocking(false).ok();
            let (m, ..) = read_request(&mut c);
            let _ = write!(c, "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            if m.starts_with("DELETE") {
                break "DELETE received";
            }
        }
        let timeout = loop {
            match rtc.poll_output().expect("poll") {
                Output::Timeout(t) => break t,
                Output::Transmit(t) => {
                    let _ = socket.send_to(&t.contents, t.destination);
                }
                Output::Event(Event::IceConnectionStateChange(IceConnectionState::Disconnected)) => {
                    break 'run "ICE disconnected";
                }
                Output::Event(Event::Connected) => println!("whip_sink: connected"),
                Output::Event(Event::MediaData(d)) => {
                    let codec = d.params.spec().codec;
                    if !codecs.contains(&codec) {
                        codecs.push(codec);
                    }
                    last = Instant::now();
                    first.get_or_insert(last);
                    if matches!(codec, Codec::Opus) {
                        audio += 1;
                    } else {
                        video += 1;
                        bytes += d.data.len() as u64;
                        if let str0m::format::CodecExtra::H264(e) = d.codec_extra
                            && e.is_keyframe
                        {
                            keys += 1;
                        }
                        file.write_all(&d.data).unwrap();
                    }
                }
                Output::Event(_) => {}
            }
        };
        let wait = timeout.saturating_duration_since(Instant::now()).clamp(Duration::from_millis(1), Duration::from_millis(20));
        socket.set_read_timeout(Some(wait)).unwrap();
        let input = match socket.recv_from(&mut buf) {
            Ok((n, source)) => Input::Receive(
                Instant::now(),
                Receive {
                    proto: Protocol::Udp,
                    source,
                    destination: socket.local_addr().unwrap(),
                    contents: buf[..n].try_into().unwrap(),
                },
            ),
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => Input::Timeout(Instant::now()),
            Err(e) => panic!("recv: {e}"),
        };
        rtc.handle_input(input).expect("input");
    };
    let span = first.map_or(0.0, |f| (last - f).as_secs_f64());
    println!(
        "whip_sink: stopped ({reason}); codecs {codecs:?}; {video} video frames ({keys} H.264 keyframes, {:.0} kb/s), {audio} audio packets over {span:.2} s",
        bytes as f64 * 8.0 / span.max(0.001) / 1000.0
    );
}

/// (request line, Authorization header, body) of one HTTP request.
fn read_request(conn: &mut std::net::TcpStream) -> (String, String, String) {
    let mut r = BufReader::new(conn);
    let mut line = String::new();
    r.read_line(&mut line).unwrap();
    let (mut len, mut auth) = (0usize, String::new());
    loop {
        let mut h = String::new();
        r.read_line(&mut h).unwrap();
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        let lower = h.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") {
            len = v.trim().parse().unwrap_or(0);
        }
        if lower.starts_with("authorization:") {
            auth = h["authorization:".len()..].trim().to_string();
        }
    }
    let mut body = vec![0; len];
    r.read_exact(&mut body).unwrap();
    (line.trim_end().to_string(), auth, String::from_utf8_lossy(&body).into_owned())
}
