use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::{str, thread, env};
use threadpool::ThreadPool;
use cached::proc_macro::cached;
use cached::SizedCache;

fn main() {
    let args: Vec<String> = env::args().collect();

    let bind = match args.get(1) {
        Some(b) => b,
        None => "127.0.0.1:8081",
    };

    println!("Starting HTTP-Connect proxy on port {}", bind);

    let listener = TcpListener::bind(bind).unwrap();
    let pool = ThreadPool::new(200);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                pool.execute(move|| {
                    handle_client(stream);
                });
            }
            Err(err) => println!("error {}", err),
        }
    }
}

fn handle_client(mut stream : TcpStream) {
    
    let mut data = [0; 8192];
    let rsize = stream.read(&mut data).unwrap();

    if let Ok(connect) = str::from_utf8(&data[..rsize]) {
        if connect.starts_with("CONNECT") {
            let endpoint:Vec<&str> = connect.split(" ").collect();
            let hostport:Vec<&str> = endpoint[1].split(":").collect();
            
            let (host, port) = (resolv(hostport[0]), hostport[1]);
            let endpoint = host + ":" + &port;

            if let Ok(mut server_stream) = TcpStream::connect(endpoint) {
                stream.write(&"HTTP/1.1 200 \r\n\r\n".as_bytes()).expect("Client disconnected");

                let mut c_server_stream = server_stream.try_clone().unwrap();
                let mut c_stream = stream.try_clone().unwrap();

                thread::spawn(move|| {
                   let mut data = [0; 8192];
                    loop {
                        if let Ok(rsize) = c_server_stream.read(&mut data[..]) {
                            if rsize == 0 {
                                break;
                            }
                            match c_stream.write(&data[..rsize]) {
                                Ok(_) => continue,
                                Err(_) => break,
                            }
                        } else {
                            break;
                        }
                    }
                });

                let mut init = [0; 30];
                let rsize = stream.read(&mut init[..]).expect("No initial data from client!");
                server_stream.write(&init[..rsize]).expect("Initial send error!");

                loop {
                   if let Ok(rsize) = stream.read(&mut data[..]) {
                        if rsize == 0 {
                            break;
                        }
                        match server_stream.write(&data[..rsize]) {
                            Ok(_) => continue,
                            Err(_) => break,
                        }
                   } else {
                       break;
                   }
                }

            } else {
                println!("Couldn't connect to server...");
            }
        }
    }
}

#[cached(
    type = "SizedCache<String, String>",
    create = "{ SizedCache::with_size(10000) }",
    convert = r#"{ format!("{}", hostname) }"#
)]
fn resolv(hostname: &str) -> String {
    println!("resolving {}", hostname);
    let client = reqwest::blocking::Client::new();
    let url = format!("https://cloudflare-dns.com/dns-query?name={}&type=A", hostname);
    let resp = client.get(&url)
        .header("accept", "application/dns-json")
        .send()
        .unwrap()
        .text()
        .unwrap();

    let r: serde_json::Value = serde_json::from_str(&resp).unwrap();

    String::from(r["Answer"].as_array().unwrap().last().unwrap()["data"].as_str().unwrap())
}

